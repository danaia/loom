<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { getSnapshot, setControl } from './bridge'

const density = ref(0.5)
const planeScale = ref(1)
const amplification = ref(0.25)
const fps = ref(0)
const gpuMemory = ref(0)
const connected = ref(false)
const resetActive = ref(false)
let pollTimer: number | undefined

const particleCount = computed(() => {
  const width = Math.min(202, 64 + Math.round(density.value * (202 - 64)))
  const height = Math.max(1, Math.min(152, Math.round((width * 152) / 202)))
  return width * height
})

const amplificationLabel = computed(() => `${(1 + amplification.value * 5).toFixed(1)}×`)

function updateControl(name: string, value: number) {
  void setControl(name, value).catch(() => {
    connected.value = false
  })
}

function updateDensity(event: Event) {
  density.value = Number((event.target as HTMLInputElement).value)
  updateControl('interaction.water_density', density.value)
}

function updatePlaneScale(event: Event) {
  planeScale.value = Number((event.target as HTMLInputElement).value)
  updateControl('interaction.plane_scale', planeScale.value)
}

function updateAmplification(event: Event) {
  amplification.value = Number((event.target as HTMLInputElement).value)
  updateControl('interaction.water_amplification', amplification.value)
}

async function resetScene() {
  resetActive.value = true
  updateControl('interaction.reset_scene', 1)
  window.setTimeout(() => {
    resetActive.value = false
  }, 420)
}

async function pollSnapshot() {
  try {
    const snapshot = await getSnapshot()
    connected.value = snapshot.connected
    density.value = snapshot.values['interaction.water_density'] ?? density.value
    planeScale.value = snapshot.values['interaction.plane_scale'] ?? planeScale.value
    amplification.value =
      snapshot.values['interaction.water_amplification'] ?? amplification.value
    fps.value = snapshot.values['interaction.hud_fps'] ?? fps.value
    gpuMemory.value = snapshot.values['interaction.hud_gpu_mb'] ?? gpuMemory.value
  } catch {
    connected.value = false
  }
}

onMounted(() => {
  void pollSnapshot()
  pollTimer = window.setInterval(pollSnapshot, 250)
})

onBeforeUnmount(() => {
  if (pollTimer !== undefined) window.clearInterval(pollTimer)
})
</script>

<template>
  <main class="panel-shell">
    <header class="hero">
      <div class="orb" aria-hidden="true">
        <span class="orb__glint"></span>
      </div>
      <div class="hero__copy">
        <p class="eyebrow">LOOM EXPERIMENT</p>
        <h1>Marble Water</h1>
        <p class="subtitle">A live, GPU-resident surface</p>
      </div>
      <div class="connection" :class="{ 'connection--online': connected }">
        <span class="connection__dot"></span>
        {{ connected ? 'LIVE' : 'WAIT' }}
      </div>
    </header>

    <section class="metrics" aria-label="Viewer performance">
      <article class="metric">
        <span class="metric__label">PRESENT</span>
        <strong>{{ Math.round(fps) }}</strong>
        <span class="metric__unit">FPS</span>
      </article>
      <article class="metric">
        <span class="metric__label">METAL</span>
        <strong>{{ Math.round(gpuMemory) }}</strong>
        <span class="metric__unit">MiB</span>
      </article>
      <div class="metric-wave" aria-hidden="true">
        <i v-for="index in 18" :key="index" :style="{ '--i': index }"></i>
      </div>
    </section>

    <section class="control-stack" aria-label="Water controls">
      <article class="control control--cyan">
        <div class="control__heading">
          <div>
            <p class="control__kicker">SURFACE</p>
            <h2>Particle density</h2>
          </div>
          <output>{{ particleCount.toLocaleString() }}</output>
        </div>
        <input
          aria-label="Water particle density"
          type="range"
          min="0"
          max="1"
          step="0.01"
          :value="density"
          :style="{ '--progress': `${density * 100}%` }"
          @input="updateDensity"
        />
        <div class="range-legend"><span>3,072</span><span>30,704 particles</span></div>
      </article>

      <article class="control control--violet">
        <div class="control__heading">
          <div>
            <p class="control__kicker">DOMAIN</p>
            <h2>Plane scale</h2>
          </div>
          <output>{{ planeScale.toFixed(2) }}×</output>
        </div>
        <input
          aria-label="Water plane scale"
          type="range"
          min="1"
          max="3"
          step="0.01"
          :value="planeScale"
          :style="{ '--progress': `${((planeScale - 1) / 2) * 100}%` }"
          @input="updatePlaneScale"
        />
        <div class="range-legend"><span>1× compact</span><span>3× wide</span></div>
      </article>

      <article class="control control--amber">
        <div class="control__heading">
          <div>
            <p class="control__kicker">ENERGY</p>
            <h2>Ripple intensity</h2>
          </div>
          <output>{{ amplificationLabel }}</output>
        </div>
        <input
          aria-label="Ripple intensity"
          type="range"
          min="0"
          max="1"
          step="0.01"
          :value="amplification"
          :style="{ '--progress': `${amplification * 100}%` }"
          @input="updateAmplification"
        />
        <div class="range-legend"><span>1× calm</span><span>6× kinetic</span></div>
      </article>
    </section>

    <button class="reset" :class="{ 'reset--active': resetActive }" @click="resetScene">
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path d="M20 11a8 8 0 1 0-2.35 5.65M20 4v7h-7" />
      </svg>
      <span>
        <b>Reset simulation</b>
        <small>Restore marbles and calm the surface</small>
      </span>
    </button>

    <footer class="interaction-guide">
      <p class="control__kicker">VIEWER INPUT</p>
      <div class="guide-row">
        <div class="keys" aria-label="W A S D keys">
          <kbd>W</kbd><kbd>A</kbd><kbd>S</kbd><kbd>D</kbd>
        </div>
        <p><strong>Steer</strong><span>WASD or arrow keys</span></p>
      </div>
      <div class="guide-row">
        <div class="mouse" aria-hidden="true"><i></i></div>
        <p><strong>Lift &amp; drop</strong><span>Drag marble · scroll for height</span></p>
      </div>
    </footer>
  </main>
</template>
