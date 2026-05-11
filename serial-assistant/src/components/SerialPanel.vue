<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, nextTick, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

const props = defineProps<{
  tabId: string;
}>();

const emit = defineEmits<{
  (e: "connection-changed", payload: { isOpen: boolean; portName: string }): void;
}>();

const ports = ref<string[]>([]);
const selectedPort = ref("");
const baudRate = ref(115200);
const dataBits = ref(8);
const stopBits = ref(1);
const parity = ref("None");
const isOpen = ref(false);
const errorMsg = ref("");

const baudRates = [9600, 14400, 19200, 38400, 57600, 115200, 230400, 460800, 921600];

interface DataEntry {
  timestamp: number;
  direction: "tx" | "rx";
  data: number[];
}

const displayMode = ref<"text" | "hex">("text");
const dataEntries = ref<DataEntry[]>([]);
const dataViewer = ref<HTMLDivElement>();

const sendMode = ref<"text" | "hex">("text");
const sendData = ref("");

let unlisten: UnlistenFn | null = null;

const rxBytes = computed(() =>
  dataEntries.value
    .filter((e) => e.direction === "rx")
    .reduce((sum, e) => sum + e.data.length, 0)
);
const txBytes = computed(() =>
  dataEntries.value
    .filter((e) => e.direction === "tx")
    .reduce((sum, e) => sum + e.data.length, 0)
);

watch([isOpen, selectedPort], ([open, port]) => {
  emit("connection-changed", { isOpen: open, portName: open ? port : "" });
});

function padTime(n: number): string {
  return n.toString().padStart(2, "0");
}

function formatTimestamp(ts: number): string {
  const d = new Date(ts);
  return `${padTime(d.getHours())}:${padTime(d.getMinutes())}:${padTime(d.getSeconds())}.${d.getMilliseconds().toString().padStart(3, "0")}`;
}

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
      portId: props.tabId,
      portName: selectedPort.value,
      baudRate: baudRate.value,
      dataBits: dataBits.value,
      stopBits: stopBits.value,
      parity: parity.value,
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
    await invoke("close_serial", { portId: props.tabId });
    isOpen.value = false;
  } catch (error) {
    errorMsg.value = error as string;
  }
}

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
    await invoke("send_serial", { portId: props.tabId, data });
    dataEntries.value.push({ timestamp: Date.now(), direction: "tx", data });
    trimData();
    scrollToBottom();
  } catch (error) {
    errorMsg.value = error as string;
  }
}

function formatEntryData(entry: DataEntry): string {
  if (displayMode.value === "hex") {
    return entry.data
      .map((b) => b.toString(16).toUpperCase().padStart(2, "0"))
      .join(" ");
  }
  return entry.data
    .map((b) => {
      if (b >= 0x20 && b < 0x7f) return String.fromCharCode(b);
      if (b === 0x0a) return "\n";
      if (b === 0x0d) return "\r";
      if (b === 0x09) return "\t";
      return ".";
    })
    .join("");
}

function clearData() {
  dataEntries.value = [];
}

function trimData() {
  const total = dataEntries.value.reduce((s, e) => s + e.data.length, 0);
  if (total > 100000) {
    let cut = 0;
    const target = 50000;
    const kept: DataEntry[] = [];
    for (let i = dataEntries.value.length - 1; i >= 0; i--) {
      kept.unshift(dataEntries.value[i]);
      cut += dataEntries.value[i].data.length;
      if (cut >= target) break;
    }
    dataEntries.value = kept;
  }
}

function scrollToBottom() {
  nextTick(() => {
    if (dataViewer.value) {
      dataViewer.value.scrollTop = dataViewer.value.scrollHeight;
    }
  });
}

onMounted(async () => {
  await fetchPorts(true);
  unlisten = await listen<number[]>(`serial-data:${props.tabId}`, (event) => {
    dataEntries.value.push({ timestamp: Date.now(), direction: "rx", data: event.payload });
    trimData();
    scrollToBottom();
  });
});

