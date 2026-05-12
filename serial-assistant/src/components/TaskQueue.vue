<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

const props = defineProps<{
  tabId: string;
  isPortOpen: boolean;
}>();

type QueueStatus = "Idle" | "Running" | "Paused" | "Stopped";

type TaskTypeValue =
  | { type: "Send"; value: { data: number[] } }
  | { type: "Delay"; value: { ms: number } }
  | { type: "Jump"; value: { target: number } }
  | { type: "LoopStart"; value: { count: number } }
  | { type: "LoopEnd" }
  | { type: "Stop" };

interface TaskItem {
  index: number;
  task_type: TaskTypeValue;
  label: string;
}

interface QueueStatusPayload {
  status: QueueStatus;
  current_index: number | null;
}

const taskList = ref<TaskItem[]>([]);
const queueStatus = ref<QueueStatus>("Idle");
const currentIndex = ref<number | null>(null);
const editingIndex = ref<number | null>(null);
const dragIdx = ref<number | null>(null);
const dragOverIdx = ref<number | null>(null);

let unlistenQueue: UnlistenFn | null = null;

const canStart = computed(() =>
  queueStatus.value === "Idle" && props.isPortOpen && taskList.value.length > 0
);
const isRunning = computed(() => queueStatus.value === "Running");
const isPaused = computed(() => queueStatus.value === "Paused");
const isIdle = computed(() => queueStatus.value === "Idle" || queueStatus.value === "Stopped");
const canEdit = computed(() => isIdle.value);

const statusText = computed(() => {
  switch (queueStatus.value) {
    case "Running": return "● 运行中";
    case "Paused": return "⏸ 已暂停";
    case "Stopped": return "⏹ 已停止";
    default: return "○ 空闲";
  }
});

const statusClass = computed(() => {
  switch (queueStatus.value) {
    case "Running": return "text-emerald-600 dark:text-emerald-400";
    case "Paused": return "text-amber-600 dark:text-amber-400";
    default: return "text-gray-400 dark:text-gray-600";
  }
});

watch(() => props.isPortOpen, (open) => {
  if (!open && (isRunning.value || isPaused.value)) {
    invoke("queue_stop", { portId: props.tabId }).catch(() => {});
    queueStatus.value = "Idle";
    currentIndex.value = null;
  }
});

function handleDocumentClick(e: MouseEvent) {
  if (editingIndex.value === null) return;
  const target = e.target as HTMLElement;
  if (!target.closest(".task-editor")) {
    editingIndex.value = null;
  }
}

watch(editingIndex, (val) => {
  if (val !== null) {
    document.addEventListener("click", handleDocumentClick, true);
  } else {
    document.removeEventListener("click", handleDocumentClick, true);
  }
});

function typeLabel(tt: TaskTypeValue): string {
  switch (tt.type) {
    case "Send": return "发送";
    case "Delay": return "延时";
    case "Jump": return "跳转";
    case "LoopStart": return "循环开始";
    case "LoopEnd": return "循环结束";
    case "Stop": return "停止";
  }
}

function typeSummary(tt: TaskTypeValue): string {
  switch (tt.type) {
    case "Send": {
      const hex = tt.value.data.map((b) => b.toString(16).toUpperCase().padStart(2, "0")).join(" ");
      return hex.length > 30 ? hex.substring(0, 30) + "..." : hex;
    }
    case "Delay": return `${tt.value.ms} ms`;
    case "Jump": return `转到 #${tt.value.target}`;
    case "LoopStart": return `重复 ${tt.value.count} 次`;
    case "LoopEnd": return "循环结束";
    case "Stop": return "停止执行";
  }
}

