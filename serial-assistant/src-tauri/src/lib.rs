use std::collections::HashMap;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum TaskType {
    Send { data: Vec<u8> },
    Delay { ms: u64 },
    Jump { target: usize },
    LoopStart { count: u32 },
    LoopEnd,
    Stop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskItem {
    pub index: usize,
    pub task_type: TaskType,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum QueueStatus {
    Idle,
    Running,
    Paused,
    Stopped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueStatusPayload {
    pub status: QueueStatus,
    pub current_index: Option<usize>,
}

struct PortConnection {
    port: Arc<Mutex<Option<Box<dyn serialport::SerialPort>>>>,
    running: Arc<AtomicBool>,
    read_thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}

struct QueueState {
    running: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    thread_handle: Mutex<Option<std::thread::JoinHandle<()>>>,
}

struct SerialState {
    connections: Mutex<HashMap<String, PortConnection>>,
    queues: Mutex<HashMap<String, QueueState>>,
}

#[tauri::command]
fn get_serial_ports() -> Result<Vec<String>, String> {
    match serialport::available_ports() {
        Ok(ports) => {
            let port_names: Vec<String> = ports.iter().map(|p| p.port_name.clone()).collect();
            Ok(port_names)
        }
        Err(e) => Err(format!("获取串口失败: {}", e)),
    }
}

#[tauri::command]
fn open_serial(
    state: State<'_, SerialState>,
    app: AppHandle,
    port_id: String,
    port_name: String,
    baud_rate: u32,
    data_bits: u8,
    stop_bits: u8,
    parity: String,
) -> Result<(), String> {
    // Close existing connection for this port_id if any
    {
        let mut connections = state.connections.lock().map_err(|e| e.to_string())?;
        if let Some(existing) = connections.remove(&port_id) {
            existing.running.store(false, Ordering::Relaxed);
            if let Ok(mut handle_guard) = existing.read_thread.lock() {
                if let Some(handle) = handle_guard.take() {
                    let _ = handle.join();
                }
            }
        }
    }

    let builder = serialport::new(&port_name, baud_rate)
        .data_bits(match data_bits {
            5 => serialport::DataBits::Five,
            6 => serialport::DataBits::Six,
            7 => serialport::DataBits::Seven,
            8 => serialport::DataBits::Eight,
            _ => return Err("Invalid data bits".into()),
        })
        .stop_bits(match stop_bits {
            1 => serialport::StopBits::One,
            2 => serialport::StopBits::Two,
            _ => return Err("Invalid stop bits".into()),
        })
        .parity(match parity.as_str() {
            "None" => serialport::Parity::None,
            "Even" => serialport::Parity::Even,
            "Odd" => serialport::Parity::Odd,
            _ => return Err("Invalid parity".into()),
        })
        .timeout(std::time::Duration::from_millis(100));

    let port = builder.open().map_err(|e| format!("打开串口失败: {}", e))?;
    port.clear(serialport::ClearBuffer::All)
        .map_err(|e| format!("清空缓冲区失败: {}", e))?;

    let port_arc = Arc::new(Mutex::new(Some(port)));
    let running = Arc::new(AtomicBool::new(true));

    let port_clone = Arc::clone(&port_arc);
    let running_clone = Arc::clone(&running);
    let app_handle = app.clone();
    let event_name = format!("serial-data:{}", port_id);

    let handle = std::thread::spawn(move || {
        let mut buf: [u8; 1024] = [0; 1024];
        loop {
            if !running_clone.load(Ordering::Relaxed) {
                break;
            }
            {
                let mut port_guard = port_clone.lock().unwrap();
                if let Some(ref mut port) = *port_guard {
                    match port.read(&mut buf) {
                        Ok(n) if n > 0 => {
                            let data: Vec<u8> = buf[..n].to_vec();
                            let _ = app_handle.emit(&event_name, data);
                        }
                        _ => {}
                    }
                } else {
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    });

    let mut connections = state.connections.lock().map_err(|e| e.to_string())?;
    connections.insert(
        port_id,
        PortConnection {
            port: port_arc,
            running,
            read_thread: Mutex::new(Some(handle)),
        },
    );

    Ok(())
}

#[tauri::command]
fn close_serial(
    state: State<'_, SerialState>,
    port_id: String,
) -> Result<(), String> {
    // Stop any running queue for this port_id
    {
        let mut queues = state.queues.lock().map_err(|e| e.to_string())?;
        if let Some(qs) = queues.get(&port_id) {
            qs.running.store(false, Ordering::Relaxed);
            qs.paused.store(false, Ordering::Relaxed);
            if let Ok(mut handle_guard) = qs.thread_handle.lock() {
                if let Some(handle) = handle_guard.take() {
                    let _ = handle.join();
                }
            }
        }
        queues.remove(&port_id);
    }

    let entry = {
        let mut connections = state.connections.lock().map_err(|e| e.to_string())?;
        connections
            .remove(&port_id)
            .ok_or(format!("串口 {} 未找到", port_id))?
    };

    entry.running.store(false, Ordering::Relaxed);
    if let Ok(mut handle_guard) = entry.read_thread.lock() {
        if let Some(handle) = handle_guard.take() {
            let _ = handle.join();
        }
    }

    Ok(())
}

#[tauri::command]
fn send_serial(
    state: State<'_, SerialState>,
    port_id: String,
    data: Vec<u8>,
) -> Result<(), String> {
    let connections = state.connections.lock().map_err(|e| e.to_string())?;
    let entry = connections
        .get(&port_id)
        .ok_or("串口未打开")?;
    let mut port_guard = entry.port.lock().map_err(|e| e.to_string())?;
    if let Some(ref mut port) = *port_guard {
        port.write_all(&data)
            .map_err(|e| format!("发送失败: {}", e))?;
        port.flush()
            .map_err(|e| format!("刷新失败: {}", e))?;
        Ok(())
    } else {
        Err("串口未打开".into())
    }
}

fn send_serial_internal(port: &Arc<Mutex<Option<Box<dyn serialport::SerialPort>>>>, data: &[u8]) -> bool {
    if let Ok(mut guard) = port.lock() {
        if let Some(ref mut p) = *guard {
            return p.write_all(data).is_ok() && p.flush().is_ok();
        }
    }
    false
}

fn execute_queue(
    tasks: Vec<TaskItem>,
    port: Arc<Mutex<Option<Box<dyn serialport::SerialPort>>>>,
    running: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    app: AppHandle,
    port_id: String,
) {
    let mut pc: usize = 0;
    let mut loop_stack: Vec<(usize, u32)> = Vec::new();
    let task_count = tasks.len();

    while pc < task_count && running.load(Ordering::Relaxed) {
        while paused.load(Ordering::Relaxed) && running.load(Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        if !running.load(Ordering::Relaxed) {
            break;
        }

        let _ = app.emit(
            &format!("queue-update:{}", port_id),
            QueueStatusPayload {
                status: QueueStatus::Running,
                current_index: Some(pc),
            },
        );

        let task = &tasks[pc];
        match &task.task_type {
            TaskType::Send { data } => {
                if !send_serial_internal(&port, data) {
                    break;
                }
                pc += 1;
            }
            TaskType::Delay { ms } => {
                let step = 10u64;
                let mut remaining = *ms;
                while remaining > 0 && running.load(Ordering::Relaxed) {
                    while paused.load(Ordering::Relaxed) && running.load(Ordering::Relaxed) {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                    if !running.load(Ordering::Relaxed) {
                        break;
                    }
                    let chunk = std::cmp::min(step, remaining);
                    std::thread::sleep(std::time::Duration::from_millis(chunk));
                    remaining = remaining.saturating_sub(chunk);
                }
                pc += 1;
            }
            TaskType::Jump { target } => {
                if *target < task_count {
                    pc = *target;
                } else {
                    pc += 1;
                }
            }
            TaskType::LoopStart { count } => {
                loop_stack.push((pc, *count));
                pc += 1;
            }
            TaskType::LoopEnd => {
                if let Some((start_idx, remaining)) = loop_stack.last_mut() {
                    if *remaining > 1 {
                        *remaining -= 1;
                        pc = *start_idx + 1;
                    } else {
                        loop_stack.pop();
                        pc += 1;
                    }
                } else {
                    pc += 1;
                }
            }
            TaskType::Stop => {
                break;
            }
        }
    }

    let final_status = if running.load(Ordering::Relaxed) {
        QueueStatus::Idle
    } else {
        QueueStatus::Stopped
    };
    let _ = app.emit(
        &format!("queue-update:{}", port_id),
        QueueStatusPayload {
            status: final_status,
            current_index: None,
        },
    );
}

#[tauri::command]
fn queue_start(
    state: State<'_, SerialState>,
    app: AppHandle,
    port_id: String,
    tasks: Vec<TaskItem>,
) -> Result<(), String> {
    if tasks.is_empty() {
        return Err("任务列表为空".into());
    }

    let connections = state.connections.lock().map_err(|e| e.to_string())?;
    let entry = connections.get(&port_id).ok_or("串口未打开")?;
    let port = Arc::clone(&entry.port);
    drop(connections);

    let mut queues = state.queues.lock().map_err(|e| e.to_string())?;
    if let Some(existing) = queues.get(&port_id) {
        existing.running.store(false, Ordering::Relaxed);
        existing.paused.store(false, Ordering::Relaxed);
        if let Ok(mut handle_guard) = existing.thread_handle.lock() {
            if let Some(handle) = handle_guard.take() {
                let _ = handle.join();
            }
        }
    }

    let running = Arc::new(AtomicBool::new(true));
    let paused = Arc::new(AtomicBool::new(false));

    let running_clone = Arc::clone(&running);
    let paused_clone = Arc::clone(&paused);
    let app_clone = app.clone();
    let port_id_clone = port_id.clone();

    let handle = std::thread::spawn(move || {
        execute_queue(tasks, port, running_clone, paused_clone, app_clone, port_id_clone);
    });

    queues.insert(
        port_id,
        QueueState {
            running,
            paused,
            thread_handle: Mutex::new(Some(handle)),
        },
    );

    Ok(())
}

#[tauri::command]
fn queue_pause(
    state: State<'_, SerialState>,
    app: AppHandle,
    port_id: String,
) -> Result<(), String> {
    let queues = state.queues.lock().map_err(|e| e.to_string())?;
    let qs = queues.get(&port_id).ok_or("任务队列未运行")?;
    qs.paused.store(true, Ordering::Relaxed);
    let _ = app.emit(
        &format!("queue-update:{}", port_id),
        QueueStatusPayload {
            status: QueueStatus::Paused,
            current_index: None,
        },
    );
    Ok(())
}

#[tauri::command]
fn queue_resume(
    state: State<'_, SerialState>,
    app: AppHandle,
    port_id: String,
) -> Result<(), String> {
    let queues = state.queues.lock().map_err(|e| e.to_string())?;
    let qs = queues.get(&port_id).ok_or("任务队列未运行")?;
    qs.paused.store(false, Ordering::Relaxed);
    let _ = app.emit(
        &format!("queue-update:{}", port_id),
        QueueStatusPayload {
            status: QueueStatus::Running,
            current_index: None,
        },
    );
    Ok(())
}

#[tauri::command]
fn queue_stop(
    state: State<'_, SerialState>,
    app: AppHandle,
    port_id: String,
) -> Result<(), String> {
    let mut queues = state.queues.lock().map_err(|e| e.to_string())?;
    let qs = queues.get(&port_id).ok_or("任务队列未运行")?;
    qs.running.store(false, Ordering::Relaxed);
    qs.paused.store(false, Ordering::Relaxed);
    let _ = app.emit(
        &format!("queue-update:{}", port_id),
        QueueStatusPayload {
            status: QueueStatus::Stopped,
            current_index: None,
        },
    );
    if let Ok(mut handle_guard) = qs.thread_handle.lock() {
        if let Some(handle) = handle_guard.take() {
            let _ = handle.join();
        }
    }
    queues.remove(&port_id);
    Ok(())
}

#[tauri::command]
fn queue_get_state(
    state: State<'_, SerialState>,
    port_id: String,
) -> Result<serde_json::Value, String> {
    let queues = state.queues.lock().map_err(|e| e.to_string())?;
    match queues.get(&port_id) {
        Some(_qs) => Ok(serde_json::json!({
            "status": "Running",
        })),
        None => Ok(serde_json::json!({
            "status": "Idle",
        })),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            window.show().unwrap();
            Ok(())
        })
        .manage(SerialState {
            connections: Mutex::new(HashMap::new()),
            queues: Mutex::new(HashMap::new()),
        })
        .invoke_handler(tauri::generate_handler![
            get_serial_ports,
            open_serial,
            close_serial,
            send_serial,
            queue_start,
            queue_pause,
            queue_resume,
            queue_stop,
            queue_get_state,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
