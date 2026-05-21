# 串口助手 二次开发教程

## 项目概述

本项目是一个基于 **Tauri v2** + **Vue 3 + TypeScript** + **Tailwind CSS** 的跨平台串口调试助手，用于替代正点原子 PD42S1 步进电机官方上位机。支持 Linux / Windows / macOS。

- **前端**: Vue 3 (Composition API) + TypeScript + Tailwind CSS v3
- **后端**: Rust + Tauri v2 + serialport
- **通信**: Tauri IPC (`invoke` 命令调用 / `listen` 事件监听)

---

## 1. 环境搭建

### 1.1 硬件要求

| 硬件 | 说明 |
|------|------|
| 正点原子 PD42S1 步进电机驱动器 | 或兼容的自定义协议串口设备 |
| USB 转 TTL 串口模块 | CH340 / CP2102 / FT232 等 |
| 杜邦线 | 连接驱动器与串口模块 (GND、TXD、RXD) |
| 24V 直流电源 | 为驱动器供电 |

> **接线参考**: 驱动器端 TXD → 串口模块 RXD，驱动器端 RXD → 串口模块 TXD，驱动器端 GND → 串口模块 GND。通信波特率默认 115200。

### 1.2 软件依赖

#### 1.2.1 Node.js (v18+)

```bash
# 验证是否已安装
node -v    # 应显示 v18.x.x 或更高
npm -v     # 应显示 9.x.x 或更高

# 未安装则使用 nvm 安装 (推荐，支持版本切换)
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.7/install.sh | bash
# 重启终端或执行:
source ~/.bashrc
nvm install 18
nvm use 18
```

#### 1.2.2 Rust 工具链

```bash
# 验证是否已安装
rustc --version   # 应显示 1.70.0 或更高
cargo --version   # 应显示 1.70.0 或更高

# 未安装则执行:
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# 安装过程中选默认选项 (1) 即可
source ~/.cargo/env
```

#### 1.2.3 系统构建依赖

**Fedora**:
```bash
sudo dnf install webkit2gtk4.1-devel openssl-devel \
  libappindicator-gtk3-devel librsvg2-devel libudev-devel
```

**Ubuntu / Debian**:
```bash
sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev \
  librsvg2-dev libssl-dev libudev-dev
```

**Arch Linux**:
```bash
sudo pacman -S webkit2gtk-4.1 libappindicator-gtk3 librsvg openssl
```

**macOS**:
```bash
# 安装 Xcode Command Line Tools
xcode-select --install
```

**Windows**:
- [Visual Studio C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) (勾选 "C++ 桌面开发")
- [WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/) (Win10 通常已内置)

### 1.3 串口权限配置 (Linux)

Linux 下默认普通用户无权访问串口设备，必须配置权限：

```bash
# 将当前用户加入 dialout 组
sudo usermod -aG dialout $USER

# 将当前用户加入 tty 组 (部分发行版需要)
sudo usermod -aG tty $USER

# 重新登录或重启使权限生效
# 验证: groups 命令的输出中应包含 dialout
groups
```

**Fedora 额外步骤** — CH340 驱动 (常见于廉价串口模块):
```bash
# 检查内核是否已内置 ch341 驱动
lsmod | grep ch341

# 如果没有输出，安装驱动
sudo dnf install kernel-devel
# 驱动通常已内置，无需手动编译
```

**验证串口可用**:
```bash
# 插入 USB 转串口模块后
dmesg | tail -5               # 应看到 ttyUSB0 或 ttyACM0
ls /dev/ttyUSB* /dev/ttyACM*  # 列出串口设备
```

### 1.4 获取代码

```bash
# SSH 方式 (推荐，需配置 SSH Key)
git clone git@github.com:Galaxy-YLX/ATK_PD42S_V1.0.git

# 或 HTTPS 方式
git clone https://github.com/Galaxy-YLX/ATK_PD42S_V1.0.git

cd ATK_PD42S_V1.0
```

仓库包含三个分支，可根据需要切换：

| 分支 | 说明 |
|------|------|
| `PD42上位机` | 设备设置 + 运动控制面板 |
| `队列串口V1.1` | 带任务队列系统的版本 |
| `简易串口助手v1.0` | 基础串口收发功能 |

