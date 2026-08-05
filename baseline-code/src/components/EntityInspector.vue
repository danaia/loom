<script setup lang="ts">
import type { Entity, GrowthEvent } from '../types'

defineProps<{
  entity: Entity | null
  connections: number
  events: GrowthEvent[]
}>()
</script>

<template>
  <aside class="rail inspector-rail">
    <header class="section-heading">
      <h2>Selected entity</h2>
      <span class="target-mark" aria-hidden="true" />
    </header>

    <section v-if="entity" class="entity-details">
      <div><span>role</span><strong><i :class="`glyph glyph-${entity.kind}`" />{{ entity.kind }}</strong></div>
      <div><span>id</span><strong>{{ entity.id }}</strong></div>
      <div><span>name</span><strong>{{ entity.name }}</strong></div>
      <div class="meter-row"><span>energy</span><strong>{{ entity.energy.toFixed(2) }}</strong><i><b :style="{ width: `${entity.energy * 100}%` }" /></i></div>
      <div class="meter-row"><span>confidence</span><strong>{{ entity.confidence.toFixed(2) }}</strong><i><b :style="{ width: `${entity.confidence * 100}%` }" /></i></div>
      <div><span>connections</span><strong>{{ connections }}</strong></div>
    </section>

    <section class="growth-log">
      <header><h2>Growth log</h2><span>latest {{ Math.min(events.length, 9) }}</span></header>
      <ol>
        <li v-for="item in [...events].reverse().slice(0, 9)" :key="item.id">
          <time>{{ item.tick.toLocaleString() }}</time>
          <i :class="item.kind === 'system' ? 'system-dot' : `glyph glyph-${item.kind}`" />
          <span>{{ item.message }}</span>
        </li>
      </ol>
    </section>
  </aside>
</template>
