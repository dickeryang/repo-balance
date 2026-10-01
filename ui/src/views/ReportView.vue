<script setup lang="ts">
import { useScanStore } from '../stores/scan'
import { useI18n } from '../i18n'
import { useFullscreen } from '../composables/useFullscreen'
import ScoreCard from './report/ScoreCard.vue'
import RadarChart from '../components/RadarChart.vue'
import DimLegend from './report/DimLegend.vue'
import FindingList from './report/FindingList.vue'
import ExportRow from './report/ExportRow.vue'

const store = useScanStore()
const { t } = useI18n()
const { el: radarPanel, isFullscreen, toggle: toggleFullscreen } = useFullscreen()
</script>

<template>
  <div class="report-toolbar">
    <el-button size="small" round @click="store.resetToConnect()">
      {{ t('report.back') }}
    </el-button>
    <div class="spacer"></div>
    <ExportRow />
  </div>
  <div class="report-head">
    <ScoreCard />
    <div ref="radarPanel" class="card radar-panel" :class="{ 'is-fullscreen': isFullscreen }">
      <el-button
        class="fullscreen-btn"
        size="small"
        circle
        :title="isFullscreen ? t('report.exitFullscreen') : t('report.fullscreen')"
        :aria-label="isFullscreen ? t('report.exitFullscreen') : t('report.fullscreen')"
        @click="toggleFullscreen"
      >
        {{ isFullscreen ? '✕' : '⛶' }}
      </el-button>
      <div class="radar-wrap">
        <RadarChart v-if="store.report" :points="store.report.radarPoints" />
      </div>
      <DimLegend />
    </div>
  </div>
  <FindingList />
</template>

<style scoped>
.report-toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 16px;
}
.report-toolbar .spacer {
  flex: 1;
}
.report-head {
  display: flex;
  gap: 20px;
  margin-bottom: 20px;
}
.radar-panel {
  flex: 1;
  display: flex;
  gap: 12px;
  position: relative;
}
.radar-wrap {
  width: 300px;
  height: 300px;
  flex-shrink: 0;
}
.fullscreen-btn {
  position: absolute;
  top: 10px;
  right: 10px;
  z-index: 10;
}
/* 全屏模式：雷达图与图例居中放大铺满屏幕 */
.radar-panel.is-fullscreen {
  background: var(--bg);
  align-items: center;
  justify-content: center;
  gap: 48px;
}
.radar-panel.is-fullscreen .radar-wrap {
  width: min(70vh, 70vw);
  height: min(70vh, 70vw);
}
.radar-panel.is-fullscreen .radar-wrap :deep(svg) {
  width: 100%;
  height: 100%;
}

/* 窗口放大：评分卡与雷达面板间距加大，雷达图随空间放大 */
@media (min-width: 1440px) {
  .radar-wrap {
    width: 340px;
    height: 340px;
  }
}
@media (min-width: 1920px) {
  .radar-wrap {
    width: 380px;
    height: 380px;
  }
}
/* 窗口较窄：报告头部换行，雷达图收缩 */
@media (max-width: 900px) {
  .report-head {
    flex-wrap: wrap;
  }
  .score-card {
    width: 100%;
  }
  .radar-panel {
    flex-direction: column;
    align-items: center;
  }
  .radar-wrap {
    width: min(260px, 70vw);
    height: min(260px, 70vw);
  }
}
</style>
