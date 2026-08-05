<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import {
  Badge as ABadge,
  Button as AButton,
  ConfigProvider as AConfigProvider,
  Progress as AProgress,
  Select as ASelect,
  Slider as ASlider,
  Switch as ASwitch,
  theme,
} from 'ant-design-vue'
import { getSnapshot, setControl } from './bridge'

const connected = ref(false)
const scale = ref(3)
const basePairs = ref(12)
const thermal = ref(0.18)
const bend = ref(0)
const separation = ref(0)
const motion = ref(true)
const smartLod = ref(true)
const lodBias = ref(0)
const showBases = ref(true)
let pollTimer: number | undefined

const { compactAlgorithm, darkAlgorithm } = theme
const panelTheme = {
  algorithm: [darkAlgorithm, compactAlgorithm],
  token: {
    colorPrimary: '#35c9e8',
    colorInfo: '#35c9e8',
    colorSuccess: '#78d69a',
    colorBgBase: '#0b1117',
    colorBgContainer: '#121b24',
    colorBgElevated: '#18232e',
    colorBorder: '#293846',
    colorText: '#e3edf2',
    colorTextSecondary: '#8297a5',
    fontSize: 12,
    controlHeight: 28,
    borderRadius: 5,
  },
}

const levels = [
  { value: 0, label: 'Quantum · hydrogen 1s' },
  { value: 1, label: 'Molecular · nucleotide sites' },
  { value: 2, label: 'Structural · base pairs' },
  { value: 3, label: 'Mesoscale · double helix' },
  { value: 4, label: 'Continuum · elastic DNA' },
]
const level = computed(() => levels[scale.value] ?? levels[3])
const turns = computed(() => basePairs.value / 10.5)
const lengthNm = computed(() => Math.max(0, basePairs.value - 1) * 0.34)
const lodPercent = computed(() => Math.round(((scale.value + 1) / levels.length) * 100))

function control(name: string, value: number) {
  void setControl(name, value).catch(() => { connected.value = false })
}

function setScale(value: number) {
  scale.value = value
  control('sandbox.scale', value)
}

function setNumber(name: string, value: number | number[]) {
  const next = typeof value === 'number' ? value : value[0]
  if (name === 'sandbox.base_pairs') basePairs.value = next
  else if (name === 'sandbox.thermal') thermal.value = next
  else if (name === 'sandbox.bend') bend.value = next
  else if (name === 'sandbox.separation') separation.value = next
  else if (name === 'sandbox.lod_bias') lodBias.value = next
  control(name, next)
}

function setToggle(name: string, value: boolean | string | number) {
  const checked = value === true
  if (name === 'sandbox.motion') motion.value = checked
  else if (name === 'sandbox.show_bases') showBases.value = checked
  else if (name === 'sandbox.smart_lod') smartLod.value = checked
  control(name, checked ? 1 : 0)
}

async function pollSnapshot() {
  try {
    const snapshot = await getSnapshot()
    connected.value = snapshot.connected
  } catch {
    connected.value = false
  }
}

function resetEquilibrium() {
  thermal.value = 0.18
  bend.value = 0
  separation.value = 0
  motion.value = true
  ;[
    ['sandbox.thermal', thermal.value],
    ['sandbox.bend', bend.value],
    ['sandbox.separation', separation.value],
    ['sandbox.motion', 1],
  ].forEach(([name, value]) => control(name as string, value as number))
}

onMounted(() => {
  void pollSnapshot()
  pollTimer = window.setInterval(pollSnapshot, 500)
})

onBeforeUnmount(() => {
  if (pollTimer !== undefined) window.clearInterval(pollTimer)
})
</script>

