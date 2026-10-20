<script setup lang="ts">
import { computed, ref } from 'vue'
import type { TrendPoint } from '../types/models'
import { useI18n } from '../i18n'

const props = defineProps<{ points: TrendPoint[] }>()
const { t } = useI18n()

/** 画布 viewBox 尺寸（实际宽度由容器决定，等比缩放）。 */
const W = 760
const H = 260
const PAD = { left: 44, right: 18, top: 16, bottom: 30 }

/** 总分序列的键（维度键与 radarPoints.key 一致，故用不可能冲突的保留键）。 */
const TOTAL_KEY = '__total'

/**
 * 曲线序列：总分（加粗）+ 五维。
 * 维度色沿用评分/雷达图既有语义色，避免同一维度在两张图里换颜色。
 */
const SERIES: { key: string; color: string; width: number }[] = [
  { key: TOTAL_KEY, color: 'var(--text)', width: 2.6 },
  { key: 'structure', color: 'var(--accent)', width: 1.6 },
  { key: 'history', color: 'var(--info)', width: 1.6 },
  { key: 'branches', color: 'var(--warning)', width: 1.6 },
  { key: 'deps', color: 'var(--ok)', width: 1.6 },
  { key: 'security', color: 'var(--critical)', width: 1.6 },
]

const innerW = W - PAD.left - PAD.right
const innerH = H - PAD.top - PAD.bottom

/** 被图例隐藏的曲线键。 */
const hidden = ref<string[]>([])

const count = computed(() => props.points.length)

function labelOf(key: string): string {
  return key === TOTAL_KEY ? t('history.trendTotal') : t(`filter.${key}`)
}

/** 取某次扫描在某序列上的值；null = 该维度当次未覆盖。 */
function valueOf(point: TrendPoint, key: string): number | null {
  return key === TOTAL_KEY ? point.total : (point.dims[key] ?? null)
}

function xAt(index: number): number {
  if (count.value <= 1) return PAD.left + innerW / 2
  return PAD.left + (index * innerW) / (count.value - 1)
}

function yAt(score: number): number {
  return PAD.top + (1 - score / 100) * innerH
}

/** 一条曲线的连续段：N/A 处断线，不做插值（插值等于伪造未覆盖维度的结论）。 */
function segmentsOf(key: string): string[] {
  const out: string[] = []
  let run: string[] = []
  props.points.forEach((point, i) => {
    const value = valueOf(point, key)
    if (value === null) {
      if (run.length > 0) out.push(run.join(' '))
      run = []
      return
    }
    run.push(`${xAt(i).toFixed(1)},${yAt(value).toFixed(1)}`)
  })
  if (run.length > 0) out.push(run.join(' '))
  return out
}

function dotsOf(key: string): { cx: number; cy: number; tip: string }[] {
  return props.points.flatMap((point, i) => {
    const value = valueOf(point, key)
    if (value === null) return []
    return [
      {
        cx: xAt(i),
        cy: yAt(value),
        tip: `${fmtFull(point.savedAt)} · ${labelOf(key)}: ${value}`,
      },
    ]
  })
}

/** 图例里的「最新值」：取时间上最近的一个非 N/A 值。 */
function latestOf(key: string): number | null {
  for (let i = props.points.length - 1; i >= 0; i -= 1) {
    const value = valueOf(props.points[i], key)
    if (value !== null) return value
  }
  return null
}

function isHidden(key: string): boolean {
  return hidden.value.includes(key)
}

function toggle(key: string) {
  hidden.value = isHidden(key)
    ? hidden.value.filter((k) => k !== key)
    : [...hidden.value, key]
}

/** 只渲染可见曲线，段与点预先算好（模板里不做分支判断）。 */
const visibleSeries = computed(() =>
  SERIES.filter((s) => !isHidden(s.key)).map((s) => ({
    key: s.key,
    color: s.color,
    width: s.width,
    segments: segmentsOf(s.key),
    dots: dotsOf(s.key),
  })),
)

