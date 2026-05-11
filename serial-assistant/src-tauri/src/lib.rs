use std::collections::HashMap;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};

struct PortConnection {
    port: Arc<Mutex<Option<Box<dyn serialport::SerialPort>>>>,
    running: Arc<AtomicBool>,
    read_thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}

struct SerialState {
    connections: Mutex<HashMap<String, PortConnection>>,
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
        })
        .invoke_handler(tauri::generate_handler![
            get_serial_ports,
            open_serial,
            close_serial,
            send_serial
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