```bash
git branch -a         # 查看所有分支
git checkout 队列串口V1.1   # 切换到任务队列分支
```

### 1.5 安装与首次运行

```bash
cd serial-assistant

# 安装 npm 依赖 (仅首次)
npm install

# 启动开发模式 (首次编译 Rust 需要 5-15 分钟下载编译依赖)
npm run tauri dev
```

首次启动 Tauri 会自动下载 Rust 依赖并编译后端。如果过程缓慢，可配置 Cargo 镜像：

```bash
# ~/.cargo/config.toml
[source.crates-io]
replace-with = 'ustc'

[source.ustc]
registry = "https://mirrors.ustc.edu.cn/crates.io-index"
```

### 1.6 环境验证清单

完成以上步骤后，逐项检查：

| 检查项 | 命令 | 预期结果 |
|--------|------|----------|
| Node.js 版本 | `node -v` | v18+ |
| Rust 版本 | `rustc --version` | 1.70+ |
| 系统库 | `pkg-config --libs webkit2gtk-4.1` | 输出库路径 (Linux) |
| 串口权限 | `groups \| grep dialout` | 包含 dialout |
| 串口设备 | `ls /dev/ttyUSB*` | 列出串口 (插入模块后) |
| npm 依赖 | `cd serial-assistant && ls node_modules/.package-lock.json` | 文件存在 |
| Tauri 编译 | `cd serial-assistant && npm run tauri dev` | 窗口正常打开 |

### 1.7 环境常见问题

| 现象 | 原因 | 解决 |
|------|------|------|
| `Permission denied` 打开串口 | 不在 dialout 组 | `sudo usermod -aG dialout $USER` 后重新登录 |
| `/dev/ttyUSB0` 不存在 | 驱动未加载或模块未插入 | 检查 USB 连接，`dmesg` 查看内核日志 |
| 编译报 `webkit2gtk` not found | 缺少系统库 | 安装对应发行版的 webkit2gtk 包 (见 1.2.3) |
| `cc: command not found` | 缺少 C 编译器 | `sudo dnf install gcc` 或安装 Build Tools |
| `serialport` crate 编译失败 | 缺少 libudev | `sudo dnf install libudev-devel` |
| npm install 报 EACCES | npm 权限问题 | 使用 nvm 管理 Node.js，避免 sudo npm |
| 窗口白屏无内容 | Vite 未就绪或端口冲突 | 检查 1420 端口是否被占用: `lsof -i :1420` |

---

## 2. 项目结构

```
serial-assistant/
├── index.html                  # HTML 入口
├── package.json                # Node 依赖配置
├── vite.config.ts              # Vite 构建配置
├── tailwind.config.js          # Tailwind CSS 配置
├── tsconfig.json               # TypeScript 配置
├── src/                        # 前端源代码
│   ├── main.ts                 # Vue 入口
│   ├── style.css               # 全局样式 (Tailwind 指令)
│   ├── App.vue                 # 根组件 (多标签页框架)
│   └── components/
│       ├── SerialPanel.vue     # 串口面板 (配置/收发/历史)
│       └── TaskQueue.vue       # 任务队列面板
├── src-tauri/                  # Rust 后端源代码
│   ├── Cargo.toml              # Rust 依赖配置
│   ├── tauri.conf.json         # Tauri 应用配置
│   ├── capabilities/
│   │   └── default.json        # 权限声明
│   ├── icons/                  # 应用图标
│   └── src/
│       ├── main.rs             # Rust 入口
│       └── lib.rs              # 核心逻辑 (串口/队列/命令)
└── public/                     # 静态资源
```

### 核心文件职责

| 文件 | 职责 |
|------|------|
| `App.vue` | 多标签页框架、主题切换 |
| `SerialPanel.vue` | 串口配置、数据收发、发送历史 |
| `TaskQueue.vue` | 可视化任务队列编辑器 |
| `lib.rs` | 串口管理、任务队列执行引擎、Tauri 命令 |

---

## 3. 架构设计

### 3.1 前端与后端通信

