use std::io::Write;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

struct SerialState {
    port: Mutex<Option<Box<dyn serialport::SerialPort>>>,
    running: Mutex<bool>,
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
    port_name: String,
    baud_rate: u32,
    data_bits: u8,
    stop_bits: u8,
    parity: String,
) -> Result<(), String> {
    // Close any existing connection first
    let mut port_guard = state.port.lock().map_err(|e| e.to_string())?;
    *port_guard = None;
    *state.running.lock().map_err(|e| e.to_string())? = false;

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

    // Clear any stale data
    port.clear(serialport::ClearBuffer::All).map_err(|e| format!("清空缓冲区失败: {}", e))?;

    *port_guard = Some(port);
    *state.running.lock().map_err(|e| e.to_string())? = true;

    // Spawn a background thread to continuously read from the serial port
    let app_handle = app.clone();
    std::thread::spawn(move || {
        let mut buf: [u8; 1024] = [0; 1024];
        loop {
            let state = app_handle.state::<SerialState>();
            {
                let running = state.running.lock().unwrap();
                if !*running {
                    break;
                }
            }
            {
                let mut port_guard = state.port.lock().unwrap();
                if let Some(ref mut port) = *port_guard {
                    match port.read(&mut buf) {
                        Ok(n) if n > 0 => {
                            let data: Vec<u8> = buf[..n].to_vec();
                            let _ = app_handle.emit("serial-data", data);
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

    Ok(())
}

#[tauri::command]
fn close_serial(state: State<'_, SerialState>) -> Result<(), String> {
    let mut port_guard = state.port.lock().map_err(|e| e.to_string())?;
    *port_guard = None;
    *state.running.lock().map_err(|e| e.to_string())? = false;
    Ok(())
}

#[tauri::command]
fn send_serial(state: State<'_, SerialState>, data: Vec<u8>) -> Result<(), String> {
    let mut port_guard = state.port.lock().map_err(|e| e.to_string())?;
    if let Some(ref mut port) = *port_guard {
        port.write_all(&data).map_err(|e| format!("发送失败: {}", e))?;
        port.flush().map_err(|e| format!("刷新失败: {}", e))?;
        Ok(())
    } else {
        Err("串口未打开".into())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(SerialState {
            port: Mutex::new(None),
            running: Mutex::new(false),
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
