<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

const theme = ref<"dark" | "light">("dark");

function toggleTheme() {
  theme.value = theme.value === "dark" ? "light" : "dark";
}

watch(theme, (t) => {
  document.documentElement.classList.toggle("dark", t === "dark");
}, { immediate: true });

// ---- 串口配置 ----
const ports = ref<string[]>([]);
const selectedPort = ref("");
const baudRate = ref(115200);
const isOpen = ref(false);
const errorMsg = ref("");

const baudRates = [9600, 14400, 19200, 38400, 57600, 115200, 230400, 460800, 921600];

// ---- 数据显示 ----
interface DataEntry { dir: "send" | "recv"; data: number[] }
const displayMode = ref<"text" | "hex">("hex");
const dataEntries = ref<DataEntry[]>([]);
const dataViewer = ref<HTMLDivElement>();

function recordSend(bytes: number[]) {
  dataEntries.value.push({ dir: "send", data: bytes });
  if (dataEntries.value.length > 100000) {
    dataEntries.value = dataEntries.value.slice(-50000);
  }
}

function recordRecv(bytes: number[]) {
  dataEntries.value.push({ dir: "recv", data: bytes });
  if (dataEntries.value.length > 100000) {
    dataEntries.value = dataEntries.value.slice(-50000);
  }
}

function totalBytes(): number {
  return dataEntries.value.reduce((sum, e) => sum + e.data.length, 0);
}

// ---- 发送 ----
const sendMode = ref<"text" | "hex">("hex");
const sendData = ref("");

// ---- 页面切换 ----
const currentPage = ref("设备设置");

// ---- 协议指令 ----
const motorAddr = ref(1);
watch(motorAddr, (v) => {
  if (v < 0) motorAddr.value = 0;
  if (v > 255) motorAddr.value = 255;
});

// ---- 系统参数 ----
const pendingSysRead = ref(false);
const sysParams = ref({
  busVoltage:      "--",
  phaseCurrent:    "--",
  fluxLinkage:     "--",
  phaseResistance: "--",
  phaseInductance: "--",
  realSpeed:       "--",
  targetPos:       "--",
  realPos:         "--",
  posError:        "--",
  pulseCount:      "--",
  enableFlag:      "--",
  inPosFlag:       "--",
  stallFlag:       "--",
});

// 自定义协议: [C5] [addr] [code] [data...] [checksum] [5C]
function customFrame(addr: number, code: number, data: number[] = []): number[] {
  const head = [0xC5, addr, code, ...data];
  let sum = 0;
  for (const b of head) sum += b;
  return [...head, sum & 0xFF, 0x5C];
}

function sendSysReadCmd() {
  const frame = customFrame(motorAddr.value, 0x31);
  pendingSysRead.value = true;
  invoke("send_serial", { data: frame }).then(() => {
    recordSend(frame);
  }).catch((e) => {
    errorMsg.value = e as string;
  });
}

function parseFloat32BE(bytes: number[], off: number): number {
  const buf = new ArrayBuffer(4);
  const dv = new DataView(buf);
  dv.setUint8(0, bytes[off]);
  dv.setUint8(1, bytes[off+1]);
  dv.setUint8(2, bytes[off+2]);
  dv.setUint8(3, bytes[off+3]);
  return dv.getFloat32(0);
}

function parseInt32BE(bytes: number[], off: number): number {
  return ((bytes[off] << 24) | (bytes[off+1] << 16) | (bytes[off+2] << 8) | bytes[off+3]) >>> 0;
}

function parseInt16BE(bytes: number[], off: number): number {
  return ((bytes[off] << 8) | bytes[off+1]) >>> 0;
}

