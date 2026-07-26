<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import {
  Badge as ABadge,
  Button as AButton,
  ConfigProvider as AConfigProvider,
  Divider as ADivider,
  Slider as ASlider,
  Tooltip as ATooltip,
  theme,
} from 'ant-design-vue'
import {
  AimOutlined,
  QuestionCircleOutlined,
  ReloadOutlined,
} from '@ant-design/icons-vue'
import { getSnapshot, setControl } from './bridge'

const density = ref(0.5)
const planeScale = ref(1)
const amplification = ref(0.25)
const playerSpeed = ref(1)
const fps = ref(0)
const gpuMemory = ref(0)
const connected = ref(false)
const resetActive = ref(false)
let pollTimer: number | undefined

const { compactAlgorithm, darkAlgorithm } = theme

const panelTheme = {
  algorithm: [darkAlgorithm, compactAlgorithm],
  token: {
    colorPrimary: '#57c7f3',
    colorInfo: '#57c7f3',
    colorSuccess: '#56d6a3',
    colorBgBase: '#080c10',
    colorBgContainer: '#0f151b',
    colorBgElevated: '#151c23',
    colorBorder: '#26313a',
    colorText: '#dce7ed',
    colorTextSecondary: '#7f919c',
    fontSize: 12,
    controlHeight: 26,
    borderRadius: 6,
    wireframe: false,
  },
  components: {
    Button: {
      defaultBg: '#121a20',
      defaultBorderColor: '#2b3943',
      defaultColor: '#dce7ed',
    },
    Slider: {
      railBg: '#28323a',
      railHoverBg: '#34414b',
      trackBg: '#57c7f3',
      trackHoverBg: '#72d4f8',
      handleColor: '#57c7f3',
      handleActiveColor: '#8cdefb',
      dotSize: 4,
      handleSize: 9,
      handleSizeHover: 11,
      railSize: 3,
    },
  },
}

const particleCount = computed(() => {
  const width = Math.min(202, 64 + Math.round(density.value * (202 - 64)))
  const height = Math.max(1, Math.min(152, Math.round((width * 152) / 202)))
  return width * height
})

const amplificationLabel = computed(() => `${(1 + amplification.value * 5).toFixed(1)}×`)
const playerSpeedLabel = computed(() => `${playerSpeed.value.toFixed(2)}×`)

function commitControl(name: string, value: number) {
  void setControl(name, value).catch(() => {
    connected.value = false
  })
}

function scalarValue(value: number | number[]) {
  return typeof value === 'number' ? value : value[0]
}

function commitDensity(value: number | number[]) {
  commitControl('interaction.water_density', scalarValue(value))
}

function commitPlaneScale(value: number | number[]) {
  commitControl('interaction.plane_scale', scalarValue(value))
}

function commitAmplification(value: number | number[]) {
  commitControl('interaction.water_amplification', scalarValue(value))
}

function commitPlayerSpeed(value: number | number[]) {
  commitControl('interaction.player_speed', scalarValue(value))
}

