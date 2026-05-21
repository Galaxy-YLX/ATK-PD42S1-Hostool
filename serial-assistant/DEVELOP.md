# 简易串口助手 二次开发教程

基于 **Tauri v2 + Vue 3 + TypeScript + Tailwind CSS** 的跨平台串口调试工具，支持多标签页同时连接多个串口。

---

## 1. 环境搭建

### 1.1 硬件

- USB 转 TTL 串口模块 (CH340 / CP2102 / FT232)
- 目标串口设备 (步进电机驱动器、单片机、传感器等)
- 杜邦线 (TXD-RXD 交叉，GND-GND)

### 1.2 软件依赖

**Node.js v18+**:
```bash
node -v    # 验证版本
# 未安装则用 nvm:
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.7/install.sh | bash
nvm install 18 && nvm use 18
```

**Rust**:
```bash
rustc --version    # 验证版本 (1.70+)
# 未安装:
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

**Linux 系统库**:
```bash
# Fedora
sudo dnf install webkit2gtk4.1-devel openssl-devel libappindicator-gtk3-devel librsvg2-devel libudev-devel
# Ubuntu/Debian
sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev libssl-dev libudev-dev
# Arch
sudo pacman -S webkit2gtk-4.1 libappindicator-gtk3 librsvg openssl
```

**macOS**: `xcode-select --install`

**Windows**: 安装 [Visual Studio C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) 和 [WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/)。

### 1.3 串口权限 (Linux)

```bash
sudo usermod -aG dialout $USER
# 重新登录后验证
groups | grep dialout
```

**验证设备**: 插入模块后 `dmesg | tail -5` 应看到 `ttyUSB0` 或 `ttyACM0`。

### 1.4 获取与运行

```bash
git clone git@github.com:Galaxy-YLX/ATK_PD42S_V1.0.git
cd ATK_PD42S_V1.0
git checkout 简易串口助手v1.0
cd serial-assistant
npm install
npm run tauri dev    # 首次编译 Rust 需 5-15 分钟
```

Cargo 下载慢可配镜像 (`~/.cargo/config.toml`):
```toml
[source.crates-io]
replace-with = 'ustc'
[source.ustc]
registry = "https://mirrors.ustc.edu.cn/crates.io-index"
```

### 1.5 环境问题速查

| 现象 | 解决 |
|------|------|
| 串口 Permission denied | `sudo usermod -aG dialout $USER` 后重新登录 |
| /dev/ttyUSB0 不存在 | 检查 USB 连接，`dmesg` 查看 |
| 编译报 webkit2gtk not found | 安装对应系统库 |
| npm install EACCES | 用 nvm 管理 Node，不用 sudo |
| serialport crate 编译失败 | `sudo dnf install libudev-devel` |
| 窗口白屏 | 检查 1420 端口: `lsof -i :1420` |

---

## 2. 项目结构

```
serial-assistant/
├── src/                        # 前端
│   ├── main.ts                 # Vue 入口
│   ├── App.vue                 # 多标签页框架 + 主题切换
│   ├── style.css               # Tailwind 指令
│   └── components/
│       └── SerialPanel.vue     # 串口配置/收发/历史
├── src-tauri/                  # Rust 后端
│   ├── Cargo.toml
│   ├── tauri.conf.json         # 窗口/打包配置
│   └── src/
│       ├── main.rs             # 入口
│       └── lib.rs              # 串口管理 + Tauri 命令
├── index.html
├── package.json
└── vite.config.ts
```

| 核心文件 | 职责 |
|----------|------|
| `App.vue` | 多标签页、暗色/亮色主题 |
| `SerialPanel.vue` | 串口参数配置、HEX/文本收发、发送历史 |
| `lib.rs` | 串口连接生命周期、后台读线程、Tauri IPC |

---

## 3. 架构

### 3.1 通信模型

```
Vue 前端                     Rust 后端
invoke('command', args) ───────→ Tauri Command
listen('event', cb)    ←─────── app.emit('event', payload)
```

### 3.2 Rust 状态

```
SerialState
└── connections: HashMap<port_id, PortConnection>
    └── PortConnection
        ├── port: Arc<Mutex<Option<Box<dyn SerialPort>>>>
        ├── running: Arc<AtomicBool>
        └── read_thread: JoinHandle<()>
