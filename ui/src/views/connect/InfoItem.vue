<script setup lang="ts">
defineProps<{ label: string; value: number | string }>()
</script>

<template>
  <div class="info-item">
    <div class="dim">{{ label }}</div>
    <div class="num">{{ value }}</div>
  </div>
</template>

<style scoped>
.info-item {
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 14px 16px;
}
.num {
  font-size: 22px;
  font-weight: 700;
  margin-top: 2px;
}
</style>