<template>
  <a-config-provider :theme="panelTheme">
    <main class="panel sandbox-panel">
      <header class="app-bar">
        <div class="identity">
          <span class="dna-mark" aria-hidden="true"></span>
          <div>
            <h1>Quantum → DNA</h1>
            <p>Multiscale physics sandbox</p>
          </div>
        </div>
        <a-badge :status="connected ? 'success' : 'default'" :text="connected ? 'Live' : 'Waiting'" />
      </header>

      <section class="model-status">
        <span>ACTIVE REPRESENTATION</span>
        <strong>{{ level.label }}</strong>
        <a-progress :percent="lodPercent" :show-info="false" stroke-color="#35c9e8" trail-color="#253442" />
        <p>Each level has its own model contract; visual LOD does not claim identical physics.</p>
      </section>

      <section class="card">
        <header><div><p>SCALE</p><h2>Representation hierarchy</h2></div><span>0 → 4</span></header>
        <div class="control stack">
          <a-select :value="scale" :options="levels" @change="(value: unknown) => typeof value === 'number' && setScale(value)" />
          <div class="scale-rail">
            <button v-for="item in levels" :key="item.value" :class="{ active: item.value === scale }" @click="setScale(item.value)">
              <b>{{ item.value }}</b><span>{{ ['ψ', 'N', 'bp', 'DNA', 'rod'][item.value] }}</span>
            </button>
          </div>
        </div>
      </section>

      <section class="card">
        <header><div><p>B-DNA</p><h2>Drew-Dickerson dodecamer</h2></div><span>CGCGAATTCGCG</span></header>
        <div class="facts">
          <div><span>Rise</span><strong>0.34 nm/bp</strong></div>
          <div><span>Twist</span><strong>34.29°/bp</strong></div>
          <div><span>Length</span><strong>{{ lengthNm.toFixed(2) }} nm</strong></div>
          <div><span>Turns</span><strong>{{ turns.toFixed(2) }}</strong></div>
        </div>
        <div class="control slider-control">
          <div><label>Base pairs</label><output>{{ basePairs }}</output></div>
          <a-slider :value="basePairs" :min="2" :max="24" :step="1" @change="(v: number | number[]) => setNumber('sandbox.base_pairs', v)" />
          <p><span>validated: 12</span><span>&gt;12 repeats sequence</span></p>
        </div>
      </section>

      <section class="card">
        <header><div><p>PERTURBATIONS</p><h2>Bounded coarse-grained modes</h2></div><span>interactive</span></header>
        <div class="control slider-control">
          <div><label>Thermal mode amplitude</label><output>{{ thermal.toFixed(2) }}</output></div>
          <a-slider :value="thermal" :min="0" :max="1" :step="0.01" @change="(v: number | number[]) => setNumber('sandbox.thermal', v)" />
          <div><label>Elastic bend</label><output>{{ bend.toFixed(2) }}</output></div>
          <a-slider :value="bend" :min="-1" :max="1" :step="0.01" @change="(v: number | number[]) => setNumber('sandbox.bend', v)" />
          <div><label>Central base separation</label><output>{{ separation.toFixed(2) }}</output></div>
          <a-slider :value="separation" :min="0" :max="1" :step="0.01" @change="(v: number | number[]) => setNumber('sandbox.separation', v)" />
        </div>
      </section>

      <section class="card compact-card">
        <header><div><p>RENDERING</p><h2>Adaptive presentation</h2></div></header>
        <div class="toggles">
          <label><span>Animate modes</span><a-switch :checked="motion" @change="(v: boolean | string | number) => setToggle('sandbox.motion', v)" /></label>
          <label><span>Show bases</span><a-switch :checked="showBases" @change="(v: boolean | string | number) => setToggle('sandbox.show_bases', v)" /></label>
          <label><span>Smart LOD</span><a-switch :checked="smartLod" @change="(v: boolean | string | number) => setToggle('sandbox.smart_lod', v)" /></label>
        </div>
        <div class="control slider-control">
          <div><label>LOD quality bias</label><output>{{ lodBias.toFixed(1) }}</output></div>
          <a-slider :value="lodBias" :min="-2" :max="2" :step="0.1" @change="(v: number | number[]) => setNumber('sandbox.lod_bias', v)" />
        </div>
      </section>

      <a-button block class="reset" @click="resetEquilibrium">Return to equilibrium</a-button>
      <p class="scientific-note">Hydrogen 1s is analytic. DNA views are ideal B-DNA and coarse-grained geometry; they are not an exact many-electron wavefunction or atomistic MD.</p>
    </main>
  </a-config-provider>
</template>