function typeBadgeClass(tt: TaskTypeValue): string {
  switch (tt.type) {
    case "Send": return "bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-300";
    case "Delay": return "bg-amber-100 dark:bg-amber-900 text-amber-700 dark:text-amber-300";
    case "Jump": return "bg-purple-100 dark:bg-purple-900 text-purple-700 dark:text-purple-300";
    case "LoopStart": return "bg-emerald-100 dark:bg-emerald-900 text-emerald-700 dark:text-emerald-300";
    case "LoopEnd": return "bg-emerald-100 dark:bg-emerald-900 text-emerald-700 dark:text-emerald-300";
    case "Stop": return "bg-red-100 dark:bg-red-900 text-red-700 dark:text-red-300";
  }
}

function makeDefaultTask(type: string): TaskItem {
  const index = taskList.value.length;
  switch (type) {
    case "Send":
      return { index, task_type: { type: "Send", value: { data: [] } }, label: "" };
    case "Delay":
      return { index, task_type: { type: "Delay", value: { ms: 500 } }, label: "" };
    case "Jump":
      return { index, task_type: { type: "Jump", value: { target: 0 } }, label: "" };
    case "LoopStart":
      return { index, task_type: { type: "LoopStart", value: { count: 1 } }, label: "" };
    case "LoopEnd":
      return { index, task_type: { type: "LoopEnd" }, label: "" };
    case "Stop":
      return { index, task_type: { type: "Stop" }, label: "" };
    default:
      return { index, task_type: { type: "Stop" }, label: "" };
  }
}

function addTask(type: string) {
  taskList.value.push(makeDefaultTask(type));
  editingIndex.value = taskList.value.length - 1;
  renumber();
}

function removeTask(idx: number) {
  taskList.value.splice(idx, 1);
  if (editingIndex.value === idx) {
    editingIndex.value = null;
  } else if (editingIndex.value !== null && editingIndex.value > idx) {
    editingIndex.value--;
  }
  renumber();
}

function moveTask(idx: number, dir: number) {
  const target = idx + dir;
  if (target < 0 || target >= taskList.value.length) return;
  const tmp = taskList.value[idx];
  taskList.value[idx] = taskList.value[target];
  taskList.value[target] = tmp;
  if (editingIndex.value === idx) editingIndex.value = target;
  else if (editingIndex.value === target) editingIndex.value = idx;
  renumber();
}

function renumber() {
  taskList.value.forEach((t, i) => (t.index = i));
}

function onDragStart(idx: number, e: DragEvent) {
  if (!canEdit.value) return;
  dragIdx.value = idx;
  if (e.dataTransfer) {
    e.dataTransfer.effectAllowed = "move";
    e.dataTransfer.setData("text/plain", String(idx));
  }
}

function onDragOver(idx: number, e: DragEvent) {
  if (!canEdit.value || dragIdx.value === null) return;
  e.preventDefault();
  if (e.dataTransfer) {
    e.dataTransfer.dropEffect = "move";
  }
  dragOverIdx.value = idx;
}

function onDragLeave() {
  dragOverIdx.value = null;
}

function onDrop(idx: number) {
  if (!canEdit.value || dragIdx.value === null) return;
  const from = dragIdx.value;
  if (from === idx) {
    dragIdx.value = null;
    dragOverIdx.value = null;
    return;
  }
  const item = taskList.value.splice(from, 1)[0];
  taskList.value.splice(idx, 0, item);
  if (editingIndex.value === from) {
    editingIndex.value = idx;
  } else if (editingIndex.value !== null) {
    if (from < idx && editingIndex.value > from && editingIndex.value <= idx) {
      editingIndex.value--;
    } else if (from > idx && editingIndex.value >= idx && editingIndex.value < from) {
      editingIndex.value++;
    }
  }
  dragIdx.value = null;
  dragOverIdx.value = null;
  renumber();
}

function onDragEnd() {
  dragIdx.value = null;
  dragOverIdx.value = null;
}

function clearAll() {
  taskList.value = [];
  editingIndex.value = null;
}

// Editing state for the inline form
const editSendData = ref("");
const editSendMode = ref<"text" | "hex">("hex");
const editDelayMs = ref(500);
const editJumpTarget = ref(0);
const editLoopCount = ref(1);
const editLabel = ref("");

