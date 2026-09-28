<script setup lang="ts">
import { useScanStore } from '../stores/scan'
import { useI18n } from '../i18n'

const store = useScanStore()
const { t, toggleLocale } = useI18n()
</script>

<template>
  <div class="topbar">
    <div class="logo">
      <div class="logo-badge">衡</div>
      <div>
        {{ t('app.name') }}
        <span class="sub">{{ t('app.sub') }}</span>
      </div>
    </div>
    <div class="spacer"></div>
    <div class="local-badge">{{ t('app.localBadge') }}</div>
    <button class="btn ghost switch-btn" @click="toggleLocale">
      {{ t('app.langToggle') }}
    </button>
    <button v-if="store.selectedPath" class="btn ghost switch-btn" @click="store.switchRepo()">
      {{ t('app.switchRepo') }}
    </button>
    <button class="btn ghost switch-btn" @click="store.setCurrentView(4)">
      {{ t('app.history') }}
    </button>
    <div class="steps">
      <div class="step" :class="{ active: store.currentView === 1, done: store.currentView > 1 }">{{ t('app.step1') }}</div>
      <div class="step-arrow">→</div>
      <div class="step" :class="{ active: store.currentView === 2, done: store.currentView > 2 }">{{ t('app.step2') }}</div>
      <div class="step-arrow">→</div>
      <div class="step" :class="{ active: store.currentView === 3 }">{{ t('app.step3') }}</div>
      <div class="step-arrow">→</div>
      <div class="step" :class="{ active: store.currentView === 4 }">{{ t('app.step4') }}</div>
    </div>
  </div>
</template>

<style scoped>
.topbar {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 18px 0 14px;
  border-bottom: 1px solid var(--border);
  margin-bottom: 24px;
}
.logo {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 18px;
  font-weight: 700;
}
.logo-badge {
  width: 34px;
  height: 34px;
  border-radius: 9px;
  background: linear-gradient(135deg, #4f8cff, #7a5cff);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 17px;
  font-weight: 800;
  color: #fff;
}
.logo .sub {
  font-size: 12px;
  font-weight: 400;
  color: var(--text-dim);
  display: block;
}
.spacer {
  flex: 1;
}
.local-badge {
  font-size: 12px;
  color: var(--info);
  border: 1px solid rgba(88, 196, 163, 0.35);
  border-radius: 999px;
  padding: 4px 12px;
  background: rgba(88, 196, 163, 0.08);
}
.switch-btn {
  font-size: 12px;
  padding: 5px 12px;
}
.steps {
  display: flex;
  gap: 6px;
  align-items: center;
  font-size: 12px;
  color: var(--text-dim);
}
.step {
  padding: 5px 12px;
  border-radius: 999px;
  border: 1px solid var(--border);
}
.step.active {
  color: #fff;
  background: var(--accent);
  border-color: var(--accent);
}
.step.done {
  color: var(--info);
  border-color: rgba(88, 196, 163, 0.4);
}
.step-arrow {
  color: var(--border);
}
</style>
