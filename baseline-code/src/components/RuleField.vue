<script setup lang="ts">
import type { Rule } from '../types'

const props = defineProps<{ rules: Rule[] }>()
const emit = defineEmits<{ change: [rule: Rule] }>()

function patch(rule: Rule, value: Partial<Rule>) {
  emit('change', { ...rule, ...value })
}
</script>

<template>
  <aside class="rail rule-rail">
    <header class="section-heading">
      <h2>Rule field</h2>
      <span class="code-mark" aria-hidden="true">&lt;/&gt;</span>
    </header>

    <div class="rule-list">
      <article v-for="(rule, index) in props.rules" :key="rule.id" class="rule-row">
        <div class="rule-number">{{ String(index + 1).padStart(2, '0') }}</div>
        <div class="rule-content">
          <div class="rule-title-row">
            <h3>{{ rule.name }}</h3>
            <button
              class="toggle"
              :class="{ enabled: rule.enabled }"
              :aria-label="`${rule.enabled ? 'Disable' : 'Enable'} ${rule.name}`"
              :aria-pressed="rule.enabled"
              @click="patch(rule, { enabled: !rule.enabled })"
            ><span /></button>
          </div>
          <p>{{ rule.description }}</p>
          <label>
            <span>Weight</span>
            <input
              type="range"
              min="0"
              max="2"
              step="0.05"
              :disabled="!rule.enabled"
              :value="rule.weight"
              @input="patch(rule, { weight: Number(($event.target as HTMLInputElement).value) })"
            />
            <output>{{ rule.weight.toFixed(2) }}</output>
          </label>
        </div>
      </article>
    </div>

    <section class="legend">
      <h2>Node legend</h2>
      <div v-for="kind in ['intent', 'agent', 'component', 'store', 'api', 'test']" :key="kind" class="legend-row">
        <i :class="`glyph glyph-${kind}`" />
        <span>{{ kind }}</span>
        <small>{{ {
          intent: 'Goals and requirements', agent: 'Autonomous builders', component: 'Reusable UI and logic',
          store: 'Shared reactive state', api: 'Contracts and boundaries', test: 'Guards and evidence',
        }[kind] }}</small>
      </div>
    </section>
  </aside>
</template>