function startEdit(idx: number) {
  const task = taskList.value[idx];
  editingIndex.value = idx;
  editLabel.value = task.label;
  switch (task.task_type.type) {
    case "Send": {
      editSendMode.value = "hex";
      editSendData.value = task.task_type.value.data
        .map((b) => b.toString(16).toUpperCase().padStart(2, "0"))
        .join(" ");
      break;
    }
    case "Delay":
      editDelayMs.value = task.task_type.value.ms;
      break;
    case "Jump":
      editJumpTarget.value = task.task_type.value.target;
      break;
    case "LoopStart":
      editLoopCount.value = task.task_type.value.count;
      break;
  }
}

function saveEdit() {
  if (editingIndex.value === null) return;
  const idx = editingIndex.value;
  const task = taskList.value[idx];
  task.label = editLabel.value;

  switch (task.task_type.type) {
    case "Send": {
      let data: number[];
      if (editSendMode.value === "hex") {
        const hex = editSendData.value.replace(/\s+/g, "");
        if (hex.length > 0) {
          if (hex.length % 2 !== 0 || !/^[0-9a-fA-F]*$/.test(hex)) return;
          data = [];
          for (let i = 0; i < hex.length; i += 2) {
            data.push(parseInt(hex.substring(i, i + 2), 16));
          }
        } else {
          data = [];
        }
      } else {
        data = Array.from(new TextEncoder().encode(editSendData.value));
      }
      task.task_type = { type: "Send", value: { data } };
      break;
    }
    case "Delay":
      task.task_type = { type: "Delay", value: { ms: editDelayMs.value } };
      break;
    case "Jump":
      task.task_type = { type: "Jump", value: { target: editJumpTarget.value } };
      break;
    case "LoopStart":
      task.task_type = { type: "LoopStart", value: { count: editLoopCount.value } };
      break;
  }
  editingIndex.value = null;
}

function cancelEdit() {
  editingIndex.value = null;
}

async function startQueue() {
  if (!canStart.value) return;
  try {
    await invoke("queue_start", { portId: props.tabId, tasks: taskList.value });
    queueStatus.value = "Running";
  } catch (e) {
    console.error("Failed to start queue:", e);
  }
}

async function pauseQueue() {
  try {
    await invoke("queue_pause", { portId: props.tabId });
  } catch (e) {
    console.error("Failed to pause queue:", e);
  }
}

async function resumeQueue() {
  try {
    await invoke("queue_resume", { portId: props.tabId });
  } catch (e) {
    console.error("Failed to resume queue:", e);
  }
}

async function stopQueue() {
  try {
    await invoke("queue_stop", { portId: props.tabId });
  } catch (e) {
    console.error("Failed to stop queue:", e);
  }
}

onMounted(async () => {
  try {
    const snapshot = await invoke<{ status: string }>("queue_get_state", { portId: props.tabId });
    if (snapshot.status !== "Idle") {
      queueStatus.value = snapshot.status as QueueStatus;
    }
  } catch {
    // No queue exists
  }

  unlistenQueue = await listen<QueueStatusPayload>(
    `queue-update:${props.tabId}`,
    (event) => {
      queueStatus.value = event.payload.status;
      currentIndex.value = event.payload.current_index;
    }
  );
});

onUnmounted(() => {
  if (isRunning.value || isPaused.value) {
    invoke("queue_stop", { portId: props.tabId }).catch(() => {});
  }
  if (unlistenQueue) unlistenQueue();
  document.removeEventListener("click", handleDocumentClick, true);
});
</script>