function resetScene() {
  resetActive.value = true
  commitControl('interaction.reset_scene', 1)
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
    playerSpeed.value = snapshot.values['interaction.player_speed'] ?? playerSpeed.value
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
  <a-config-provider :theme="panelTheme">
    <main class="panel-shell">
      <header class="app-bar">
        <div class="app-identity">
          <div class="water-mark" aria-hidden="true"><i></i></div>
          <div>
            <h1>Marble Water</h1>
            <p>Simulation controls</p>
          </div>
        </div>

        <a-badge
          class="connection-badge"
          :status="connected ? 'success' : 'default'"
          :text="connected ? 'Live' : 'Waiting'"
        />
      </header>

      <section class="telemetry" aria-label="Viewer performance">
        <div class="telemetry__item">
          <span>Frame rate</span>
          <strong>{{ Math.round(fps) }}<small>fps</small></strong>
        </div>
        <a-divider type="vertical" />
        <div class="telemetry__item">
          <span>GPU memory</span>
          <strong>{{ Math.round(gpuMemory) }}<small>MiB</small></strong>
        </div>
        <div class="telemetry__pulse" :class="{ 'telemetry__pulse--live': connected }">
          <i v-for="index in 8" :key="index" :style="{ '--i': index }"></i>
        </div>
      </section>

      <section class="control-panel" aria-labelledby="simulation-heading">
        <div class="section-heading">
          <div>
            <p class="overline">SIMULATION</p>
            <h2 id="simulation-heading">Water surface</h2>
          </div>
          <span>4 parameters</span>
        </div>

        <div class="control-list">
          <article class="control-row">
            <div class="control-row__heading">
              <div class="control-label">
                <span>Particle density</span>
                <a-tooltip title="Number of samples used to resolve the water surface.">
                  <question-circle-outlined />
                </a-tooltip>
              </div>
              <output>{{ particleCount.toLocaleString() }}<small> particles</small></output>
            </div>
            <a-slider
              v-model:value="density"
              :min="0"
              :max="1"
              :step="0.01"
              :tooltip="{ formatter: null }"
              aria-label="Water particle density"
              @change="commitDensity"
            />
            <div class="range-legend"><span>3,072</span><span>30,704</span></div>
          </article>

          <article class="control-row">
            <div class="control-row__heading">
              <div class="control-label">
                <span>Plane scale</span>
                <a-tooltip title="Changes the horizontal size of the simulation domain.">
                  <question-circle-outlined />
                </a-tooltip>
              </div>
              <output>{{ planeScale.toFixed(2) }}×</output>
            </div>
            <a-slider
              v-model:value="planeScale"
              :min="1"
              :max="3"
              :step="0.01"
              :tooltip="{ formatter: null }"
              aria-label="Water plane scale"
              @change="commitPlaneScale"
            />
            <div class="range-legend"><span>Compact</span><span>Wide</span></div>
          </article>

          <article class="control-row">
            <div class="control-row__heading">
              <div class="control-label">
                <span>Ripple intensity</span>
                <a-tooltip title="Amplifies displacement as ripples move through the water.">
                  <question-circle-outlined />
                </a-tooltip>
              </div>
              <output>{{ amplificationLabel }}</output>
            </div>
            <a-slider
              v-model:value="amplification"
              :min="0"
              :max="1"
              :step="0.01"
              :tooltip="{ formatter: null }"
              aria-label="Ripple intensity"
              @change="commitAmplification"
            />
            <div class="range-legend"><span>Calm</span><span>Kinetic</span></div>
          </article>

          <article class="control-row">
            <div class="control-row__heading">
              <div class="control-label">
                <span>Movement speed</span>
                <a-tooltip title="Controls how quickly the player marble moves.">
                  <question-circle-outlined />
                </a-tooltip>
              </div>
              <output>{{ playerSpeedLabel }}</output>
            </div>
            <a-slider
              v-model:value="playerSpeed"
              :min="0.25"
              :max="3"
              :step="0.05"
              :tooltip="{ formatter: null }"
              aria-label="Player movement speed"
              @change="commitPlayerSpeed"
            />
            <div class="range-legend"><span>Drift</span><span>Fast wake</span></div>
          </article>
        </div>
      </section>

      <a-button class="reset-button" block :loading="resetActive" @click="resetScene">
        <template #icon><reload-outlined /></template>
        Reset simulation
      </a-button>

      <footer class="input-guide">
        <div class="section-heading section-heading--compact">
          <div>
            <p class="overline">VIEWER INPUT</p>
            <h2>Navigation</h2>
          </div>
        </div>
        <div class="shortcut-row">
          <div class="shortcut-row__keys" aria-label="W A S D keys">
            <kbd>W</kbd><kbd>A</kbd><kbd>S</kbd><kbd>D</kbd>
          </div>
          <span><strong>Steer</strong>WASD or arrow keys</span>
        </div>
        <div class="shortcut-row">
          <aim-outlined class="shortcut-row__icon" />
          <span><strong>Lift &amp; drop</strong>Drag marble · scroll for height</span>
        </div>
      </footer>
    </main>
  </a-config-provider>
</template>