```

每个标签页通过 UUID (`port_id`) 关联独立的串口连接。后台线程每 10ms 轮询，收到数据通过 `serial-data:{portId}` 事件推送前端。

---

## 4. 前端开发

### 4.1 添加 UI 控件

在 `SerialPanel.vue` 中添加：

```typescript
const mySetting = ref(false);

async function doSomething() {
  await invoke("send_serial", { portId: props.tabId, data: [0x01, 0x02] });
}
```

```html
<button @click="doSomething" :disabled="!isOpen" class="px-3 py-1 text-xs rounded-md ...">
  执行
</button>
```

### 4.2 创建新组件

```bash
touch src/components/MyPanel.vue
```

```vue
<script setup lang="ts">
import { onMounted, onUnmounted } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

const props = defineProps<{ portId: string }>();
let unlisten: UnlistenFn | null = null;

onMounted(async () => {
  unlisten = await listen<number[]>(`serial-data:${props.portId}`, (e) => {
    // e.payload 是收到的字节数组
  });
});
onUnmounted(() => unlisten?.());
</script>
```

在 `App.vue` 中引入：`import MyPanel from "./components/MyPanel.vue"`

### 4.3 IPC 速查

```typescript
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// 调用后端
const ports = await invoke<string[]>("get_serial_ports");
await invoke("open_serial", { portId, portName, baudRate, dataBits, stopBits, parity });
await invoke("send_serial", { portId, data: [0x48, 0x65] });
await invoke("close_serial", { portId });

// 监听事件
listen<number[]>(`serial-data:${portId}`, (e) => { /* e.payload */ });
```

参数名转换：前端 camelCase ↔ Rust snake_case (Tauri 自动转换)。

### 4.4 Tailwind 暗色模式

```html
<div class="bg-white dark:bg-gray-900 text-gray-900 dark:text-gray-100">
<button class="bg-blue-600 hover:bg-blue-700 dark:bg-blue-700 dark:hover:bg-blue-600 text-white">
```

---

## 5. 后端开发 (Rust)

### 5.1 添加命令

`lib.rs` 中：
```rust
#[tauri::command]
fn my_cmd(state: tauri::State<'_, SerialState>, arg: String) -> Result<String, String> {
    Ok(format!("收到: {}", arg))
}
```

`run()` 中注册：
```rust
.invoke_handler(tauri::generate_handler![my_cmd, /* 已有命令 */])
```

前端调用：
```typescript
await invoke<string>("my_cmd", { arg: "hello" });
```

### 5.2 串口操作

```rust
let port = connection.port.lock().unwrap();
if let Some(ref mut serial) = *port {
    serial.write_all(&data).map_err(|e| e.to_string())?;
    serial.flush().map_err(|e| e.to_string())?;

    let mut buf = [0u8; 1024];
    match serial.read(&mut buf) {
        Ok(n) => { /* buf[..n] */ }
        Err(ref e) if e.kind() == io::ErrorKind::TimedOut => { /* 无数据 */ }
        Err(e) => { /* 错误 */ }
    }
}
```

### 5.3 后台线程

```rust
let running = Arc::new(AtomicBool::new(true));
let running_c = running.clone();

let handle = thread::spawn(move || {
    while running_c.load(Ordering::Relaxed) {
        // 循环工作
        thread::sleep(Duration::from_millis(10));
    }
});

running.store(false, Ordering::Relaxed);  // 停止
```

### 5.4 推送事件

```rust
use tauri::Emitter;
app_handle.emit("event-name", payload).map_err(|e| e.to_string())?;
```

---

## 6. 构建与调试

### 6.1 命令

```bash
npm run tauri dev      # 开发 (热重载)
npm run tauri build    # 生产构建 → .deb/.rpm/.AppImage/.dmg/.msi
npm run build          # 仅构建前端
```

### 6.2 调试

- **前端**: 右键窗口 → Inspect Element (Chrome DevTools)
- **后端**: `eprintln!("{:?}", state);` 输出到启动终端
- **Vite 缓存**: `rm -rf node_modules/.vite` 清理残留

---

## 7. 参考资源

- [Tauri v2 文档](https://v2.tauri.app/)
- [Vue 3 文档](https://cn.vuejs.org/)
- [Tailwind CSS 文档](https://tailwindcss.com/docs)
- [serialport crate](https://docs.rs/serialport/latest/serialport/)
