<script setup lang="ts">
import type { RadarPoint } from '../../types/models'

const props = defineProps<{ points: RadarPoint[] }>()

const CX = 150
const CY = 150
const R = 105

function angleFor(index: number): number {
  return -Math.PI / 2 + (index * 2 * Math.PI) / 5
}

function vertex(index: number, radius: number): { x: number; y: number } {
  const a = angleFor(index)
  return { x: CX + radius * Math.cos(a), y: CY + radius * Math.sin(a) }
}

function gridPolygon(radius: number): string {
  return Array.from({ length: 5 }, (_, i) => {
    const v = vertex(i, radius)
    return `${v.x},${v.y}`
  }).join(' ')
}

function scorePolygon(): string {
  return props.points
    .map((p, i) => {
      const score = p.score ?? 0
      const radius = (score / 100) * R
      const v = vertex(i, radius)
      return `${v.x},${v.y}`
    })
    .join(' ')
}

function labelPos(index: number): { x: number; y: number } {
  return vertex(index, R + 18)
}

/** 五个维度顶点方向的轴线端点（index 0-4，顶部轴线对应 index 0）。 */
const axisPoints = Array.from({ length: 5 }, (_, i) => vertex(i, R))
</script>

<template>
  <svg width="300" height="300" viewBox="0 0 300 300">
    <polygon v-for="r in [0.25, 0.5, 0.75, 1.0]" :key="r" :points="gridPolygon(R * r)" fill="none" stroke="var(--border)" stroke-width="1" />
    <line v-for="(v, i) in axisPoints" :key="i" :x1="CX" :y1="CY" :x2="v.x" :y2="v.y" stroke="var(--border)" stroke-width="1" />
    <polygon :points="scorePolygon()" fill="rgba(79,140,255,0.25)" stroke="var(--accent)" stroke-width="2" />
    <circle v-for="(p, i) in points" :key="i" :cx="vertex(i, (p.score ?? 0) / 100 * R).x" :cy="vertex(i, (p.score ?? 0) / 100 * R).y" r="3" fill="var(--accent)" />
    <text v-for="(p, i) in points" :key="'label-' + i" :x="labelPos(i).x" :y="labelPos(i).y" text-anchor="middle" dominant-baseline="middle" fill="var(--text-dim)" font-size="11">
      {{ p.name }} {{ p.score ?? 'N/A' }}
    </text>
  </svg>
</template>