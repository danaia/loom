<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { Entity, EntityKind, Link } from '../types'

const props = defineProps<{ entities: Entity[]; links: Link[]; selectedId: string; running: boolean }>()
const emit = defineEmits<{ select: [id: string] }>()
const canvas = ref<HTMLCanvasElement | null>(null)
let frame = 0
let last = 0
let width = 0
let height = 0
let dpr = 1
let resizeObserver: ResizeObserver | undefined

const colors: Record<EntityKind, string> = {
  intent: '#d9f322', agent: '#55dfd5', component: '#bba5ff',
  store: '#7ee36b', api: '#6bd4ff', test: '#ff7968',
}

function resize() {
  const element = canvas.value
  if (!element) return
  const bounds = element.getBoundingClientRect()
  dpr = Math.min(2, window.devicePixelRatio || 1)
  width = bounds.width
  height = bounds.height
  element.width = Math.round(width * dpr)
  element.height = Math.round(height * dpr)
}

function position(entity: Entity) {
  return { x: entity.x * width, y: entity.y * height }
}

function simulate(delta: number) {
  if (!props.running || !width || !height) return
  const entities = props.entities
  const scale = Math.min(1.6, delta / 16.67)
  for (let i = 0; i < entities.length; i += 1) {
    const a = entities[i]
    let fx = (0.5 - a.x) * 0.000025
    let fy = (0.5 - a.y) * 0.000025
    for (let j = i + 1; j < entities.length; j += 1) {
      const b = entities[j]
      const dx = a.x - b.x
      const dy = a.y - b.y
      const distanceSq = dx * dx + dy * dy + 0.0004
      if (distanceSq < 0.018) {
        const force = 0.0000018 / distanceSq
        fx += dx * force
        fy += dy * force
        b.vx -= dx * force * scale
        b.vy -= dy * force * scale
      }
    }
    a.vx = (a.vx + fx * scale) * 0.96
    a.vy = (a.vy + fy * scale) * 0.96
  }
  for (const link of props.links) {
    const source = entities.find((entity) => entity.id === link.source)
    const target = entities.find((entity) => entity.id === link.target)
    if (!source || !target) continue
    const dx = target.x - source.x
    const dy = target.y - source.y
    const distance = Math.hypot(dx, dy) || 1
    const force = (distance - 0.19) * 0.0003 * link.strength
    source.vx += (dx / distance) * force * scale
    source.vy += (dy / distance) * force * scale
    target.vx -= (dx / distance) * force * scale
    target.vy -= (dy / distance) * force * scale
  }
  for (const entity of entities) {
    entity.x = Math.max(0.055, Math.min(0.945, entity.x + entity.vx * scale))
    entity.y = Math.max(0.06, Math.min(0.94, entity.y + entity.vy * scale))
  }
}

function drawNode(ctx: CanvasRenderingContext2D, entity: Entity, time: number) {
  const { x, y } = position(entity)
  const color = colors[entity.kind]
  const selected = entity.id === props.selectedId
  const radius = selected ? 12 : entity.kind === 'component' ? 9 : 7
  const pulse = entity.bornAt > 0 ? Math.max(0, 1 - (time / 800) % 1) : 0
  ctx.save()
  ctx.translate(x, y)
  ctx.shadowColor = color
  ctx.shadowBlur = selected ? 22 : 10
  ctx.fillStyle = '#0b1720'
  ctx.strokeStyle = color
  ctx.lineWidth = selected ? 2.2 : 1.5
  ctx.beginPath()
  if (entity.kind === 'component') {
    for (let i = 0; i < 6; i += 1) {
      const angle = Math.PI / 3 * i - Math.PI / 2
      const px = Math.cos(angle) * radius
      const py = Math.sin(angle) * radius
      i === 0 ? ctx.moveTo(px, py) : ctx.lineTo(px, py)
    }
    ctx.closePath()
  } else if (entity.kind === 'store' || entity.kind === 'api' || entity.kind === 'test') {
    ctx.rect(-radius * 0.75, -radius * 0.75, radius * 1.5, radius * 1.5)
  } else {
    ctx.arc(0, 0, radius, 0, Math.PI * 2)
  }
  ctx.fill()
  ctx.stroke()
  if (entity.kind === 'api') ctx.rotate(Math.PI / 4)
  if (entity.kind === 'test') {
    ctx.beginPath(); ctx.moveTo(0, -radius); ctx.lineTo(radius, radius); ctx.lineTo(-radius, radius); ctx.closePath(); ctx.fill(); ctx.stroke()
  }
  if (pulse > 0) {
    ctx.globalAlpha = pulse * 0.4
    ctx.beginPath(); ctx.arc(0, 0, radius + (1 - pulse) * 28, 0, Math.PI * 2); ctx.stroke()
  }
  ctx.restore()
}