function parseSysParams(raw: number[]): boolean {
  // 响应: [C5] [addr] [31] [err] [40 bytes payload] [checksum] [5C] = 46 bytes
  // 查找 C5 xx 31 01 的位置（可能前面有指令回响）
  let off = -1;
  for (let i = 0; i <= raw.length - 46; i++) {
    if (raw[i] === 0xC5 && raw[i+2] === 0x31 && raw[i+3] === 0x01) {
      off = i; break;
    }
  }
  if (off < 0) return false;
  const p = raw.slice(off + 4, off + 44);

  sysParams.value = {
    busVoltage:      parseFloat32BE(p, 0).toFixed(2) + " V",
    phaseCurrent:    parseInt16BE(p, 4) + " mA",
    fluxLinkage:     parseFloat32BE(p, 6).toFixed(3) + " mWb",
    phaseResistance: parseFloat32BE(p, 10).toFixed(3) + " Ω",
    phaseInductance: parseFloat32BE(p, 14).toFixed(3) + " mH",
    realSpeed:       parseInt16BE(p, 18) + " RPM",
    targetPos:       parseInt32BE(p, 20).toString(),
    realPos:         parseInt32BE(p, 24).toString(),
    posError:        parseInt32BE(p, 28).toString(),
    pulseCount:      parseInt32BE(p, 32).toString(),
    enableFlag:      p[36] === 0 ? "已使能" : "未使能",
    inPosFlag:       p[38] === 1 ? "已到位" : "未到位",
    stallFlag:       p[37] === 1 ? "已堵转" : "未堵转",
  };
  return true;
}

let unlisten: UnlistenFn | null = null;
let unlistenPorts: UnlistenFn | null = null;

// ---- 串口操作 ----
async function fetchPorts(silent = false) {
  try {
    ports.value = await invoke<string[]>("get_serial_ports");
    if (ports.value.length > 0 && !selectedPort.value) {
      selectedPort.value = ports.value[0];
    }
    errorMsg.value = "";
  } catch (error) {
    if (!silent) errorMsg.value = error as string;
  }
}

async function toggleSerial() {
  if (isOpen.value) {
    await closeSerial();
  } else {
    await openSerial();
  }
}

async function openSerial() {
  if (!selectedPort.value) {
    errorMsg.value = "请选择串口";
    return;
  }
  try {
    await invoke("open_serial", {
      portName: selectedPort.value,
      baudRate: baudRate.value,
      dataBits: 8,
      stopBits: 1,
      parity: "None",
    });
    isOpen.value = true;
    errorMsg.value = "";
    dataEntries.value = [];
  } catch (error) {
    errorMsg.value = error as string;
  }
}

async function closeSerial() {
  try {
    await invoke("close_serial");
    isOpen.value = false;
  } catch (error) {
    errorMsg.value = error as string;
  }
}

// ---- 发送 ----
async function send() {
  if (!sendData.value) return;
  let data: number[];
  if (sendMode.value === "hex") {
    const hex = sendData.value.replace(/\s+/g, "");
    if (hex.length % 2 !== 0 || !/^[0-9a-fA-F]*$/.test(hex)) {
      errorMsg.value = "HEX 格式错误";
      return;
    }
    data = [];
    for (let i = 0; i < hex.length; i += 2) {
      data.push(parseInt(hex.substring(i, i + 2), 16));
    }
  } else {
    const encoder = new TextEncoder();
    data = Array.from(encoder.encode(sendData.value));
  }
  try {
    await invoke("send_serial", { data });
    recordSend(data);
  } catch (error) {
    errorMsg.value = error as string;
  }
}

// ---- 数据显示格式化 ----
function bytesToHex(bytes: number[]): string {
  return bytes.map(b => b.toString(16).toUpperCase().padStart(2, "0")).join(" ");
}

function bytesToText(bytes: number[]): string {
  return bytes.map(b => {
    if (b >= 0x20 && b < 0x7f) return String.fromCharCode(b);
    if (b === 0x0a) return "\n";
    if (b === 0x0d) return "\r";
    if (b === 0x09) return "\t";
    return ".";
  }).join("");
}

function formatEntry(entry: DataEntry): string {
  const prefix = entry.dir === "send" ? "→ " : "← ";
  return displayMode.value === "hex"
    ? prefix + bytesToHex(entry.data)
    : prefix + bytesToText(entry.data);
}

function clearData() {
  dataEntries.value = [];
}

