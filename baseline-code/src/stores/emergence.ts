import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { emergenceApi } from '../api/emergenceApi'
import type { Entity, EntityKind, GrowthEvent, Link, Phase, Rule, SystemSnapshot } from '../types'

const phases: Phase[] = ['observe', 'propose', 'validate', 'commit']
const growthKinds: EntityKind[] = ['intent', 'agent', 'component', 'store', 'api', 'test']
const names: Record<EntityKind, string[]> = {
  intent: ['Reduce coupling', 'Expose telemetry', 'Clarify contracts'],
  agent: ['Scout / Sable', 'Builder / Fern', 'Verifier / Vale'],
  component: ['ContractMap', 'IntentQueue', 'EvidenceRail'],
  store: ['useIntentStore', 'useEvidenceStore', 'useGraphStore'],
  api: ['POST /intents', 'GET /evidence', 'PATCH /rules'],
  test: ['contract.spec', 'growth.spec', 'stability.spec'],
}

const clamp = (value: number, min = 0, max = 1) => Math.min(max, Math.max(min, value))

export const useEmergenceStore = defineStore('emergence', () => {
  const rules = ref<Rule[]>([])
  const entities = ref<Entity[]>([])
  const links = ref<Link[]>([])
  const events = ref<GrowthEvent[]>([])
  const tick = ref(0)
  const stability = ref(0)
  const selectedId = ref('component_04')
  const running = ref(true)
  const loading = ref(true)
  const phase = computed<Phase>(() => phases[tick.value % phases.length])
  const selected = computed(() => entities.value.find((entity) => entity.id === selectedId.value) ?? null)
  const selectedConnections = computed(() => links.value.filter((link) => link.source === selectedId.value || link.target === selectedId.value).length)

  function hydrate(snapshot: SystemSnapshot) {
    rules.value = snapshot.rules
    entities.value = snapshot.entities
    links.value = snapshot.links
    events.value = snapshot.events
    tick.value = snapshot.tick
    stability.value = snapshot.stability
  }

  async function initialize() {
    hydrate(await emergenceApi.getSnapshot())
    loading.value = false
  }

  async function updateRule(next: Rule) {
    rules.value = rules.value.map((rule) => rule.id === next.id ? next : rule)
    await emergenceApi.updateRule(next)
  }

  function addEvent(kind: GrowthEvent['kind'], message: string) {
    events.value.push({ id: `${tick.value}-${kind}-${events.value.length}`, tick: tick.value, kind, message })
    events.value = events.value.slice(-12)
  }

  function grow() {
    if (entities.value.length >= 34) return
    const enabledWeight = rules.value.filter((rule) => rule.enabled).reduce((sum, rule) => sum + rule.weight, 0)
    const shouldGrow = enabledWeight > 2.4 && tick.value % 8 === 3
    if (!shouldGrow) return
    const kind = growthKinds[entities.value.length % growthKinds.length]
    const index = entities.value.filter((entity) => entity.kind === kind).length
    const angle = entities.value.length * 2.399
    const entity: Entity = {
      id: `${kind}_${String(entities.value.length + 1).padStart(2, '0')}`,
      kind,
      name: names[kind][index % names[kind].length],
      role: kind === 'agent' ? 'autonomous builder' : kind,
      x: clamp(0.5 + Math.cos(angle) * (0.18 + (entities.value.length % 4) * 0.045), 0.08, 0.92),
      y: clamp(0.5 + Math.sin(angle) * (0.18 + (entities.value.length % 3) * 0.06), 0.08, 0.92),
      vx: 0,
      vy: 0,
      energy: clamp(0.55 + enabledWeight * 0.05),
      confidence: 0.64,
      bornAt: tick.value,
    }
    const compatible = entities.value.filter((candidate) => candidate.kind !== kind)
    const target = compatible[(tick.value + entities.value.length) % compatible.length]
    entities.value.push(entity)
    if (target) links.value.push({ source: target.id, target: entity.id, strength: 0.58 })
    addEvent(kind, `${kind[0].toUpperCase()}${kind.slice(1)} emerged: ${entity.name}`)
  }

  function step() {
    tick.value += 1
    grow()
    const averageEnergy = entities.value.reduce((sum, entity) => sum + entity.energy, 0) / Math.max(1, entities.value.length)
    const coverage = new Set(entities.value.map((entity) => entity.kind)).size / growthKinds.length
    stability.value = clamp(stability.value * 0.97 + (averageEnergy * 0.55 + coverage * 0.45) * 0.03)
    if (phase.value === 'commit') {
      void emergenceApi.commit(snapshot())
    }
  }

  function snapshot(): SystemSnapshot {
    return {
      rules: rules.value,
      entities: entities.value,
      links: links.value,
      events: events.value,
      tick: tick.value,
      stability: stability.value,
    }
  }

  async function reset() {
    hydrate(await emergenceApi.reset())
    selectedId.value = 'component_04'
    running.value = true
  }

  return {
    rules, entities, links, events, tick, stability, selectedId, running, loading,
    phase, selected, selectedConnections, initialize, updateRule, step, reset,
  }
})
