export const entityKinds = ['intent', 'agent', 'component', 'store', 'api', 'test'] as const
export type EntityKind = (typeof entityKinds)[number]
export type Phase = 'observe' | 'propose' | 'validate' | 'commit'

export interface Rule {
  id: string
  name: string
  description: string
  weight: number
  enabled: boolean
}

export interface Entity {
  id: string
  kind: EntityKind
  name: string
  role: string
  x: number
  y: number
  vx: number
  vy: number
  energy: number
  confidence: number
  bornAt: number
}

export interface Link {
  source: string
  target: string
  strength: number
}

export interface GrowthEvent {
  id: string
  tick: number
  kind: EntityKind | 'system'
  message: string
}

export interface SystemSnapshot {
  rules: Rule[]
  entities: Entity[]
  links: Link[]
  events: GrowthEvent[]
  tick: number
  stability: number
}
