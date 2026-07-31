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
  defaultParticleAgent,
  loadParticleAgents,
  resetParticleAgents,
  saveParticleAgents,
  subscribeParticleAgents,
  uniqueAgentName,
} from './agentRoster'
import type { ParticleAgent } from './agentRoster'
import { particleFields } from './particlePanelSchema'

const fps = ref(0)
const gpuMemory = ref(0)
const gpuFrameTime = ref(0)
const gpuBudget = ref(1000 / 120)
const gpuPressure = ref(0)
const spaceDrag = ref(0)
const particleAgents = ref<ParticleAgent[]>([defaultParticleAgent(0)])
const agentCount = ref(particleAgents.value.length)
const selectedParticleId = ref(0)
const connected = ref(false)
const showResetConfirmation = ref(false)
const resetInProgress = ref(false)
let pollTimer: number | undefined
let rosterUnlisten: (() => void) | undefined

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
const agentTypeNames = ['General', 'Scout', 'Builder'] as const
const selectedParticle = computed(
  () => particleAgents.value.find((agent) => agent.id === selectedParticleId.value) ?? null,
)

function commitControl(name: string, value: number) {
  void setControl(name, value).catch(() => {
    connected.value = false
  })
}

function commitDrag(value: number | number[]) {
  commitControl('interaction.space_drag', typeof value === 'number' ? value : value[0])
}

function syncSpawnType(type: string) {
  const index = agentTypeNames.indexOf(type as typeof agentTypeNames[number])
  commitControl('interaction.agent_type', Math.max(0, index))
}

async function persistParticleAgents() {
  try {
    particleAgents.value = await saveParticleAgents(particleAgents.value)
  } catch {
    connected.value = false
  }
}

function updateSelectedField(key: string, value: unknown) {
  const particle = selectedParticle.value
  if (!particle) return
  if (key === 'name' && typeof value === 'string') particle.name = value.trim() || particle.name
  else if (key === 'type' && typeof value === 'string') {
    particle.type = value
    syncSpawnType(value)
  }
  else if (key === 'skills' && typeof value === 'string') {
    particle.skills = value.split(',').map((skill) => skill.trim()).filter(Boolean)
  } else if (!['id', 'name', 'type', 'skills'].includes(key)) {
    particle.fields[key] = value
  }
  void persistParticleAgents()
}

function selectedFieldValue(key: string): string | number {
  const particle = selectedParticle.value
  if (!particle) return ''
  if (key === 'id') return particle.id
  if (key === 'name') return particle.name
  if (key === 'type') return particle.type
  if (key === 'skills') return particle.skills.join(', ')
  const value = particle.fields[key]
  return typeof value === 'number' || typeof value === 'string' ? value : ''
}

function linkSelectedParticle() {
  if (!selectedParticle.value) return
  selectedParticle.value.agentLinked = true
  void persistParticleAgents()
}

function reconcileParticleAgents(nextCount: number, lowMask: number, highMask: number) {
  const count = Math.max(0, Math.min(32, Math.round(nextCount)))
  let changed = false
  for (let id = 0; id < 32; id += 1) {
    const mask = id < 16 ? lowMask : highMask
    const active = (Math.round(mask) & (1 << (id % 16))) !== 0
    if (!active) continue
    const existing = particleAgents.value.find((agent) => agent.id === id)
    if (!existing) {
      const type = selectedParticle.value?.type ?? 'General'
      particleAgents.value.push(
        defaultParticleAgent(
          id,
          uniqueAgentName(`${type} ${id + 1}`, particleAgents.value),
          type,
        ),
      )
      changed = true
    } else if (existing.fields.metalActive === false) {
      existing.fields.metalActive = true
      existing.agentLinked = true
      changed = true
    }
  }
  particleAgents.value.sort((left, right) => left.id - right.id)
  agentCount.value = count
  if (changed) void persistParticleAgents()
}

function wait(milliseconds: number) {
  return new Promise<void>((resolve) => window.setTimeout(resolve, milliseconds))
}

async function waitForRuntimeReset() {
  for (let attempt = 0; attempt < 20; attempt += 1) {
    const snapshot = await getSnapshot()
    const count = Math.round(snapshot.values['interaction.agent_count'] ?? -1)
    const lowMask = Math.round(snapshot.values['interaction.active_mask_low'] ?? -1)
    const highMask = Math.round(snapshot.values['interaction.active_mask_high'] ?? -1)
    if (count === 1 && lowMask === 1 && highMask === 0) {
      await wait(50)
      return
    }
    await wait(50)
  }
  throw new Error('The Metal view did not confirm its reset.')
}

