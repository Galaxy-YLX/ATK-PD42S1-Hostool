# 简易串口助手 二次开发教程

基于 **Tauri v2 + Vue 3 + TypeScript + Tailwind CSS** 的跨平台串口调试工具，支持多标签页同时连接多个串口，内置**可编程任务队列**实现串口自动化控制。

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
│       ├── SerialPanel.vue     # 串口配置/收发/历史
│       └── TaskQueue.vue       # 任务队列 — 串口自动化时序引擎
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
| `TaskQueue.vue` | 任务队列可视化编辑、拖拽排序、执行高亮 |
| `lib.rs` | 串口连接生命周期、后台读线程、任务队列执行引擎、Tauri IPC |

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
├── connections: HashMap<port_id, PortConnection>
│   └── PortConnection
│       ├── port: Arc<Mutex<Option<Box<dyn SerialPort>>>>
│       ├── running: Arc<AtomicBool>
│       └── read_thread: JoinHandle<()>
└── queues: HashMap<port_id, QueueState>
    └── QueueState
        ├── running: Arc<AtomicBool>
        ├── paused: Arc<AtomicBool>
        └── thread_handle: JoinHandle<()>
```

每个标签页通过 UUID (`port_id`) 关联独立的串口连接和任务队列。后台读线程每 10ms 轮询，收到数据通过 `serial-data:{portId}` 事件推送前端。任务队列在独立线程中执行，通过 `queue-update:{portId}` 事件推送执行进度。

---

## 4. 任务队列 — 核心特性

任务队列是本项目的核心功能：一个**可编程的串口自动化时序引擎**，通过组合 6 种指令类型实现对串口设备的批量自动化控制。适用于步进电机多段运动控制、传感器轮询采集、设备初始化时序等场景。

### 4.1 六种任务指令

| 指令 | 说明 | 参数 |
|------|------|------|
| **发送 (Send)** | 通过串口发送字节序列 | `data: number[]` — 原始字节数组 |
| **延时 (Delay)** | 等待指定毫秒数 | `ms: number` — 毫秒 |
| **跳转 (Jump)** | 无条件跳转到指定任务索引 | `target: number` — 目标任务序号 (0-based) |
| **循环开始 (LoopStart)** | 标记循环体开始，指定重复次数 | `count: number` — 循环次数 |
| **循环结束 (LoopEnd)** | 标记循环体结束，与 LoopStart 配对 | 无参数 |
| **停止 (Stop)** | 立即终止队列执行 | 无参数 |

这六种指令组合构成一套**简易脚本语言**：
- **顺序执行**：Send → Delay → Send → Delay → ...
- **循环**：LoopStart 和 LoopEnd 配对使用，支持嵌套
- **跳转**：Jump 实现任意流程跳转
- **终止**：Stop 在条件满足时提前退出

典型用例——步进电机 S 形加减速控制：
```
[0] 循环开始 ×3
[1]   发送     01 06 00 2A 00 05     # 设置加速度
[2]   发送     01 06 00 2B 00 64     # 设置速度 100rpm
[3]   发送     01 06 00 2C 07 D0     # 运行 2000 步
[4]   延时     2000 ms               # 等待执行完成
[5] 循环结束
[6] 停止
```

### 4.2 数据结构

**TypeScript 前端** (`TaskQueue.vue`):

```typescript
type TaskTypeValue =
  | { type: "Send";      value: { data: number[] } }
  | { type: "Delay";     value: { ms: number } }
  | { type: "Jump";      value: { target: number } }
  | { type: "LoopStart"; value: { count: number } }
  | { type: "LoopEnd" }
  | { type: "Stop" };

interface TaskItem {
  index: number;        // 自动分配序号
  task_type: TaskTypeValue;
  label: string;        // 用户自定义标签
}
```

**Rust 后端** (`lib.rs`)，使用 serde tagged-enum 序列化：

```rust
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum TaskType {
    Send { data: Vec<u8> },
    Delay { ms: u64 },
    Jump { target: usize },
    LoopStart { count: u32 },
    LoopEnd,
    Stop,
}

pub struct TaskItem {
    pub index: usize,
    pub task_type: TaskType,
    pub label: String,
}
```

前后端数据结构一一对应，Tauri IPC 自动完成 JSON 序列化/反序列化。

### 4.3 执行引擎

执行引擎位于 `lib.rs` 的 `execute_queue()` 函数中，运行在**独立 OS 线程**上，不阻塞 UI。

**核心模型**：程序计数器 (PC) + 循环栈 (Loop Stack)

```
pc = 0, loop_stack = []
while pc < task_count:
    emit queue-update 事件 (当前状态 + pc)
    task = tasks[pc]
    match task:
        Send     → write_all + flush, pc++
        Delay    → 分片 sleep (10ms), pc++
        Jump     → pc = target
        LoopStart → loop_stack.push((pc, count)), pc++
        LoopEnd  → 栈顶 remaining > 1 ? pc = start+1 : pop + pc++
        Stop     → break