<template>
  <div class="rounded-xl border border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900 overflow-hidden">
    <!-- Header -->
    <div
      class="flex items-center justify-between px-4 py-2.5 border-b border-gray-100 dark:border-gray-800 bg-gray-50 dark:bg-gray-900"
    >
      <div class="flex items-center gap-2">
        <span class="text-sm font-semibold text-gray-700 dark:text-gray-200">任务队列</span>
        <span :class="statusClass" class="text-xs font-medium">{{ statusText }}</span>
        <span
          v-if="currentIndex !== null && taskList.length > 0"
          class="text-xs text-gray-400 dark:text-gray-600"
        >
          任务 {{ currentIndex + 1 }}/{{ taskList.length }}
        </span>
      </div>
    </div>

    <div>
      <!-- Control Bar -->
      <div class="flex items-center gap-2 px-4 py-2.5 border-b border-gray-100 dark:border-gray-800">
        <button
          v-if="isIdle"
          @click="startQueue"
          :disabled="!canStart"
          class="px-4 py-1.5 text-xs font-semibold text-white bg-emerald-600 hover:bg-emerald-700 dark:bg-emerald-700 dark:hover:bg-emerald-600 rounded-md disabled:bg-gray-200 dark:disabled:bg-gray-800 disabled:text-gray-400 dark:disabled:text-gray-600 transition-colors"
        >
          启动
        </button>
        <button
          v-if="isRunning"
          @click="pauseQueue"
          class="px-4 py-1.5 text-xs font-semibold text-white bg-amber-600 hover:bg-amber-700 dark:bg-amber-700 dark:hover:bg-amber-600 rounded-md transition-colors"
        >
          暂停
        </button>
        <button
          v-if="isPaused"
          @click="resumeQueue"
          class="px-4 py-1.5 text-xs font-semibold text-white bg-emerald-600 hover:bg-emerald-700 dark:bg-emerald-700 dark:hover:bg-emerald-600 rounded-md transition-colors"
        >
          继续
        </button>
        <button
          v-if="isRunning || isPaused"
          @click="stopQueue"
          class="px-4 py-1.5 text-xs font-semibold text-white bg-red-600 hover:bg-red-700 dark:bg-red-700 dark:hover:bg-red-600 rounded-md transition-colors"
        >
          停止
        </button>
        <span v-if="!canStart && isIdle" class="text-xs text-gray-400 dark:text-gray-600">
          {{ !props.isPortOpen ? '请先打开串口' : taskList.length === 0 ? '请添加任务' : '' }}
        </span>
      </div>

      <!-- Task List -->
      <div class="max-h-72 overflow-y-auto">
        <template v-if="taskList.length > 0">
          <div
            v-for="(task, idx) in taskList"
            :key="idx"
            :class="[
              'border-b border-gray-50 dark:border-gray-900 last:border-b-0 transition-colors',
              currentIndex === idx ? 'bg-blue-50 dark:bg-blue-950' : '',
              dragOverIdx === idx && dragIdx !== idx ? 'border-t-2 border-t-blue-500 dark:border-t-blue-400' : ''
            ]"
            @dragover="onDragOver(idx, $event)"
            @dragleave="onDragLeave"
            @drop="onDrop(idx)"
          >
            <!-- Task Row -->
            <div
              :draggable="canEdit"
              :class="[
                'flex items-center gap-2 px-4 py-2 text-sm group transition-opacity',
                dragIdx === idx ? 'opacity-40' : '',
                canEdit ? 'cursor-grab active:cursor-grabbing' : ''
              ]"
              @dragstart="onDragStart(idx, $event)"
              @dragend="onDragEnd"
            >
              <span class="text-gray-300 dark:text-gray-700 text-xs shrink-0 select-none" :class="canEdit ? 'visible' : 'invisible'">&#x2630;</span>
              <span class="text-gray-400 dark:text-gray-600 text-xs w-6 shrink-0 tabular-nums">{{ task.index }}</span>
              <span
                :class="typeBadgeClass(task.task_type)"
                class="text-[11px] font-medium px-1.5 py-0.5 rounded shrink-0"
              >
                {{ typeLabel(task.task_type) }}
              </span>
              <span class="flex-1 truncate text-gray-600 dark:text-gray-400 font-mono text-xs">
                {{ typeSummary(task.task_type) }}
              </span>
              <span
                v-if="task.label"
                class="text-[11px] text-gray-400 dark:text-gray-500 italic truncate max-w-[120px] shrink-0"
              >
                {{ task.label }}
              </span>
              <span
                v-if="currentIndex === idx"
                class="text-blue-500 dark:text-blue-400 text-xs animate-pulse shrink-0"
              >&#9664;</span>
              <template v-if="canEdit">
                <button
                  @click="moveTask(idx, -1)"
                  :disabled="idx === 0"
                  class="text-gray-300 dark:text-gray-700 hover:text-gray-500 dark:hover:text-gray-400 disabled:opacity-30 text-sm leading-none shrink-0"
                  title="上移"
                >&#9650;</button>
                <button
                  @click="moveTask(idx, 1)"
                  :disabled="idx === taskList.length - 1"
                  class="text-gray-300 dark:text-gray-700 hover:text-gray-500 dark:hover:text-gray-400 disabled:opacity-30 text-sm leading-none shrink-0"
                  title="下移"
                >&#9660;</button>
                <button
                  @click="startEdit(idx)"
                  class="text-gray-300 dark:text-gray-700 hover:text-blue-500 dark:hover:text-blue-400 text-sm leading-none shrink-0"
                  title="编辑"
                >&#9998;</button>
                <button
                  @click="removeTask(idx)"
                  class="text-gray-300 dark:text-gray-700 hover:text-red-500 dark:hover:text-red-400 text-sm leading-none shrink-0"
                  title="删除"
                >&times;</button>
              </template>
            </div>

            <!-- Inline Editor -->
            <div
              v-if="editingIndex === idx"
              class="task-editor px-4 pb-3 pt-1 ml-6 border-t border-gray-100 dark:border-gray-800 bg-gray-50 dark:bg-gray-950 rounded-b"
            >
              <!-- Send editor -->
              <div v-if="task.task_type.type === 'Send'" class="flex flex-col gap-2">
                <div class="flex items-center gap-1.5">
                  <button
                    :class="editSendMode === 'hex' ? 'bg-blue-50 dark:bg-blue-950 border-blue-300 dark:border-blue-700 text-blue-700 dark:text-blue-300' : 'border-gray-200 dark:border-gray-700 text-gray-600 dark:text-gray-400'"
                    @click="editSendMode = 'hex'"
                    class="px-2 py-0.5 text-[11px] font-medium rounded border transition-colors"
                  >HEX</button>
                  <button
                    :class="editSendMode === 'text' ? 'bg-blue-50 dark:bg-blue-950 border-blue-300 dark:border-blue-700 text-blue-700 dark:text-blue-300' : 'border-gray-200 dark:border-gray-700 text-gray-600 dark:text-gray-400'"
                    @click="editSendMode = 'text'"
                    class="px-2 py-0.5 text-[11px] font-medium rounded border transition-colors"
                  >文本</button>
                </div>
                <input
                  v-model="editSendData"
                  :placeholder="editSendMode === 'hex' ? '例如: AA BB CC' : '输入文本...'"
                  class="px-2.5 py-1.5 text-sm font-mono rounded border border-gray-300 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 outline-none focus:border-blue-500 dark:focus:border-blue-400"
                />
              </div>

              <!-- Delay editor -->
              <div v-if="task.task_type.type === 'Delay'" class="flex items-center gap-2">
                <input
                  v-model.number="editDelayMs"
                  type="number"
                  min="0"
                  step="10"
                  class="w-28 px-2.5 py-1.5 text-sm font-mono rounded border border-gray-300 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 outline-none focus:border-blue-500 dark:focus:border-blue-400"
                />
                <span class="text-xs text-gray-500 dark:text-gray-400">毫秒</span>
              </div>

              <!-- Jump editor -->
              <div v-if="task.task_type.type === 'Jump'" class="flex items-center gap-2">
                <span class="text-xs text-gray-500 dark:text-gray-400">转到任务 #</span>
                <input
                  v-model.number="editJumpTarget"
                  type="number"
                  min="0"
                  :max="taskList.length - 1"
                  class="w-20 px-2.5 py-1.5 text-sm font-mono rounded border border-gray-300 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 outline-none focus:border-blue-500 dark:focus:border-blue-400"
                />
              </div>

              <!-- LoopStart editor -->
              <div v-if="task.task_type.type === 'LoopStart'" class="flex items-center gap-2">
                <span class="text-xs text-gray-500 dark:text-gray-400">重复</span>
                <input
                  v-model.number="editLoopCount"
                  type="number"
                  min="1"
                  class="w-20 px-2.5 py-1.5 text-sm font-mono rounded border border-gray-300 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 outline-none focus:border-blue-500 dark:focus:border-blue-400"
                />
                <span class="text-xs text-gray-500 dark:text-gray-400">次</span>
              </div>

              <!-- Label field (all types) -->
              <div class="flex items-center gap-2 mt-2">
                <input
                  v-model="editLabel"
                  placeholder="标签（可选）"
                  class="flex-1 px-2.5 py-1.5 text-sm rounded border border-gray-300 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 outline-none focus:border-blue-500 dark:focus:border-blue-400"
                />
              </div>

              <!-- Save/Cancel -->
              <div class="flex items-center gap-1.5 mt-2">
                <button
                  @click="saveEdit"
                  class="px-3 py-1 text-xs font-medium text-white bg-blue-600 hover:bg-blue-700 dark:bg-blue-700 dark:hover:bg-blue-600 rounded transition-colors"
                >保存</button>
                <button
                  @click="cancelEdit"
                  class="px-3 py-1 text-xs font-medium text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800 rounded transition-colors"
                >取消</button>
              </div>
            </div>
          </div>
        </template>
        <div v-else class="px-4 py-8 text-center text-gray-300 dark:text-gray-700 text-sm">
          暂无任务，请添加任务
        </div>
      </div>

      <!-- Bottom Bar: Add Task Buttons -->
      <div
        v-if="canEdit"
        class="flex items-center gap-1.5 px-4 py-2.5 border-t border-gray-100 dark:border-gray-800 bg-gray-50 dark:bg-gray-900"
      >
        <button @click="addTask('Send')"
          class="px-2.5 py-1 text-[11px] font-medium rounded border border-blue-200 dark:border-blue-800 text-blue-600 dark:text-blue-400 hover:bg-blue-50 dark:hover:bg-blue-950 transition-colors"
        >+ 发送</button>
        <button @click="addTask('Delay')"
          class="px-2.5 py-1 text-[11px] font-medium rounded border border-amber-200 dark:border-amber-800 text-amber-600 dark:text-amber-400 hover:bg-amber-50 dark:hover:bg-amber-950 transition-colors"
        >+ 延时</button>
        <button @click="addTask('Jump')"
          class="px-2.5 py-1 text-[11px] font-medium rounded border border-purple-200 dark:border-purple-800 text-purple-600 dark:text-purple-400 hover:bg-purple-50 dark:hover:bg-purple-950 transition-colors"
        >+ 跳转</button>
        <button @click="addTask('LoopStart')"
          class="px-2.5 py-1 text-[11px] font-medium rounded border border-emerald-200 dark:border-emerald-800 text-emerald-600 dark:text-emerald-400 hover:bg-emerald-50 dark:hover:bg-emerald-950 transition-colors"
        >+ 循环开始</button>
        <button @click="addTask('LoopEnd')"
          class="px-2.5 py-1 text-[11px] font-medium rounded border border-emerald-200 dark:border-emerald-800 text-emerald-600 dark:text-emerald-400 hover:bg-emerald-50 dark:hover:bg-emerald-950 transition-colors"
        >+ 循环结束</button>
        <button @click="addTask('Stop')"
          class="px-2.5 py-1 text-[11px] font-medium rounded border border-red-200 dark:border-red-800 text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-950 transition-colors"
        >+ 停止</button>
        <button
          v-if="taskList.length > 0"
          @click="clearAll"
          class="ml-auto px-2.5 py-1 text-[11px] font-medium rounded border border-gray-200 dark:border-gray-700 text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
        >清空全部</button>
      </div>
    </div>
  </div>
</template>