onUnmounted(() => {
  if (unlisten) unlisten();
  if (isOpen.value) {
    invoke("close_serial", { portId: props.tabId });
  }
});
</script>

<template>
  <div>
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

      <div class="flex flex-col gap-1">
        <label class="text-[11px] font-medium uppercase tracking-wider text-gray-500 dark:text-gray-500">数据位</label>
        <select v-model="dataBits" :disabled="isOpen"
          class="select-arrow pr-7 pl-2.5 py-1.5 text-sm rounded-md border border-gray-300 dark:border-gray-700 bg-gray-50 dark:bg-gray-800 text-gray-900 dark:text-gray-100 disabled:opacity-40 focus:border-blue-500 dark:focus:border-blue-400 outline-none">
          <option :value="8">8</option><option :value="7">7</option><option :value="6">6</option><option :value="5">5</option>
        </select>
      </div>

      <div class="flex flex-col gap-1">
        <label class="text-[11px] font-medium uppercase tracking-wider text-gray-500 dark:text-gray-500">停止位</label>
        <select v-model="stopBits" :disabled="isOpen"
          class="select-arrow pr-7 pl-2.5 py-1.5 text-sm rounded-md border border-gray-300 dark:border-gray-700 bg-gray-50 dark:bg-gray-800 text-gray-900 dark:text-gray-100 disabled:opacity-40 focus:border-blue-500 dark:focus:border-blue-400 outline-none">
          <option :value="1">1</option><option :value="2">2</option>
        </select>
      </div>

      <div class="flex flex-col gap-1">
        <label class="text-[11px] font-medium uppercase tracking-wider text-gray-500 dark:text-gray-500">校验位</label>
        <select v-model="parity" :disabled="isOpen"
          class="select-arrow pr-7 pl-2.5 py-1.5 text-sm rounded-md border border-gray-300 dark:border-gray-700 bg-gray-50 dark:bg-gray-800 text-gray-900 dark:text-gray-100 disabled:opacity-40 focus:border-blue-500 dark:focus:border-blue-400 outline-none">
          <option value="None">None</option><option value="Even">Even</option><option value="Odd">Odd</option>
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

    <!-- 数据收发查看器 -->
    <div class="mb-3 rounded-xl border border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900 overflow-hidden">
      <div class="flex items-center justify-between px-4 py-2.5 border-b border-gray-100 dark:border-gray-800 bg-gray-50 dark:bg-gray-900">
        <span class="text-sm font-semibold text-gray-700 dark:text-gray-200">数据收发</span>
        <div class="flex items-center gap-1.5">
          <span class="text-[11px] text-gray-400 dark:text-gray-600 mr-2 tabular-nums">
            RX:{{ rxBytes }} TX:{{ txBytes }}
          </span>
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
      <div ref="dataViewer" class="h-80 overflow-y-auto p-4 bg-gray-50 dark:bg-gray-950 font-mono text-sm leading-relaxed whitespace-pre-wrap break-all">
        <template v-if="dataEntries.length > 0">
          <div
            v-for="(entry, idx) in dataEntries"
            :key="idx"
            class="mb-0.5"
            :class="entry.direction === 'tx' ? 'text-blue-600 dark:text-blue-400' : 'text-emerald-600 dark:text-emerald-400'"
          >
            <span class="text-gray-400 dark:text-gray-600 font-normal">[{{ formatTimestamp(entry.timestamp) }}]</span>
            <span class="font-semibold ml-1">{{ entry.direction === 'tx' ? 'TX' : 'RX' }}</span>
            <span class="ml-1 text-gray-800 dark:text-gray-200">{{ formatEntryData(entry) }}</span>
          </div>
        </template>
        <div v-else class="text-gray-300 dark:text-gray-700 font-sans text-sm">等待数据...</div>
      </div>
    </div>

    <!-- 发送区域 -->
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
</template>

<style scoped>
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
