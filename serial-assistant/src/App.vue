<script setup lang="ts">
import { ref, onMounted, watch } from "vue";
import SerialPanel from "./components/SerialPanel.vue";

const theme = ref<"dark" | "light">("dark");

function toggleTheme() {
  theme.value = theme.value === "dark" ? "light" : "dark";
}

watch(theme, (t) => {
  document.documentElement.classList.toggle("dark", t === "dark");
}, { immediate: true });

interface Tab {
  id: string;
  title: string;
  isOpen: boolean;
  portName: string;
}

const tabs = ref<Tab[]>([]);
const activeTabId = ref<string | null>(null);

function addTab() {
  const id = crypto.randomUUID();
  const index = tabs.value.length + 1;
  tabs.value.push({ id, title: `串口 ${index}`, isOpen: false, portName: "" });
  activeTabId.value = id;
}

function removeTab(tabId: string) {
  tabs.value = tabs.value.filter((t) => t.id !== tabId);
  if (activeTabId.value === tabId) {
    activeTabId.value = tabs.value.length > 0 ? tabs.value[tabs.value.length - 1].id : null;
  }
}

function onConnectionChanged(tabId: string, payload: { isOpen: boolean; portName: string }) {
  const tab = tabs.value.find((t) => t.id === tabId);
  if (tab) {
    tab.isOpen = payload.isOpen;
    tab.portName = payload.portName;
  }
}

onMounted(() => {
  addTab();
});
</script>

<template>
  <div class="min-h-screen bg-gray-100 dark:bg-gray-950 p-5 font-sans text-gray-900 dark:text-gray-100 transition-colors">
    <div class="max-w-4xl mx-auto">

      <!-- 标签栏 -->
      <div class="flex items-center gap-1 mb-3 overflow-x-auto">
        <button
          v-for="tab in tabs"
          :key="tab.id"
          @click="activeTabId = tab.id"
          :class="activeTabId === tab.id
            ? 'border-blue-500 dark:border-blue-400 bg-white dark:bg-gray-900 text-gray-900 dark:text-gray-100'
            : 'border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800 text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-700'"
          class="flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium rounded-lg border transition-colors whitespace-nowrap"
        >
          <span
            class="w-1.5 h-1.5 rounded-full flex-shrink-0"
            :class="tab.isOpen ? 'bg-emerald-500' : 'bg-gray-400 dark:bg-gray-600'"
          ></span>
          <span>{{ tab.isOpen ? tab.portName : tab.title }}</span>
          <span
            @click.stop="removeTab(tab.id)"
            class="ml-1 text-gray-400 hover:text-red-500 dark:hover:text-red-400 text-sm leading-none"
          >&times;</span>
        </button>
        <button
          @click="addTab"
          class="px-3 py-1.5 text-sm font-medium text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-800 rounded-lg transition-colors whitespace-nowrap"
        >
          + 新标签
        </button>
        <button
          @click="toggleTheme"
          class="ml-auto px-3 py-1.5 text-xs font-medium rounded-md border border-gray-300 dark:border-gray-700 bg-white dark:bg-gray-900 text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors"
        >
          {{ theme === "dark" ? "☀ 白天模式" : "☾ 黑夜模式" }}
        </button>
      </div>

      <!-- 活动标签内容 -->
      <div v-if="activeTabId">
        <SerialPanel
          v-for="tab in tabs"
          :key="tab.id"
          :tabId="tab.id"
          v-show="activeTabId === tab.id"
          @connection-changed="(payload: any) => onConnectionChanged(tab.id, payload)"
        />
      </div>
      <div v-else class="text-center text-gray-400 dark:text-gray-600 py-20 text-sm">
        没有打开的标签页
      </div>

    </div>
  </div>
</template>
