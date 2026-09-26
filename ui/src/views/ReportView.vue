<script setup lang="ts">
import { useScanStore } from '../stores/scan'
import ScoreCard from './report/ScoreCard.vue'
import RadarChart from '../components/RadarChart.vue'
import DimLegend from './report/DimLegend.vue'
import FindingList from './report/FindingList.vue'
import ExportRow from './report/ExportRow.vue'

const store = useScanStore()
</script>

<template>
  <div class="report-head">
    <ScoreCard />
    <div class="card radar-panel">
      <div class="radar-wrap">
        <RadarChart v-if="store.report" :points="store.report.radarPoints" />
      </div>
      <DimLegend />
    </div>
  </div>
  <FindingList />
  <ExportRow />
  <div style="margin-top:20px; display:flex; justify-content:center;">
    <button class="btn ghost" @click="store.resetToConnect()">← 重新选择仓库</button>
  </div>
</template>

<style scoped>
.report-head {
  display: flex;
  gap: 20px;
  margin-bottom: 20px;
}
.radar-panel {
  flex: 1;
  display: flex;
  gap: 12px;
}
.radar-wrap {
  width: 300px;
  height: 300px;
  flex-shrink: 0;
}
</style>