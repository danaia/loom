# PQO Emergent Code baseline

A Vue 3 reference model for growing software through local agent rules. PQO owns
the initial rule weights in `emergent-code.pqo`; the API parses that graph and
exposes explicit runtime overrides; Pinia holds the current projection; the
canvas renders the resulting entities and contracts.

```text
PQO rule graph
→ API boundary
→ Pinia projection
→ observe / propose / validate / commit
→ particle canvas + evidence log
```

This is deliberately a browser-side reference model, not a replacement for the
native Metal runtime. Its in-memory API is shaped so it can later be replaced by
a PQO runtime adapter without moving authoritative rule definitions into Vue.

The implementation follows the full-screen design reference in
`docs/emergent-code-concept.png`.

## Run

```sh
cd baseline-code
npm install
npm run dev
```

Click nodes to inspect them, tune or disable PQO rules, pause/resume the system,
advance one phase at a time, and reset to the canonical PQO seed state.

## Architecture

- `emergent-code.pqo` — executable rule graph and initial conditions.
- `src/api/emergenceApi.ts` — typed API boundary and PQO rule projection.
- `src/stores/emergence.ts` — agent population, growth loop, and committed UI state.
- `src/components/SystemCanvas.vue` — visual projection and local particle motion.
- `src/components/RuleField.vue` — explicit rule overrides.

The emergence loop is intentionally constrained: agents may cause new intents,
components, stores, APIs, and tests to appear, but only enabled PQO rules affect
growth and only the commit phase persists a new snapshot.
