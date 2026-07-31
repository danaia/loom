import { emit, listen } from '@tauri-apps/api/event'
import {
  loadParticleAgentRecords,
  resetBaselineData,
  saveParticleAgentRecords,
} from './bridge'
import type { ParticleAgentRecord } from './bridge'
import { particlePanelSchema } from './particlePanelSchema'
import type { UnlistenFn } from '@tauri-apps/api/event'

const rosterKey = 'pqo.particle-agents.v1'
const rosterChangedEvent = 'pqo-particle-roster-changed'
const baselineResetEvent = 'pqo-baseline-reset'

export type ParticleAgent = ParticleAgentRecord

export function defaultParticleAgent(
  id: number,
  name = `General ${id + 1}`,
  type = 'General',
): ParticleAgent {
  return {
    id,
    schemaVersion: particlePanelSchema.version,
    name,
    type,
    agentLinked: true,
    skills: [],
    fields: Object.fromEntries(
      particlePanelSchema.fields
        .filter((field) =>
          field.scope === 'particle' &&
          !['name', 'type', 'skills'].includes(field.key),
        )
        .map((field) => [field.key, field.defaultValue]),
    ),
  }
}

function migrateStoredRoster(): ParticleAgent[] {
  try {
    const stored = JSON.parse(localStorage.getItem(rosterKey) ?? 'null') as unknown
    if (!Array.isArray(stored)) return []
    const roster = stored.filter(
      (entry): entry is { id: number; name: string; type: string } =>
        typeof entry === 'object' &&
        entry !== null &&
        typeof (entry as { id?: unknown }).id === 'number' &&
        typeof (entry as { name?: unknown }).name === 'string' &&
        typeof (entry as { type?: unknown }).type === 'string',
    )
    return roster.slice(0, 32).map((entry) =>
      defaultParticleAgent(entry.id, entry.name, entry.type),
    )
  } catch {
    return []
  }
}

export function applyParticleSchema(agent: ParticleAgent): ParticleAgent {
  const defaults = defaultParticleAgent(agent.id, agent.name, agent.type)
  return {
    ...defaults,
    ...agent,
    schemaVersion: particlePanelSchema.version,
    agentLinked: agent.agentLinked !== false,
    skills: Array.isArray(agent.skills) ? agent.skills : [],
    fields: { ...defaults.fields, ...(agent.fields ?? {}) },
  }
}

export async function loadParticleAgents(): Promise<ParticleAgent[]> {
  const stored = await loadParticleAgentRecords()
  if (stored.length) {
    const resolved = stored.map(applyParticleSchema)
    if (JSON.stringify(resolved) !== JSON.stringify(stored)) {
      return (await saveParticleAgentRecords(resolved)).map(applyParticleSchema)
    }
    return resolved
  }
  const migrated = migrateStoredRoster()
  const roster = migrated.length ? migrated : [defaultParticleAgent(0)]
  const saved = await saveParticleAgentRecords(roster)
  localStorage.removeItem(rosterKey)
  return saved.map(applyParticleSchema)
}

export async function saveParticleAgents(roster: ParticleAgent[]) {
  const saved = await saveParticleAgentRecords(roster.slice(0, 32).map(applyParticleSchema))
  const resolved = saved.map(applyParticleSchema)
  await emit(rosterChangedEvent, resolved)
  return resolved
}

export async function subscribeParticleAgents(
  listener: (roster: ParticleAgent[]) => void,
): Promise<UnlistenFn> {
  return listen<ParticleAgent[]>(rosterChangedEvent, (event) => {
    listener(event.payload.map(applyParticleSchema))
  })
}

export async function resetParticleAgents(): Promise<ParticleAgent[]> {
  const reset = (await resetBaselineData()).map(applyParticleSchema)
  await emit(rosterChangedEvent, reset)
  await emit(baselineResetEvent)
  return reset
}

export async function subscribeBaselineReset(listener: () => void): Promise<UnlistenFn> {
  return listen(baselineResetEvent, listener)
}

export function uniqueAgentName(base: string, roster: ParticleAgent[]) {
  const clean = base.trim() || 'General'
  const names = new Set(roster.map((agent) => agent.name.toLocaleLowerCase()))
  if (!names.has(clean.toLocaleLowerCase())) return clean
  let suffix = 2
  while (names.has(`${clean} ${suffix}`.toLocaleLowerCase())) suffix += 1
  return `${clean} ${suffix}`
}
