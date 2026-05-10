<script setup lang="ts">
import { ref, onMounted } from "vue";
// 引入 Tauri 的 invoke 方法，用于呼叫 Rust 后端
import { invoke } from "@tauri-apps/api/core";

const ports = ref<string[]>([]);
const selectedPort = ref("");
const errorMsg = ref("");

// 获取串口列表
async function fetchPorts() {
  try {
    // 🌟 调用 Rust 后端的 get_serial_ports 命令
    ports.value = await invoke<string[]>("get_serial_ports");
    if (ports.value.length > 0) {
      selectedPort.value = ports.value[0]; // 默认选中第一个
    }
    errorMsg.value = "";
  } catch (error) {
    errorMsg.value = error as string;
    console.error(error);
  }
}

// 页面加载时获取一次
onMounted(() => {
  fetchPorts();
});
</script>

<template>
  <div class="container">
    <h1>跨平台串口助手 🚀</h1>
    <div class="controls">
      <select v-model="selectedPort">
        <option disabled value="">请选择串口</option>
        <option v-for="port in ports" :key="port" :value="port">
          {{ port }}
        </option>
      </select>
      <button @click="fetchPorts">🔄 刷新列表</button>
    </div>
    <p v-if="errorMsg" class="error">{{ errorMsg }}</p>
    <p v-else class="hint">当前选择: {{ selectedPort }}</p>
  </div>
</template>

<style scoped>
.container { 
  padding: 20px; 
  font-family: sans-serif; 
}
.controls { 
  display: flex; 
  gap: 10px; 
  margin-bottom: 20px; 
}
select { 
  padding: 5px 10px; 
  min-width: 200px; 
}
button { 
  padding: 5px 15px; 
  cursor: pointer; 
}
.error { color: red; }
.hint { color: #666; }
</style>
