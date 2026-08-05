<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { getSnapshot, setControl } from './bridge'

const mass = ref(18)
const count = ref(3)
const connected = ref(false)
const dropping = ref(false)
let pollTimer: number | undefined
let dropSequence = 0

const densityRatio = computed(() => mass.value / 14.1)
const behavior = computed(() => {
  if (densityRatio.value < 0.9) return 'Floats high'
  if (densityRatio.value <= 1.1) return 'Near neutral'
  if (densityRatio.value < 3.2) return 'Sinks slowly'
  return 'Sinks fast'
})

function commit(name: string, value: number) {
  void setControl(name, value).catch(() => {
    connected.value = false
  })
}

function updateMass(event: Event) {
  mass.value = Number((event.target as HTMLInputElement).value)
  commit('water.sphere_mass_g', mass.value)
}

function updateCount(value: number) {
  count.value = value
  commit('water.sphere_count', value)
}

function dropSpheres() {
  dropping.value = true
  dropSequence += 1
  commit('water.drop_spheres', dropSequence)
  window.setTimeout(() => {
    dropping.value = false
  }, 420)
}

async function pollSnapshot() {
  try {
    const snapshot = await getSnapshot()
    connected.value = snapshot.connected
    mass.value = snapshot.values['water.sphere_mass_g'] ?? mass.value
    count.value = snapshot.values['water.sphere_count'] ?? count.value
  } catch {
    connected.value = false
  }
}

onMounted(() => {
  void pollSnapshot()
  pollTimer = window.setInterval(pollSnapshot, 300)
})

onBeforeUnmount(() => {
  if (pollTimer !== undefined) window.clearInterval(pollTimer)
})
</script>

<template>
  <main class="panel-shell">
    <header class="app-bar">
      <div class="identity">
        <div class="sphere-mark" aria-hidden="true"></div>
        <div>
          <h1>Sphere drop</h1>
          <p>Water displacement controls</p>
        </div>
      </div>
      <span class="status" :class="{ live: connected }">
        <i></i>{{ connected ? 'Live' : 'Waiting' }}
      </span>
    </header>

    <section class="mass-readout" aria-live="polite">
      <div>
        <span>Mass per sphere</span>
        <strong>{{ mass.toFixed(0) }}<small>g</small></strong>
      </div>
      <div class="behavior">
        <span>{{ behavior }}</span>
        <small>{{ densityRatio.toFixed(2) }}× water density</small>
      </div>
    </section>

    <section class="controls" aria-label="Sphere controls">
      <label for="mass-slider">
        <span>Sphere mass</span>
        <output>{{ mass.toFixed(0) }} g</output>
      </label>
      <input
        id="mass-slider"
        :value="mass"
        type="range"
        min="2"
        max="120"
        step="1"
        @input="updateMass"
      />
      <div class="range-labels"><span>2 g</span><span>neutral 14.1 g</span><span>120 g</span></div>

      <div class="count-heading">
        <span>Sphere count</span>
        <output>{{ count }}</output>
      </div>
      <div class="count-control" role="group" aria-label="Sphere count">
        <button
          v-for="value in 5"
          :key="value"
          type="button"
          :class="{ selected: count === value }"
          :aria-pressed="count === value"
          @click="updateCount(value)"
        >
          {{ value }}
        </button>
      </div>
    </section>

    <button class="drop-button" :class="{ active: dropping }" type="button" @click="dropSpheres">
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <circle cx="12" cy="7" r="3.5" />
        <path d="M12 11.5v6m-3-3 3 3 3-3" />
      </svg>
      Drop {{ count === 1 ? 'sphere' : `${count} spheres` }}
    </button>

    <footer>
      Identical 3 cm spheres · mass changes buoyancy, impact, and settling depth
    </footer>
  </main>
</template>