emit queue-update 事件 (最终状态)
```

**并发控制**：两个 `Arc<AtomicBool>` 标志位
- `running`: 队列线程的运行开关，设为 `false` 即终止
- `paused`: 暂停标志，线程在检查点自旋等待（每 100ms 轮询）

**关键行为**:
- Delay 期间可响应暂停/停止（每 10ms 检查标志位）
- Jump 到越界目标时自动 pc++ 继续执行
- LoopEnd 无匹配 LoopStart 时当作普通 pc++ 处理
- Send 失败立即退出循环，停止执行

### 4.4 生命周期与串口关联

任务队列与串口连接**强绑定**：

```
打开串口 → 添加任务 → 启动队列 → 暂停/继续/停止 → 关闭串口（自动停止队列）
```

- **启动前提**：串口必须已打开（`isPortOpen === true`）且任务列表非空
- **关闭串口**：`close_serial` Rust 端自动停止并清理队列线程
- **前端切换标签页**：组件 `onUnmounted` 自动调用 `queue_stop`
- **`isPortOpen` 变化**：前端 watcher 检测到串口关闭时立即停止队列并重置状态

### 4.5 Tauri IPC 命令

| 命令 | 方向 | 参数 | 用途 |
|------|------|------|------|
| `queue_start` | 前端→后端 | `portId, tasks: TaskItem[]` | 传入完整任务列表，启动执行线程 |
| `queue_pause` | 前端→后端 | `portId` | 设置暂停标志，线程自旋等待 |
| `queue_resume` | 前端→后端 | `portId` | 清除暂停标志，线程继续执行 |
| `queue_stop` | 前端→后端 | `portId` | 终止线程，JoinHandle join，清理状态 |
| `queue_get_state` | 前端→后端 | `portId` | 前端挂载时同步当前队列状态 |

**事件推送**（后端→前端）：

| 事件 | 负载 | 用途 |
|------|------|------|
| `queue-update:{portId}` | `{ status, current_index }` | 每步执行后推送状态+当前索引，前端据此高亮当前任务 |

### 4.6 前端 UI

`TaskQueue.vue` 组件接收 `tabId` 和 `isPortOpen` 两个 props，嵌入在 `SerialPanel.vue` 中。

**布局**：左侧添加任务按钮栏 + 右侧任务列表 + 顶部控制栏

- **状态栏**：显示当前状态（空闲/运行中/已暂停/已停止）+ 进度（"任务 3/8"）+ 当前执行箭头指示
- **添加任务**：6 个类型按钮 + "清空全部"按钮（仅空闲时可用）
- **控制按钮**：启动（需串口已开且任务非空）/ 暂停 / 继续 / 停止
- **任务列表**：拖拽排序、行内编辑、上移/下移/删除
- **行内编辑**：
  - Send：HEX/文本模式切换 + 输入框
  - Delay：数字输入 + ±10ms 微调按钮
  - Jump：数字输入 + ±1 微调按钮
  - LoopStart：次数输入 + ±1 微调按钮
- **执行高亮**：当前正在执行的任务行有脉冲箭头动画

### 4.7 扩展指南

**添加新任务类型**需同时修改三处：

1. **TypeScript 类型** (`TaskQueue.vue` lines 13-19)：在 `TaskTypeValue` 联合类型中新增
2. **Rust 枚举** (`lib.rs` lines 8-17)：在 `TaskType` 枚举中新增变体
3. **执行引擎** (`lib.rs` `execute_queue()` 的 match 分支)：实现新指令的逻辑

前端 UI 的行内编辑器也需对应扩展（`editingType`、`editingValue` 的绑定和处理）。

---

## 5. 前端开发

### 5.1 添加 UI 控件

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

### 5.2 创建新组件

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

### 5.3 IPC 速查

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

### 5.4 Tailwind 暗色模式

```html
<div class="bg-white dark:bg-gray-900 text-gray-900 dark:text-gray-100">
<button class="bg-blue-600 hover:bg-blue-700 dark:bg-blue-700 dark:hover:bg-blue-600 text-white">
```

---

## 6. 后端开发 (Rust)

### 6.1 添加命令

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

### 6.2 串口操作

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

### 6.3 后台线程

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

### 6.4 推送事件

```rust
use tauri::Emitter;
app_handle.emit("event-name", payload).map_err(|e| e.to_string())?;
```

---

## 7. 构建与调试

### 7.1 命令

```bash
npm run tauri dev      # 开发 (热重载)
npm run tauri build    # 生产构建 → .deb/.rpm/.AppImage/.dmg/.msi
npm run build          # 仅构建前端
```

### 7.2 调试

- **前端**: 右键窗口 → Inspect Element (Chrome DevTools)
- **后端**: `eprintln!("{:?}", state);` 输出到启动终端
- **Vite 缓存**: `rm -rf node_modules/.vite` 清理残留

---

## 8. 参考资源

- [Tauri v2 文档](https://v2.tauri.app/)
- [Vue 3 文档](https://cn.vuejs.org/)
- [Tailwind CSS 文档](https://tailwindcss.com/docs)
- [serialport crate](https://docs.rs/serialport/latest/serialport/)