function draw(time: number) {
  const element = canvas.value
  if (!element || !width || !height) return
  const ctx = element.getContext('2d')
  if (!ctx) return
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
  ctx.clearRect(0, 0, width, height)
  ctx.strokeStyle = 'rgba(125, 162, 174, .075)'
  ctx.lineWidth = 1
  for (let x = 24; x < width; x += 32) { ctx.beginPath(); ctx.moveTo(x, 0); ctx.lineTo(x, height); ctx.stroke() }
  for (let y = 24; y < height; y += 32) { ctx.beginPath(); ctx.moveTo(0, y); ctx.lineTo(width, y); ctx.stroke() }
  for (const link of props.links) {
    const source = props.entities.find((entity) => entity.id === link.source)
    const target = props.entities.find((entity) => entity.id === link.target)
    if (!source || !target) continue
    const a = position(source); const b = position(target)
    const active = source.id === props.selectedId || target.id === props.selectedId
    ctx.strokeStyle = active ? 'rgba(217, 243, 34, .72)' : `rgba(139, 174, 183, ${0.14 + link.strength * 0.28})`
    ctx.lineWidth = active ? 1.3 : 0.8
    ctx.setLineDash(link.strength < 0.55 ? [2, 4] : [])
    ctx.beginPath(); ctx.moveTo(a.x, a.y); ctx.lineTo(b.x, b.y); ctx.stroke()
  }
  ctx.setLineDash([])
  props.entities.forEach((entity) => drawNode(ctx, entity, time))
  const selected = props.entities.find((entity) => entity.id === props.selectedId)
  if (selected) {
    const pos = position(selected)
    const label = `${selected.kind} / ${selected.name}`
    ctx.font = '12px IBM Plex Mono, monospace'
    const labelWidth = Math.min(220, ctx.measureText(label).width + 24)
    const left = Math.min(width - labelWidth - 12, pos.x + 17)
    const top = Math.max(12, pos.y - 18)
    ctx.fillStyle = 'rgba(7, 16, 22, .94)'; ctx.strokeStyle = colors[selected.kind]; ctx.lineWidth = 1
    ctx.beginPath(); ctx.roundRect(left, top, labelWidth, 34, 4); ctx.fill(); ctx.stroke()
    ctx.fillStyle = '#dce7e8'; ctx.fillText(label, left + 12, top + 21)
  }
}

function loop(time: number) {
  const delta = last ? Math.min(32, time - last) : 16.67
  last = time
  simulate(delta)
  draw(time)
  frame = requestAnimationFrame(loop)
}

function onClick(event: MouseEvent) {
  const bounds = canvas.value?.getBoundingClientRect()
  if (!bounds) return
  const x = event.clientX - bounds.left
  const y = event.clientY - bounds.top
  let closest: { id: string; distance: number } | undefined
  for (const entity of props.entities) {
    const point = position(entity)
    const distance = Math.hypot(point.x - x, point.y - y)
    if (distance < 24 && (!closest || distance < closest.distance)) closest = { id: entity.id, distance }
  }
  if (closest) emit('select', closest.id)
}

watch(() => [props.entities.length, props.links.length], () => draw(performance.now()))
onMounted(() => {
  resizeObserver = new ResizeObserver(resize)
  if (canvas.value) resizeObserver.observe(canvas.value)
  resize()
  frame = requestAnimationFrame(loop)
})
onBeforeUnmount(() => { cancelAnimationFrame(frame); resizeObserver?.disconnect() })
</script>

<template>
  <section class="system-stage">
    <header class="section-heading canvas-heading">
      <div><h2>Live system</h2><span>{{ entities.length }} entities / {{ links.length }} contracts</span></div>
      <span class="canvas-hint">Select a node to inspect</span>
    </header>
    <canvas ref="canvas" aria-label="Interactive particle graph of the emerging software system" @click="onClick" />
  </section>
</template>