function scrollToBottom() {
  nextTick(() => {
    if (dataViewer.value) {
      dataViewer.value.scrollTop = dataViewer.value.scrollHeight;
    }
  });
}

// ---- 生命周期 ----
onMounted(async () => {
  await fetchPorts(true);
  unlisten = await listen<number[]>("serial-data", (event) => {
    if (pendingSysRead.value && parseSysParams(event.payload)) {
      pendingSysRead.value = false;
    }
    recordRecv(event.payload);
    scrollToBottom();
  });
  unlistenPorts = await listen<string[]>("ports-changed", (event) => {
    const newPorts = event.payload;
    ports.value = newPorts;
    if (!isOpen.value) {
      if (!selectedPort.value || !newPorts.includes(selectedPort.value)) {
        selectedPort.value = newPorts.length > 0 ? newPorts[0] : "";
      }
    }
  });
});

onUnmounted(() => {
  if (unlisten) unlisten();
  if (unlistenPorts) unlistenPorts();
  if (isOpen.value) {
    invoke("close_serial");
  }
});
</script>

<template>
  <div class="min-h-screen bg-gray-100 dark:bg-gray-950 p-5 font-sans text-gray-900 dark:text-gray-100 transition-colors">
    <div class="max-w-6xl mx-auto">

      <!-- 标题栏 -->
      <div class="flex items-center justify-between mb-4">
        <h1 class="text-xl font-semibold tracking-tight">正点原子步进电机驱动器上位机 V2.0</h1>
        <button
          @click="toggleTheme"
          class="px-3 py-1.5 text-xs font-medium rounded-md border border-gray-300 dark:border-gray-700 bg-white dark:bg-gray-900 text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors"
        >
          {{ theme === "dark" ? "☀ 白天模式" : "☾ 黑夜模式" }}
        </button>
      </div>

      <!-- 错误提示 -->
      <div
        v-if="errorMsg"
        class="flex items-center justify-between px-4 py-2.5 mb-3 rounded-lg border border-red-300 dark:border-red-800 bg-red-50 dark:bg-red-950 text-red-700 dark:text-red-400 text-sm"
      >
        <span>{{ errorMsg }}</span>
        <button @click="errorMsg = ''" class="ml-3 text-red-400 hover:text-red-600 dark:hover:text-red-300 text-base leading-none">&times;</button>
      </div>

      <!-- 串口配置栏 -->
      <div class="flex flex-wrap items-end gap-3 p-4 mb-3 rounded-xl border border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900">
        <div class="flex flex-col gap-1">
          <label class="text-[11px] font-medium uppercase tracking-wider text-gray-500 dark:text-gray-500">串口</label>
          <div class="flex gap-1.5">
            <select v-model="selectedPort" :disabled="isOpen"
              class="select-arrow pr-7 pl-2.5 py-1.5 text-sm rounded-md border border-gray-300 dark:border-gray-700 bg-gray-50 dark:bg-gray-800 text-gray-900 dark:text-gray-100 disabled:opacity-40 focus:border-blue-500 dark:focus:border-blue-400 outline-none min-w-[130px]">
              <option disabled value="">请选择串口</option>
              <option v-for="port in ports" :key="port" :value="port">{{ port }}</option>
            </select>
            <button @click="fetchPorts()" :disabled="isOpen"
              class="px-3 py-1.5 text-xs font-medium rounded-md border border-gray-300 dark:border-gray-700 bg-gray-50 dark:bg-gray-800 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 disabled:opacity-40 transition-colors">
              刷新
            </button>
          </div>
        </div>

        <div class="flex flex-col gap-1">
          <label class="text-[11px] font-medium uppercase tracking-wider text-gray-500 dark:text-gray-500">波特率</label>
          <select v-model="baudRate" :disabled="isOpen"
            class="select-arrow pr-7 pl-2.5 py-1.5 text-sm rounded-md border border-gray-300 dark:border-gray-700 bg-gray-50 dark:bg-gray-800 text-gray-900 dark:text-gray-100 disabled:opacity-40 focus:border-blue-500 dark:focus:border-blue-400 outline-none min-w-[100px]">
            <option v-for="b in baudRates" :key="b" :value="b">{{ b }}</option>
          </select>
        </div>

        <button @click="toggleSerial"
          :class="isOpen
            ? 'bg-red-600 hover:bg-red-700 dark:bg-red-700 dark:hover:bg-red-600'
            : 'bg-emerald-600 hover:bg-emerald-700 dark:bg-emerald-700 dark:hover:bg-emerald-600'"
          class="ml-auto px-5 py-1.5 text-sm font-semibold text-white rounded-md transition-colors">
          {{ isOpen ? "关闭串口" : "打开串口" }}
        </button>

        <span class="text-xs font-medium" :class="isOpen ? 'text-emerald-600 dark:text-emerald-400' : 'text-gray-400 dark:text-gray-600'">
          {{ isOpen ? "● 已连接" : "○ 未连接" }}
        </span>
      </div>

      <!-- 主内容区：两栏 -->
      <div class="flex gap-3">
        <!-- 左侧：页面切换 + 功能面板 -->
        <div class="w-[340px] shrink-0 flex flex-col gap-3">
          <!-- 页面切换栏 -->
          <div class="flex rounded-xl border border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900 overflow-hidden">
            <button
              v-for="page in ['设备设置', '运动控制']"
              :key="page"
              @click="currentPage = page"
              :class="currentPage === page
                ? 'bg-blue-50 dark:bg-blue-950 text-blue-700 dark:text-blue-300 border-blue-300 dark:border-blue-700'
                : 'text-gray-500 dark:text-gray-500 border-transparent hover:text-gray-700 dark:hover:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-800'"
              class="flex-1 px-3 py-2 text-xs font-medium border-b-2 transition-colors"
            >{{ page }}</button>
          </div>

          <!-- 设备设置页 -->
          <div v-if="currentPage === '设备设置'" class="rounded-xl border border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900 overflow-hidden">
            <div class="px-4 py-2.5 border-b border-gray-100 dark:border-gray-800 bg-gray-50 dark:bg-gray-900">
              <div class="flex items-center justify-between">
                <span class="text-sm font-semibold text-gray-700 dark:text-gray-200">设备设置</span>
                <div class="flex items-center gap-1.5">
                  <label class="text-[10px] font-medium uppercase tracking-wider text-gray-500 dark:text-gray-500">电机地址</label>
                  <div class="flex items-stretch">
                    <input
                      v-model.number="motorAddr"
                      type="number" min="0" max="255"
                      :disabled="isOpen"
                      class="no-spin w-14 px-1 py-1 text-xs text-center rounded-l-md border border-blue-400 dark:border-blue-500 bg-blue-50 dark:bg-blue-950 text-blue-700 dark:text-blue-300 outline-none font-mono"
                      title="电机地址 0x00-0xFF" />
                    <button @click="motorAddr = Math.min(255, motorAddr + 1)"
                      class="px-2 text-sm font-medium border-y border-blue-400 dark:border-blue-500 bg-blue-50 dark:bg-blue-950 text-blue-600 dark:text-blue-400 hover:bg-blue-100 dark:hover:bg-blue-900 transition-colors">+</button>
                    <button @click="motorAddr = Math.max(0, motorAddr - 1)"
                      class="px-2 text-sm font-medium rounded-r-md border-y border-r border-blue-400 dark:border-blue-500 bg-blue-50 dark:bg-blue-950 text-blue-600 dark:text-blue-400 hover:bg-blue-100 dark:hover:bg-blue-900 transition-colors">−</button>
                  </div>
                </div>
              </div>
            </div>
            <div class="px-3 pb-3">
            </div>
          </div>

          <!-- 运动控制页 -->
          <div v-if="currentPage === '运动控制'" class="rounded-xl border border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900 overflow-hidden">
            <div class="px-4 py-2.5 border-b border-gray-100 dark:border-gray-800 bg-gray-50 dark:bg-gray-900">
              <div class="flex items-center justify-between">
                <span class="text-sm font-semibold text-gray-700 dark:text-gray-200">运动控制</span>
                <div class="flex items-center gap-1.5">
                  <label class="text-[10px] font-medium uppercase tracking-wider text-gray-500 dark:text-gray-500">电机地址</label>
                  <div class="flex items-stretch">
                    <input v-model.number="motorAddr" type="number" min="0" max="255"
                      class="no-spin w-14 px-1 py-1 text-xs text-center rounded-l-md border border-blue-400 dark:border-blue-500 bg-blue-50 dark:bg-blue-950 text-blue-700 dark:text-blue-300 outline-none font-mono"
                      title="电机地址 0x00-0xFF" />
                    <button @click="motorAddr = Math.min(255, motorAddr + 1)"
                      class="px-2 text-sm font-medium border-y border-blue-400 dark:border-blue-500 bg-blue-50 dark:bg-blue-950 text-blue-600 dark:text-blue-400 hover:bg-blue-100 dark:hover:bg-blue-900 transition-colors">+</button>
                    <button @click="motorAddr = Math.max(0, motorAddr - 1)"
                      class="px-2 text-sm font-medium rounded-r-md border-y border-r border-blue-400 dark:border-blue-500 bg-blue-50 dark:bg-blue-950 text-blue-600 dark:text-blue-400 hover:bg-blue-100 dark:hover:bg-blue-900 transition-colors">−</button>
                  </div>
                </div>
              </div>
            </div>
            <div class="p-3 space-y-3 text-xs text-gray-500 dark:text-gray-400">
              <div>运动控制页面（待实现）</div>
            </div>
          </div>

          <!-- 系统参数（始终在运动控制下方） -->
          <div class="rounded-xl border border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900 overflow-hidden">
            <div class="flex items-center justify-between px-4 py-2.5 border-b border-gray-100 dark:border-gray-800 bg-gray-50 dark:bg-gray-900">
              <span class="text-sm font-semibold text-gray-700 dark:text-gray-200">系统参数</span>
              <button @click="sendSysReadCmd" :disabled="!isOpen"
                class="px-2.5 py-1 text-xs font-medium text-white bg-blue-600 hover:bg-blue-700 dark:bg-blue-700 dark:hover:bg-blue-600 rounded-md disabled:bg-gray-200 dark:disabled:bg-gray-800 disabled:text-gray-400 dark:disabled:text-gray-600 transition-colors">
                读取系统参数
              </button>
            </div>
            <div class="p-3 grid grid-cols-2 gap-x-3 gap-y-1.5 text-xs">
              <div class="flex justify-between"><span class="text-gray-500 dark:text-gray-400">总线电压</span><span class="font-mono text-gray-700 dark:text-gray-200">{{ sysParams.busVoltage }}</span></div>
              <div class="flex justify-between"><span class="text-gray-500 dark:text-gray-400">相电流</span><span class="font-mono text-gray-700 dark:text-gray-200">{{ sysParams.phaseCurrent }}</span></div>
              <div class="flex justify-between"><span class="text-gray-500 dark:text-gray-400">电机磁链</span><span class="font-mono text-gray-700 dark:text-gray-200">{{ sysParams.fluxLinkage }}</span></div>
              <div class="flex justify-between"><span class="text-gray-500 dark:text-gray-400">相电阻</span><span class="font-mono text-gray-700 dark:text-gray-200">{{ sysParams.phaseResistance }}</span></div>
              <div class="flex justify-between"><span class="text-gray-500 dark:text-gray-400">相电感</span><span class="font-mono text-gray-700 dark:text-gray-200">{{ sysParams.phaseInductance }}</span></div>
              <div class="flex justify-between"><span class="text-gray-500 dark:text-gray-400">实时转速</span><span class="font-mono text-gray-700 dark:text-gray-200">{{ sysParams.realSpeed }}</span></div>
              <div class="flex justify-between col-span-2 border-t border-gray-100 dark:border-gray-800 pt-1.5 mt-0.5"></div>
              <div class="flex justify-between"><span class="text-gray-500 dark:text-gray-400">目标位置</span><span class="font-mono text-gray-700 dark:text-gray-200">{{ sysParams.targetPos }}</span></div>
              <div class="flex justify-between"><span class="text-gray-500 dark:text-gray-400">实时位置</span><span class="font-mono text-gray-700 dark:text-gray-200">{{ sysParams.realPos }}</span></div>
              <div class="flex justify-between"><span class="text-gray-500 dark:text-gray-400">位置误差</span><span class="font-mono text-gray-700 dark:text-gray-200">{{ sysParams.posError }}</span></div>
              <div class="flex justify-between"><span class="text-gray-500 dark:text-gray-400">累计脉冲</span><span class="font-mono text-gray-700 dark:text-gray-200">{{ sysParams.pulseCount }}</span></div>
              <div class="flex justify-between"><span class="text-gray-500 dark:text-gray-400">使能标志</span><span :class="sysParams.enableFlag === '已使能' ? 'text-emerald-600 dark:text-emerald-400' : 'text-red-500 dark:text-red-400'" class="font-mono">{{ sysParams.enableFlag }}</span></div>
              <div class="flex justify-between"><span class="text-gray-500 dark:text-gray-400">到位标志</span><span :class="sysParams.inPosFlag === '已到位' ? 'text-emerald-600 dark:text-emerald-400' : 'text-orange-500 dark:text-orange-400'" class="font-mono">{{ sysParams.inPosFlag }}</span></div>
              <div class="flex justify-between"><span class="text-gray-500 dark:text-gray-400">堵转情况</span><span :class="sysParams.stallFlag === '已堵转' ? 'text-red-500 dark:text-red-400' : 'text-emerald-600 dark:text-emerald-400'" class="font-mono">{{ sysParams.stallFlag }}</span></div>
            </div>
          </div>
        </div>

        <!-- 右侧：接收数据 + 发送 -->
        <div class="flex-1 min-w-0 flex flex-col gap-3">
          <div class="rounded-xl border border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900 overflow-hidden">
            <div class="flex items-center justify-between px-4 py-2.5 border-b border-gray-100 dark:border-gray-800 bg-gray-50 dark:bg-gray-900">
              <span class="text-sm font-semibold text-gray-700 dark:text-gray-200">数据收发</span>
              <div class="flex items-center gap-1.5">
                <span class="text-[11px] text-gray-400 dark:text-gray-600 mr-2 tabular-nums">{{ totalBytes() }} 字节</span>
                <button
                  :class="displayMode === 'text' ? 'bg-blue-50 dark:bg-blue-950 border-blue-300 dark:border-blue-700 text-blue-700 dark:text-blue-300' : 'border-gray-200 dark:border-gray-700 text-gray-600 dark:text-gray-400 hover:bg-gray-50 dark:hover:bg-gray-800'"
                  @click="displayMode = 'text'"
                  class="px-2.5 py-1 text-xs font-medium rounded border transition-colors">
                  文本
                </button>
                <button
                  :class="displayMode === 'hex' ? 'bg-blue-50 dark:bg-blue-950 border-blue-300 dark:border-blue-700 text-blue-700 dark:text-blue-300' : 'border-gray-200 dark:border-gray-700 text-gray-600 dark:text-gray-400 hover:bg-gray-50 dark:hover:bg-gray-800'"
                  @click="displayMode = 'hex'"
                  class="px-2.5 py-1 text-xs font-medium rounded border transition-colors">
                  HEX
                </button>
                <button @click="clearData"
                  class="px-2.5 py-1 text-xs font-medium rounded border border-gray-200 dark:border-gray-700 text-gray-600 dark:text-gray-400 hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors">
                  清空
                </button>
              </div>
            </div>
            <div ref="dataViewer" class="h-96 overflow-y-auto p-4 bg-gray-50 dark:bg-gray-950 font-mono text-sm leading-relaxed">
              <template v-if="dataEntries.length > 0">
                <div
                  v-for="(entry, i) in dataEntries"
                  :key="i"
                  :class="entry.dir === 'send' ? 'text-emerald-700 dark:text-emerald-400' : 'text-sky-700 dark:text-sky-400'"
                  class="whitespace-pre-wrap break-all"
                >{{ formatEntry(entry) }}</div>
              </template>
              <div v-else class="text-gray-300 dark:text-gray-700 font-sans text-sm">等待数据...</div>
            </div>
          </div>

          <div class="rounded-xl border border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900 overflow-hidden">
            <div class="flex items-center justify-between px-4 py-2.5 border-b border-gray-100 dark:border-gray-800 bg-gray-50 dark:bg-gray-900">
              <span class="text-sm font-semibold text-gray-700 dark:text-gray-200">发送数据</span>
              <div class="flex items-center gap-1.5">
                <button
                  :class="sendMode === 'text' ? 'bg-blue-50 dark:bg-blue-950 border-blue-300 dark:border-blue-700 text-blue-700 dark:text-blue-300' : 'border-gray-200 dark:border-gray-700 text-gray-600 dark:text-gray-400 hover:bg-gray-50 dark:hover:bg-gray-800'"
                  @click="sendMode = 'text'"
                  class="px-2.5 py-1 text-xs font-medium rounded border transition-colors">
                  文本
                </button>
                <button
                  :class="sendMode === 'hex' ? 'bg-blue-50 dark:bg-blue-950 border-blue-300 dark:border-blue-700 text-blue-700 dark:text-blue-300' : 'border-gray-200 dark:border-gray-700 text-gray-600 dark:text-gray-400 hover:bg-gray-50 dark:hover:bg-gray-800'"
                  @click="sendMode = 'hex'"
                  class="px-2.5 py-1 text-xs font-medium rounded border transition-colors">
                  HEX
                </button>
              </div>
            </div>
            <div class="flex gap-2 p-4">
              <input
                v-model="sendData"
                :placeholder="sendMode === 'hex' ? '例如: AA BB CC 0D 0A' : '输入要发送的文本...'"
                @keyup.enter="send"
                class="flex-1 px-3 py-2 text-sm rounded-md border border-gray-300 dark:border-gray-700 bg-gray-50 dark:bg-gray-800 text-gray-900 dark:text-gray-100 placeholder:text-gray-300 dark:placeholder:text-gray-600 focus:border-blue-500 dark:focus:border-blue-400 outline-none font-mono transition-colors" />
              <button @click="send" :disabled="!isOpen"
                class="px-5 py-2 text-sm font-semibold text-white bg-blue-600 hover:bg-blue-700 dark:bg-blue-700 dark:hover:bg-blue-600 rounded-md disabled:bg-gray-200 dark:disabled:bg-gray-800 disabled:text-gray-400 dark:disabled:text-gray-600 transition-colors">
                发送
              </button>
            </div>
          </div>
        </div>
      </div>

    </div>
  </div>
