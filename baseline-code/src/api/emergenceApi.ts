import pqoSource from '../../emergent-code.pqo?raw'
import type { Entity, EntityKind, GrowthEvent, Link, Rule, SystemSnapshot } from '../types'

const metaPattern = /^\/\/ @rule ([^|]+)\|([^|]+)\|(.+)$/gm
const weightPattern = /^const rules\.([a-z_]+): f32 = ([\d.]+)$/gm

function parseRules(): Rule[] {
  const metadata = new Map<string, { name: string; description: string }>()
  for (const match of pqoSource.matchAll(metaPattern)) {
    metadata.set(match[1], { name: match[2], description: match[3] })
  }
  return [...pqoSource.matchAll(weightPattern)].map((match) => ({
    id: match[1],
    name: metadata.get(match[1])?.name ?? match[1],
    description: metadata.get(match[1])?.description ?? '',
    weight: Number(match[2]),
    enabled: true,
  }))
}

const seeds: Array<[EntityKind, string, string, number, number]> = [
  ['intent', 'Define rule grammar', 'specification', 0.50, 0.12],
  ['agent', 'Scout / Iris', 'observer', 0.24, 0.24],
  ['agent', 'Builder / Moss', 'builder', 0.66, 0.20],
  ['component', 'RuleEditor', 'component', 0.50, 0.38],
  ['store', 'useRuleStore', 'shared state', 0.34, 0.52],
  ['api', 'GET /rules', 'contract', 0.72, 0.46],
  ['test', 'rule-weight.spec', 'verification', 0.58, 0.64],
  ['component', 'SystemCanvas', 'component', 0.25, 0.72],
  ['intent', 'Explain decisions', 'requirement', 0.78, 0.76],
  ['store', 'useGrowthStore', 'shared state', 0.48, 0.84],
  ['api', 'POST /step', 'contract', 0.86, 0.58],
  ['test', 'emergence.spec', 'verification', 0.12, 0.50],
]

// API payloads can originate as Vue proxies; JSON is the wire-format boundary.
const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value)) as T

function makeEntity(seed: (typeof seeds)[number], index: number): Entity {
  const [kind, name, role, x, y] = seed
  return {
    id: `${kind}_${String(index + 1).padStart(2, '0')}`,
    kind,
    name,
    role,
    x,
    y,
    vx: 0,
    vy: 0,
    energy: 0.62 + (index % 4) * 0.07,
    confidence: 0.68 + (index % 3) * 0.08,
    bornAt: 0,
  }
}

function initialSnapshot(): SystemSnapshot {
  const entities = seeds.map(makeEntity)
  const links: Link[] = [
    ['intent_01', 'agent_02'], ['intent_01', 'agent_03'], ['agent_02', 'component_04'],
    ['agent_03', 'component_04'], ['component_04', 'store_05'], ['component_04', 'api_06'],
    ['store_05', 'test_07'], ['api_06', 'test_07'], ['component_08', 'store_10'],
    ['intent_09', 'api_11'], ['api_11', 'test_12'], ['agent_02', 'component_08'],
    ['store_10', 'test_07'], ['agent_03', 'api_11'], ['component_08', 'test_12'],
  ].map(([source, target], index) => ({ source, target, strength: 0.45 + (index % 4) * 0.12 }))

  return {
    rules: parseRules(),
    entities,
    links,
    tick: 12480,
    stability: 0.74,
    events: [
      event(12466, 'intent', 'Intent created: define rule grammar'),
      event(12469, 'agent', 'Builder claimed the highest-energy intent'),
      event(12472, 'component', 'Component created: RuleEditor'),
      event(12475, 'store', 'Shared rule state stabilized'),
      event(12478, 'api', 'Contract validated: GET /rules'),
      event(12480, 'system', 'System coherence increased'),
    ],
  }
}

function event(tick: number, kind: GrowthEvent['kind'], message: string): GrowthEvent {
  return { id: `${tick}-${kind}-${message}`, tick, kind, message }
}

let state = initialSnapshot()

const latency = <T>(value: T) => new Promise<T>((resolve) => window.setTimeout(() => resolve(clone(value)), 80))

export const emergenceApi = {
  getSnapshot: () => latency(state),
  updateRule(rule: Rule) {
    state.rules = state.rules.map((item) => item.id === rule.id ? clone(rule) : item)
    state.events.push(event(state.tick, 'system', `Rule override: ${rule.name} = ${rule.enabled ? rule.weight.toFixed(2) : 'off'}`))
    return latency(rule)
  },
  commit(snapshot: SystemSnapshot) {
    state = clone(snapshot)
    return Promise.resolve()
  },
  reset() {
    state = initialSnapshot()
    return latency(state)
  },
}