async function resetToGroundZero() {
  if (resetInProgress.value) return
  resetInProgress.value = true
  try {
    await setControl('interaction.reset', 1)
    await waitForRuntimeReset()
    particleAgents.value = await resetParticleAgents()
    agentCount.value = 1
    selectedParticleId.value = 0
    showResetConfirmation.value = false
  } catch {
    connected.value = false
  } finally {
    resetInProgress.value = false
  }
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
    const nextSelectedParticleId = Math.max(
      0,
      Math.min(31, Math.round(snapshot.values['interaction.selected'] ?? selectedParticleId.value)),
    )
    if (nextSelectedParticleId !== selectedParticleId.value) {
      selectedParticleId.value = nextSelectedParticleId
      const type = particleAgents.value.find((agent) => agent.id === nextSelectedParticleId)?.type
      if (type) syncSpawnType(type)
    }
    if (!resetInProgress.value) {
      reconcileParticleAgents(
        snapshot.values['interaction.agent_count'] ?? agentCount.value,
        snapshot.values['interaction.active_mask_low'] ?? 1,
        snapshot.values['interaction.active_mask_high'] ?? 0,
      )
    }
  } catch {
    connected.value = false
  }
}

onMounted(async () => {
  try {
    rosterUnlisten = await subscribeParticleAgents((roster) => {
      particleAgents.value = roster
    })
    particleAgents.value = await loadParticleAgents()
    agentCount.value = particleAgents.value.length
    syncSpawnType(particleAgents.value[0]?.type ?? 'General')
  } catch {
    connected.value = false
  }
  void pollSnapshot()
  pollTimer = window.setInterval(pollSnapshot, 100)
})

onBeforeUnmount(() => {
  if (pollTimer !== undefined) window.clearInterval(pollTimer)
  rosterUnlisten?.()
})
</script>

<template>
  <a-config-provider :theme="panelTheme">
    <main class="panel">
      <header class="app-bar">
        <div class="identity">
          <span class="particle-mark" aria-hidden="true"></span>
          <div>
            <h1>Pqo Baseline</h1>
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

      <section class="card selected-particle">
        <header>
          <div>
            <p>SELECTED PARTICLE</p>
            <h2>{{ selectedParticle?.name ?? 'Waiting for selection' }}</h2>
          </div>
          <span v-if="selectedParticle">#{{ selectedParticle.id }}</span>
        </header>
        <div v-if="selectedParticle" class="control metadata-fields">
          <label v-for="field in particleFields" :key="field.key">
            <span>{{ field.label }}</span>
            <output v-if="field.scope === 'readonly'">
              {{ selectedFieldValue(field.key) }}
            </output>
            <a-select
              v-else-if="field.kind === 'select'"
              :value="selectedFieldValue(field.key)"
              :options="field.options?.map((option) => ({ value: option, label: option }))"
              @change="(value: unknown) => updateSelectedField(field.key, value)"
            />
            <a-input
              v-else
              :value="selectedFieldValue(field.key)"
              :placeholder="field.kind === 'skills' ? 'skills/example/SKILL.md' : undefined"
              @update:value="(value: string) => updateSelectedField(field.key, value)"
            />
          </label>
          <a-button
            v-if="!selectedParticle.agentLinked"
            block
            @click="linkSelectedParticle"
          >
            Link to Agents window
          </a-button>
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
        <a-button block class="reset" @click="showResetConfirmation = true">
          <template #icon><reload-outlined /></template>
          Reset to ground zero
        </a-button>
        <a-button block class="agents" @click="openAgents">
          Agents
        </a-button>
        <p class="interaction-hint">Drag directly · select, then click space to target</p>
      </section>

    </main>
    <div
      v-if="showResetConfirmation"
      class="reset-backdrop"
      @click.self="!resetInProgress && (showResetConfirmation = false)"
    >
      <section class="reset-dialog" role="dialog" aria-modal="true" aria-labelledby="reset-title">
        <h3 id="reset-title">Reset to ground zero?</h3>
        <p>This removes added particles, metadata, and agent chats.</p>
        <div>
          <a-button :disabled="resetInProgress" @click="showResetConfirmation = false">
            Cancel
          </a-button>
          <a-button danger type="primary" :loading="resetInProgress" @click="resetToGroundZero">
            Reset
          </a-button>
        </div>
      </section>
    </div>
  </a-config-provider>
</template>