```
┌──────────────────────┐       invoke('command', {args...})
│   Vue 前端           │ ─────────────────────────────────→
│   (TypeScript)       │ ←─────────────────────────────────
└──────────────────────┘       listen('event-name', callback)
                                     ↑
                                     │ Tauri IPC
                                     ↓
┌──────────────────────┐
│   Rust 后端          │
│   (Tauri Commands)   │
│   + SerialState      │
│   + 后台线程         │
└──────────────────────┘
```

**前端调用后端**: 使用 `invoke<T>(command_name, args)`，返回 Promise。
**后端推送前端**: 使用 `app.emit("event-name", payload)`，前端通过 `listen()` 接收。

### 3.2 状态模型 (Rust 侧)

```
SerialState                           // 应用全局状态
├── connections: HashMap<port_id, PortConnection>
│   └── PortConnection
│       ├── port: Arc<Mutex<Option<Box<dyn SerialPort>>>>
│       ├── running: Arc<AtomicBool>   // 读线程控制
│       └── read_thread: JoinHandle<()>
└── queues: HashMap<port_id, QueueState>
    └── QueueState
        ├── running: Arc<AtomicBool>   // 队列执行控制
        ├── paused: Arc<AtomicBool>    // 队列暂停控制
        └── thread_handle: JoinHandle<()>
```

每个标签页有一个唯一的 `port_id` (UUID)，通过它关联串口连接和任务队列。

### 3.3 任务队列引擎

任务队列是一个简单的字节码解释器：

```
TaskItem { index, task_type, label }
                │
                ▼
┌──────────────────────────────────────────┐
│ TaskType:                                │
│  Send     { data: Vec<u8> }              │ 发送数据
│  Delay    { ms: u64 }                    │ 延时等待
│  Jump     { target: usize }              │ 跳转到指定任务
│  LoopStart{ count: u32 }                 │ 循环开始
│  LoopEnd                                 │ 循环结束
│  Stop                                    │ 停止执行
└──────────────────────────────────────────┘

执行流程: pc++ → 取指令 → 执行 → 循环
循环实现: 遇到 LoopStart 将 (位置, 次数) 压栈
         遇到 LoopEnd 检查栈顶，次数>1 则跳回
```

---

## 4. 前端开发指南

### 4.1 添加新页面/面板

在 `App.vue` 的标签页内容区域 (由 `currentPage` 切换) 添加新页面：

```vue
<!-- App.vue template 中 -->
<div v-if="currentPage === '设备设置'">
  <!-- 现有内容 -->
</div>
<div v-if="currentPage === '运动控制'">
  <!-- 现有内容 -->
</div>
<!-- 新增页面 -->
<div v-if="currentPage === '数据可视化'">
  <DataChart :portId="activeTabId" />
</div>
```

然后在页面切换按钮区添加按钮：

```vue
<button
  v-for="page in ['设备设置', '运动控制', '数据可视化']"
  :key="page"
  @click="currentPage = page"
  ...
>{{ page }}</button>
```

### 4.2 创建新组件

```bash
touch src/components/DataChart.vue
```

组件模板：

```vue
<script setup lang="ts">
import { ref, onMounted, onUnmounted } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

const props = defineProps<{ portId: string }>();

let unlisten: UnlistenFn | null = null;

onMounted(async () => {
  unlisten = await listen<number[]>(`serial-data:${props.portId}`, (event) => {
    // 处理收到的串口数据
    console.log("收到数据:", event.payload);
  });
});

onUnmounted(() => {
  if (unlisten) unlisten();
});
</script>

<template>
  <div class="rounded-xl border border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900 p-4">
    <!-- 组件内容 -->
  </div>
</template>
```

### 4.3 调用后端命令

```typescript
import { invoke } from "@tauri-apps/api/core";

// 获取串口列表
const ports = await invoke<string[]>("get_serial_ports");

// 打开串口
await invoke("open_serial", {
  portId: props.portId,
  portName: "/dev/ttyUSB0",
  baudRate: 115200,
  dataBits: 8,
  stopBits: 1,
  parity: "None",
});

// 发送数据
await invoke("send_serial", {
  portId: props.portId,
  data: [0xC5, 0x01, 0xFA, 0x00, 0xFA, 0x5C],
});
```