</template>

<style>
/* 隐藏 number input 原生调节按钮 */
.no-spin::-webkit-outer-spin-button,
.no-spin::-webkit-inner-spin-button {
  -webkit-appearance: none;
  margin: 0;
}
.no-spin {
  -moz-appearance: textfield;
}

/* 自定义 select 下拉箭头 — 亮/暗自适应 */
.select-arrow {
  appearance: none;
  -webkit-appearance: none;
  background-image: url("data:image/svg+xml,%3csvg xmlns='http://www.w3.org/2000/svg' fill='none' viewBox='0 0 20 20'%3e%3cpath stroke='%236b7280' stroke-linecap='round' stroke-linejoin='round' stroke-width='1.5' d='M6 8l4 4 4-4'/%3e%3c/svg%3e");
  background-position: right 0.5rem center;
  background-repeat: no-repeat;
  background-size: 1.25em 1.25em;
}
.dark .select-arrow {
  background-image: url("data:image/svg+xml,%3csvg xmlns='http://www.w3.org/2000/svg' fill='none' viewBox='0 0 20 20'%3e%3cpath stroke='%239ca3af' stroke-linecap='round' stroke-linejoin='round' stroke-width='1.5' d='M6 8l4 4 4-4'/%3e%3c/svg%3e");
}
</style>
