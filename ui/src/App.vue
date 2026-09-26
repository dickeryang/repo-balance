<script setup lang="ts">
import { useScanStore } from './stores/scan'
import TopBar from './components/TopBar.vue'
import ConnectView from './views/ConnectView.vue'
import ScanView from './views/ScanView.vue'
import ReportView from './views/ReportView.vue'

const store = useScanStore()
</script>

<template>
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
  </div>
</template>

<style scoped>
.view.visible {
  animation: fadein 0.25s ease;
}
</style>