### 4.4 监听后端事件

```typescript
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// 串口数据事件: serial-data:{portId}
const unlisten = await listen<number[]>(`serial-data:${portId}`, (event) => {
  console.log("收到字节:", event.payload); // number[]
});

// 队列状态事件: queue-update:{portId}
await listen<{ status: string; current?: number }>(
  `queue-update:${portId}`,
  (event) => {
    console.log("队列状态:", event.payload.status);
  }
);

// 组件卸载时取消监听
onUnmounted(() => { if (unlisten) unlisten(); });
```

### 4.5 Tailwind 样式规范

项目使用 Tailwind `class` 策略的暗色模式。所有需要适配暗色的元素加 `dark:` 前缀：

```html
<!-- 容器 -->
<div class="bg-white dark:bg-gray-900 border-gray-200 dark:border-gray-800">
<!-- 文字 -->
<span class="text-gray-700 dark:text-gray-200">
<!-- 按钮 -->
<button class="bg-blue-600 hover:bg-blue-700 dark:bg-blue-700 dark:hover:bg-blue-600 text-white">
```

---

## 5. 后端开发指南 (Rust)

### 5.1 添加新 Tauri 命令

在 `src-tauri/src/lib.rs` 中添加：

```rust
#[tauri::command]
fn my_new_command(
    state: tauri::State<'_, SerialState>,
    some_arg: String,
) -> Result<String, String> {
    // 你的逻辑
    Ok(format!("接收到的参数: {}", some_arg))
}
```

然后在 `run()` 函数的 `generate_handler!` 宏中注册：

```rust
.invoke_handler(tauri::generate_handler![
    // ... 已有命令
    my_new_command,
])
```

前端即可调用：

```typescript
const result = await invoke<string>("my_new_command", { someArg: "hello" });
```

### 5.2 访问串口

```rust
use serialport::{SerialPort};

// 在 PortConnection 中获取串口引用
let port = connection.port.lock().unwrap();
if let Some(ref mut serial) = *port {
    // 写入
    serial.write_all(&data).map_err(|e| e.to_string())?;
    serial.flush().map_err(|e| e.to_string())?;

    // 读取
    let mut buf = [0u8; 1024];
    match serial.read(&mut buf) {
        Ok(n) => { /* buf[..n] 是接收到的数据 */ }
        Err(ref e) if e.kind() == io::ErrorKind::TimedOut => { /* 超时，无数据 */ }
        Err(e) => { /* 其他错误 */ }
    }
}
```

### 5.3 后台线程模式

现有代码的标准模式 (参考 `open_serial` 中的读线程)：

```rust
let running = Arc::new(AtomicBool::new(true));
let running_clone = running.clone();
let port_clone = Arc::clone(&port);

let handle = thread::spawn(move || {
    while running_clone.load(Ordering::Relaxed) {
        // 执行循环工作
        // 例如读取串口数据、发送到前端
        thread::sleep(Duration::from_millis(10));
    }
});

// 停止线程
running.store(false, Ordering::Relaxed);
// handle.join() 等待线程结束
```

### 5.4 向前端推送事件

```rust
use tauri::Emitter;

// 在命令函数中，app_handle 通过参数获取
app_handle.emit("my-event", payload).map_err(|e| e.to_string())?;

// 发送到特定窗口
window.emit("my-event", payload).map_err(|e| e.to_string())?;
```

### 5.5 添加依赖

编辑 `src-tauri/Cargo.toml`：

```toml
[dependencies]
# 现有依赖...
my-crate = "1.0"                    # 纯 Rust crate
```

如果是需要系统库的 crate (如 `serialport`)，需确保开发环境已安装对应的系统依赖。

---

## 6. 常见开发任务

### 6.1 添加新的串口参数

1. **Rust 侧** (`lib.rs`): 在 `open_serial` 命令签名中添加参数，传递给 `serialport::new()` 的 builder。
2. **前端侧** (`SerialPanel.vue`): 添加对应的 UI 控件 (dropdown/input)，绑定到 `invoke` 调用。
3. **需要同时修改的类型**: `open_serial` 的参数列表和前端 `invoke` 的参数对象必须匹配 (参数名用 camelCase)。