const yTicks = [0, 25, 50, 75, 100]

/** X 轴刻度：按点数抽稀（最多约 6 个），始终保留首尾。 */
const xTicks = computed<number[]>(() => {
  const n = count.value
  if (n === 0) return []
  const step = Math.max(1, Math.ceil(n / 6))
  const idx: number[] = []
  for (let i = 0; i < n; i += step) idx.push(i)
  if (idx[idx.length - 1] !== n - 1) idx.push(n - 1)
  return idx
})

function pad2(n: number): string {
  return String(n).padStart(2, '0')
}

/** X 轴标签：月-日（同一天多次扫描靠 tooltip 区分）。 */
function fmtDay(ms: number): string {
  const d = new Date(ms)
  return `${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`
}

/** tooltip 用完整时间：年-月-日 时:分。 */
function fmtFull(ms: number): string {
  const d = new Date(ms)
  return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())} ${pad2(d.getHours())}:${pad2(d.getMinutes())}`
}
</script>

<template>
  <div class="trend">
    <div class="legend">
      <button
        v-for="s in SERIES"
        :key="s.key"
        class="chip"
        :class="{ off: isHidden(s.key) }"
        :title="t('history.trendToggle')"
        @click="toggle(s.key)"
      >
        <span class="dot" :style="{ background: s.color }"></span>
        <span class="name">{{ labelOf(s.key) }}</span>
        <span class="val">{{ latestOf(s.key) ?? 'N/A' }}</span>
      </button>
    </div>

    <div class="canvas">
      <svg :viewBox="`0 0 ${W} ${H}`" role="img" :aria-label="t('history.trendTitle')">
        <g>
          <line
            v-for="tick in yTicks"
            :key="`grid-${tick}`"
            :x1="PAD.left"
            :y1="yAt(tick)"
            :x2="W - PAD.right"
            :y2="yAt(tick)"
            stroke="var(--border)"
            stroke-width="1"
            :stroke-dasharray="tick === 0 ? '' : '3 4'"
          />
          <text
            v-for="tick in yTicks"
            :key="`ylabel-${tick}`"
            :x="PAD.left - 8"
            :y="yAt(tick)"
            text-anchor="end"
            dominant-baseline="middle"
            fill="var(--text-dim)"
            font-size="11"
          >
            {{ tick }}
          </text>
        </g>

        <template v-for="s in visibleSeries" :key="s.key">
          <polyline
            v-for="(seg, si) in s.segments"
            :key="`${s.key}-seg-${si}`"
            :points="seg"
            fill="none"
            :stroke="s.color"
            :stroke-width="s.width"
            stroke-linejoin="round"
            stroke-linecap="round"
          />
          <circle
            v-for="(d, di) in s.dots"
            :key="`${s.key}-dot-${di}`"
            :cx="d.cx"
            :cy="d.cy"
            r="2.8"
            :fill="s.color"
          >
            <title>{{ d.tip }}</title>
          </circle>
        </template>

        <text
          v-for="i in xTicks"
          :key="`xtick-${i}`"
          :x="xAt(i)"
          :y="H - 10"
          text-anchor="middle"
          fill="var(--text-dim)"
          font-size="11"
        >
          {{ fmtDay(points[i].savedAt) }}
        </text>
      </svg>
    </div>
  </div>
</template>

<style scoped>
.trend {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.legend {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.chip {
  display: flex;
  align-items: center;
  gap: 6px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 999px;
  padding: 4px 10px;
  font-size: 11px;
  color: var(--text);
  cursor: pointer;
  transition:
    opacity 0.15s,
    border-color 0.15s;
}
.chip:hover {
  border-color: var(--accent);
}
.chip.off {
  opacity: 0.4;
}
.chip .dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}
.chip .name {
  color: var(--text-dim);
}
.chip .val {
  font-weight: 700;
  font-family: 'SF Mono', Menlo, monospace;
}
.canvas {
  width: 100%;
}
.canvas svg {
  width: 100%;
  height: auto;
  display: block;
}
</style>
