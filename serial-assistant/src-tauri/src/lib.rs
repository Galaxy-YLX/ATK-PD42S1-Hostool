use tauri::command;
use serialport::available_ports;

// 定义一个 Tauri Command，供前端调用
#[command]
fn get_serial_ports() -> Result<Vec<String>, String> {
    // 调用 serialport 库获取可用端口
    match available_ports() {
        Ok(ports) => {
            // 提取端口名称 (Windows 下如 COM3，Linux 下如 /dev/ttyUSB0)
            let port_names: Vec<String> = ports.iter().map(|p| p.port_name.clone()).collect();
            Ok(port_names)
        }
        Err(e) => Err(format!("获取串口失败: {}", e)),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // 🌟 关键：把 Command 注册到这里，前端才能调用它
        .invoke_handler(tauri::generate_handler![get_serial_ports])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
