<script setup lang="ts">
import { computed } from 'vue'
import zhCn from 'element-plus/es/locale/lang/zh-cn'
import en from 'element-plus/es/locale/lang/en'
import { useScanStore } from './stores/scan'
import { useI18n } from './i18n'
import TopBar from './components/TopBar.vue'
import ConnectView from './views/ConnectView.vue'
import ScanView from './views/ScanView.vue'
import ReportView from './views/ReportView.vue'
import HistoryView from './views/HistoryView.vue'

const store = useScanStore()
const { locale } = useI18n()
const elLocale = computed(() => (locale.value === 'zh' ? zhCn : en))
</script>

<template>
  <el-config-provider :locale="elLocale">
    <TopBar />
    <div class="view-container">
      <div class="view" v-show="store.currentView === 1" :class="{ visible: store.currentView === 1 }">
        <ConnectView />
      </div>
      <div class="view" v-show="store.currentView === 2" :class="{ visible: store.currentView === 2 }">
        <ScanView />
      </div>
      <div class="view" v-show="store.currentView === 3" :class="{ visible: store.currentView === 3 }">
        <ReportView />
      </div>
      <div class="view" v-show="store.currentView === 4" :class="{ visible: store.currentView === 4 }">
        <HistoryView />
      </div>
    </div>
  </el-config-provider>
</template>

<style scoped>
.view.visible {
  animation: fadein 0.25s ease;
}
</style>
