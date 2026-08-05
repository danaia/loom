<script setup lang="ts">
import { onBeforeUnmount, onMounted } from 'vue'
import { storeToRefs } from 'pinia'
import RuleField from './components/RuleField.vue'
import SystemCanvas from './components/SystemCanvas.vue'
import EntityInspector from './components/EntityInspector.vue'
import PhaseRail from './components/PhaseRail.vue'
import { useEmergenceStore } from './stores/emergence'

const store = useEmergenceStore()
const { rules, entities, links, events, tick, stability, selectedId, selected, selectedConnections, running, loading, phase } = storeToRefs(store)
let timer: number | undefined

function toggleRun() { running.value = !running.value }

onMounted(async () => {
  await store.initialize()
  timer = window.setInterval(() => { if (running.value) store.step() }, 560)
})
onBeforeUnmount(() => { if (timer !== undefined) window.clearInterval(timer) })
</script>

<template>
  <main class="app-shell" :class="{ loading }">
    <header class="top-bar">
      <div class="brand"><i aria-hidden="true"><span /><span /><span /><span /></i><h1><strong>PQO</strong><b>/</b> Emergent code</h1></div>
      <div class="runtime-state"><i :class="{ paused: !running }" /><span>{{ running ? 'Simulation active' : 'Simulation paused' }}</span></div>
      <nav aria-label="Simulation controls">
        <button @click="running = true" :disabled="running"><span aria-hidden="true">▶</span> Run</button>
        <button :class="{ active: running }" @click="toggleRun"><span aria-hidden="true">Ⅱ</span> {{ running ? 'Pause' : 'Resume' }}</button>
        <button @click="store.step"><span aria-hidden="true">→</span> Step</button>
        <button @click="store.reset"><span aria-hidden="true">↻</span> Reset</button>
      </nav>
    </header>

    <div class="workspace">
      <RuleField :rules="rules" @change="store.updateRule" />
      <SystemCanvas :entities="entities" :links="links" :selected-id="selectedId" :running="running" @select="selectedId = $event" />
      <EntityInspector :entity="selected" :connections="selectedConnections" :events="events" />
    </div>

    <PhaseRail :phase="phase" :tick="tick" :population="entities.length" :stability="stability" />
    <div v-if="loading" class="loading-screen">Reading PQO rule graph…</div>
  </main>
</template>
