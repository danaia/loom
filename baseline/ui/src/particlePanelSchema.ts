export type ParticleFieldScope = 'global' | 'particle' | 'readonly'
export type ParticleFieldKind = 'text' | 'select' | 'skills' | 'number'

export interface ParticlePanelField {
  key: string
  label: string
  scope: ParticleFieldScope
  kind: ParticleFieldKind
  defaultValue: unknown
  options?: string[]
  control?: string
}

export const particlePanelSchema = {
  version: 1,
  fields: [
    {
      key: 'id',
      label: 'Particle ID',
      scope: 'readonly',
      kind: 'number',
      defaultValue: 0,
    },
    {
      key: 'name',
      label: 'Name',
      scope: 'particle',
      kind: 'text',
      defaultValue: 'General',
    },
    {
      key: 'type',
      label: 'Type',
      scope: 'particle',
      kind: 'select',
      defaultValue: 'General',
      options: ['General', 'Scout', 'Builder'],
    },
    {
      key: 'skills',
      label: 'Attached skills',
      scope: 'particle',
      kind: 'skills',
      defaultValue: [],
    },
    {
      key: 'description',
      label: 'Description',
      scope: 'particle',
      kind: 'text',
      defaultValue: '',
    },
    {
      key: 'spaceDrag',
      label: 'Space drag',
      scope: 'global',
      kind: 'number',
      defaultValue: 0,
      control: 'interaction.space_drag',
    },
  ] satisfies ParticlePanelField[],
} as const

export const particleFields = particlePanelSchema.fields.filter(
  (field) => field.scope !== 'global',
)
