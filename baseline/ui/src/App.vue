<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import {
  Badge as ABadge,
  Button as AButton,
  ConfigProvider as AConfigProvider,
  Input as AInput,
  Progress as AProgress,
  Select as ASelect,
  Slider as ASlider,
  theme,
} from 'ant-design-vue'
import { ReloadOutlined } from '@ant-design/icons-vue'
import { getSnapshot, openAgentsWindow, setControl } from './bridge'
import {
  loadParticleAgents,
  saveParticleAgents,
  uniqueAgentName,
} from './agentRoster'

const fps = ref(0)
const gpuMemory = ref(0)
const gpuFrameTime = ref(0)
const gpuBudget = ref(1000 / 120)
const gpuPressure = ref(0)
const spaceDrag = ref(0)
const agentType = ref(0)
const particleAgents = ref(loadParticleAgents())
const agentCount = ref(particleAgents.value.length)
const agentName = ref(
  uniqueAgentName(`General ${agentCount.value + 1}`, particleAgents.value),
)
const connected = ref(false)
let pollTimer: number | undefined

const { compactAlgorithm, darkAlgorithm } = theme
const panelTheme = {
  algorithm: [darkAlgorithm, compactAlgorithm],
  token: {
    colorPrimary: '#26a8e8',
    colorInfo: '#26a8e8',
    colorSuccess: '#89d185',
    colorBgBase: '#151719',
    colorBgContainer: '#1d2023',
    colorBgElevated: '#24282c',
    colorBorder: '#343a40',
    colorText: '#d7dadd',
    colorTextSecondary: '#8e969e',
    fontSize: 12,
    controlHeight: 26,
    borderRadius: 3,
  },
}

const gpuPercent = computed(() => Math.min(100, Math.max(0, gpuPressure.value)))
const pressureColor = computed(() => {
  if (gpuPressure.value >= 100) return '#f14c4c'
  if (gpuPressure.value >= 70) return '#cca700'
  return '#89d185'
})
const agentTypeNames = ['General', 'Scout', 'Builder']
const agentTypeName = computed(() => agentTypeNames[agentType.value] ?? 'General')
const agentNameValid = computed(() => {
  const candidate = agentName.value.trim().toLocaleLowerCase()
  return candidate.length > 0 &&
    candidate.length <= 32 &&
    !particleAgents.value.some((agent) => agent.name.toLocaleLowerCase() === candidate)
})

function commitControl(name: string, value: number) {
  void setControl(name, value).catch(() => {
    connected.value = false
  })
}

function commitDrag(value: number | number[]) {
  commitControl('interaction.space_drag', typeof value === 'number' ? value : value[0])
}

function commitAgentType(value: unknown) {
  if (typeof value !== 'number') return
  agentType.value = value
  commitControl('interaction.agent_type', value)
}

function reconcileParticleAgents(nextCount: number) {
  const count = Math.max(1, Math.min(32, Math.round(nextCount)))
  const countChanged =
    count !== agentCount.value || count !== particleAgents.value.length
  if (!countChanged) return

  if (count < particleAgents.value.length) {
    particleAgents.value = particleAgents.value.slice(0, count)
  }
  while (particleAgents.value.length < count) {
    const preferred = particleAgents.value.length === agentCount.value
      ? agentName.value
      : agentTypeName.value
    particleAgents.value.push({
      id: particleAgents.value.length,
      name: uniqueAgentName(preferred, particleAgents.value),
      type: agentTypeName.value,
    })
  }
  agentCount.value = count
  saveParticleAgents(particleAgents.value)
  agentName.value = uniqueAgentName(
    `${agentTypeName.value} ${count + 1}`,
    particleAgents.value,
  )
}

function resetParticle() {
  commitControl('interaction.reset', 1)
}

function openAgents() {
  void openAgentsWindow().catch(() => {
    connected.value = false
  })
}

