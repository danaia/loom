<script setup lang="ts">
import type { Phase } from '../types'

defineProps<{ phase: Phase; tick: number; population: number; stability: number }>()
const phases: Phase[] = ['observe', 'propose', 'validate', 'commit']
</script>

<template>
  <footer class="phase-rail">
    <section class="phase-sequence">
      <span class="metric-label">Phase</span>
      <div>
        <template v-for="(item, index) in phases" :key="item">
          <strong :class="{ active: item === phase }">{{ item }}</strong>
          <i v-if="index < phases.length - 1" aria-hidden="true">→</i>
        </template>
      </div>
    </section>
    <section><span class="metric-label">Tick</span><strong>{{ tick.toLocaleString() }}</strong></section>
    <section><span class="metric-label">Population</span><strong>{{ population }}</strong></section>
    <section class="stability"><span class="metric-label">Stability</span><strong>{{ stability.toFixed(2) }}</strong><i><b :style="{ width: `${stability * 100}%` }" /></i></section>
  </footer>
</template>