### 6.2 添加新的任务类型

1. **Rust 侧** (`lib.rs`): 在 `TaskType` 枚举中添加新变体。
2. **队列执行逻辑**: 在 `run_queue` 函数的 match 分支中添加新类型的处理。
3. **前端侧** (`TaskQueue.vue`): 
   - 在任务类型按钮区添加新按钮
   - 在 `addTask` / `getDefaultTask` 中添加默认值
   - 添加对应的编辑面板

### 6.3 实现 PD42 自定义协议

PD42 使用帧格式：`[C5] [addr] [code] [data...] [checksum] [5C]`

校验和 = 帧头到数据区的所有字节之和的低 8 位。

当前项目已在前端 `App.vue` 中有 `customFrame()` 辅助函数，但未系统化。建议方案：

1. **创建协议模块** (`src/utils/protocol.ts`):
```typescript
// 帧构造
function buildFrame(addr: number, code: number, data: number[]): number[] {
  const head = [0xC5, addr, code, ...data];
  const sum = head.reduce((a, b) => a + b, 0);
  return [...head, sum & 0xFF, 0x5C];
}

// 帧解析
function parseFrame(bytes: number[]): { addr: number; code: number; data: number[] } | null {
  // 查找帧头 C5...帧尾 5C，验证校验和
}
```

2. **或在 Rust 侧实现**: 在 `lib.rs` 中添加 `send_pd42_command` 命令，封装协议细节。

### 6.4 修改应用配置

编辑 `src-tauri/tauri.conf.json`:

```json
{
  "app": {
    "windows": [{
      "title": "你的应用名",
      "width": 1024,
      "height": 768
    }]
  },
  "bundle": {
    "icon": ["icons/32x32.png", ...]
  }
}
```

---

## 7. 构建与发布

### 7.1 开发调试

```bash
npm run tauri dev    # 带热重载的开发模式
```

### 7.2 生产构建

```bash
npm run tauri build  # 构建可分发的安装包
```

构建产物在 `src-tauri/target/release/bundle/` 下：
- Linux: `.deb`, `.rpm`, `.AppImage`
- macOS: `.dmg`, `.app`
- Windows: `.msi`, `.exe`

### 7.3 仅构建前端 (调试用)

```bash
npm run build    # 构建前端到 dist/
npm run preview  # 预览构建结果 (纯前端，不含 Tauri)
```

---

## 8. 调试技巧

### 8.1 前端调试

在 `npm run tauri dev` 运行时，右键点击应用窗口 → **Inspect Element** 打开 Chrome DevTools。

或者在 `tauri.conf.json` 中临时设置窗口初始可见：
```json
{ "visible": true }
```

### 8.2 后端调试

使用 `println!()` 或 `eprintln!()` 输出到终端，开发模式下会显示在启动 Tauri 的终端中。

更好的方式：
```rust
// Cargo.toml
[dependencies]
log = "0.4"

// 代码中
log::info!("串口状态: {:?}", state);
```

### 8.3 常见问题

| 问题 | 原因 | 解决 |
|------|------|------|
| Vite 报 ENOENT 找不到组件 | Vite 缓存残留 | `rm -rf node_modules/.vite` |
| 编译报 linking `libwebkit2gtk` 失败 | 缺少系统库 | 安装 webkit2gtk4.1-devel |
| `serialport` 编译失败 | 缺少 libudev | `sudo dnf install libudev-devel` |
| 端口打不开 | 权限不足 | `sudo usermod -aG dialout $USER` 后重新登录 |
| Tauri 窗口不显示 | `visible: false` 阻塞 | 检查 `lib.rs` 的 `setup` 中是否调用了 `window.show()` |

---

## 9. 文档资源

- [Tauri v2 官方文档](https://v2.tauri.app/)
- [Vue 3 文档](https://cn.vuejs.org/)
- [Tailwind CSS 文档](https://tailwindcss.com/docs)
- [serialport crate](https://docs.rs/serialport/latest/serialport/)
- [正点原子 PD42S1 产品资料](https://www.openedv.com/docs/motors/step-motors/pd42s1.html)

---

> 本文档随项目持续更新。欢迎提交 Issue 或 PR 补充完善。