async function pollSnapshot() {
  try {
    const snapshot = await getSnapshot()
    connected.value = snapshot.connected
    spaceDrag.value = snapshot.values['interaction.space_drag'] ?? spaceDrag.value
    fps.value = snapshot.values['interaction.hud_fps'] ?? fps.value
    gpuMemory.value = snapshot.values['interaction.hud_gpu_mb'] ?? gpuMemory.value
    gpuFrameTime.value =
      snapshot.values['interaction.hud_gpu_frame_ms'] ?? gpuFrameTime.value
    gpuBudget.value =
      snapshot.values['interaction.hud_gpu_budget_ms'] ?? gpuBudget.value
    gpuPressure.value =
      snapshot.values['interaction.hud_gpu_pressure'] ?? gpuPressure.value
    reconcileParticleAgents(snapshot.values['interaction.agent_count'] ?? agentCount.value)
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
    <main class="panel">
      <header class="app-bar">
        <div class="identity">
          <span class="particle-mark" aria-hidden="true"></span>
          <div>
            <h1>Loom Baseline</h1>
            <p>Selectable particles · zero gravity</p>
          </div>
        </div>
        <a-badge
          :status="connected ? 'success' : 'default'"
          :text="connected ? 'Live' : 'Waiting'"
        />
      </header>

      <section class="telemetry" aria-label="Runtime telemetry">
        <div class="metrics">
          <div><span>FPS</span><strong>{{ Math.round(fps) }}</strong></div>
          <div><span>GPU memory</span><strong>{{ Math.round(gpuMemory) }}<small> MiB</small></strong></div>
          <div><span>Add agent</span><strong>⌘ Click</strong></div>
        </div>
        <div class="pressure">
          <div>
            <span>GPU frame</span>
            <strong :style="{ color: pressureColor }">{{ gpuFrameTime.toFixed(2) }} ms</strong>
          </div>
          <a-progress
            :percent="gpuPercent"
            :show-info="false"
            :stroke-color="pressureColor"
            trail-color="#343a40"
            :stroke-width="5"
            
          />
          <p class="smallp">{{ Math.round(gpuPressure) }}% of {{ gpuBudget.toFixed(2) }} ms budget</p>
        </div>
      </section>

      <section class="card">
        <header>
          <div>
            <p>PHYSICS</p>
            <h2>Vacuum</h2>
          </div>
          <span>gravity 0 m/s²</span>
        </header>
        <div class="control">
          <div>
            <label for="agent-type">New agent type</label>
            <output>{{ agentTypeName }}</output>
          </div>
          <a-select
            id="agent-type"
            :value="agentType"
            style="width: 100%"
            :options="[
              { value: 0, label: 'General' },
              { value: 1, label: 'Scout' },
              { value: 2, label: 'Builder' },
            ]"
            @change="commitAgentType"
          />
          <a-input
            v-model:value="agentName"
            class="agent-name"
            :maxlength="32"
            placeholder="Unique agent name"
            :status="agentNameValid ? undefined : 'error'"
          />
          <p>
            <span>{{ agentNameValid ? 'Unique name' : 'Will add a unique suffix' }}</span>
            <span>⌘ Click space to add</span>
          </p>
        </div>
        <div class="control">
          <div>
            <label for="space-drag">Space drag</label>
            <output>{{ spaceDrag.toFixed(2) }} s⁻¹</output>
          </div>
          <a-slider
            id="space-drag"
            v-model:value="spaceDrag"
            :min="0"
            :max="0.5"
            :step="0.01"
            @change="commitDrag"
          />
          <p><span>vacuum</span><span>damped</span></p>
        </div>
        <a-button block class="reset" @click="resetParticle">
          <template #icon><reload-outlined /></template>
          Reset particle
        </a-button>
        <a-button block class="agents" @click="openAgents">
          Agents
        </a-button>
        <p class="interaction-hint">Drag directly · select, then click space to target</p>
      </section>

    </main>
  </a-config-provider>
</template>
