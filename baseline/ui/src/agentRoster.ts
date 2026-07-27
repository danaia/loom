export interface ParticleAgent {
  id: number
  name: string
  type: string
}

const rosterKey = 'loom.particle-agents.v1'

const initialRoster: ParticleAgent[] = [{ id: 0, name: 'General 1', type: 'General' }]

export function loadParticleAgents(): ParticleAgent[] {
  try {
    const stored = JSON.parse(localStorage.getItem(rosterKey) ?? 'null') as unknown
    if (!Array.isArray(stored)) return [...initialRoster]
    const roster = stored.filter(
      (entry): entry is ParticleAgent =>
        typeof entry === 'object' &&
        entry !== null &&
        typeof (entry as ParticleAgent).id === 'number' &&
        typeof (entry as ParticleAgent).name === 'string' &&
        typeof (entry as ParticleAgent).type === 'string',
    )
    return roster.length ? roster.slice(0, 32) : [...initialRoster]
  } catch {
    return [...initialRoster]
  }
}

export function saveParticleAgents(roster: ParticleAgent[]) {
  localStorage.setItem(rosterKey, JSON.stringify(roster.slice(0, 32)))
}

export function uniqueAgentName(base: string, roster: ParticleAgent[]) {
  const clean = base.trim() || 'General'
  const names = new Set(roster.map((agent) => agent.name.toLocaleLowerCase()))
  if (!names.has(clean.toLocaleLowerCase())) return clean
  let suffix = 2
  while (names.has(`${clean} ${suffix}`.toLocaleLowerCase())) suffix += 1
  return `${clean} ${suffix}`
}
