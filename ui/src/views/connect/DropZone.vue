<script setup lang="ts">
import { useScanStore } from '../../stores/scan'
const store = useScanStore()
</script>

<template>
  <div class="dropzone" @click="store.selectDirectory()">
    <div class="icon">📂</div>
    <div class="title">选择要体检的 Git 仓库</div>
    <p>路径仅来自系统目录选择对话框，应用不会直接触达文件系统之外的数据</p>
    <button class="btn" style="margin-top:20px;" @click.stop="store.selectDirectory()">选择目录…</button>
    <p class="dim" style="margin-top:14px;">没有仓库？试试演示仓库 <code>demo-repo</code>（内置三类合成问题数据）</p>
  </div>
</template>

<style scoped>
.dropzone {
  border: 2px dashed var(--border);
  border-radius: var(--radius);
  padding: 64px 24px;
  text-align: center;
  transition: all 0.2s;
  background: var(--bg-panel);
  cursor: pointer;
}
.dropzone:hover {
  border-color: var(--accent);
  background: var(--accent-soft);
}
.icon {
  font-size: 44px;
  margin-bottom: 14px;
}
.title {
  font-size: 16px;
  font-weight: 600;
}
.dropzone p {
  color: var(--text-dim);
  font-size: 13px;
  margin-top: 8px;
}
code {
  background: var(--bg-card);
  padding: 2px 6px;
  border-radius: 4px;
  font-family: "SF Mono", Menlo, monospace;
}
</style>