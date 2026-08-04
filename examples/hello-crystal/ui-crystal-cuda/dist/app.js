const invoke = (command, args = {}) => window.__TAURI_INTERNALS__?.invoke(command, args)

const controls = {
  growth: { value: .72, digits: 2 },
  anisotropy: { value: .68, digits: 2 },
  temperature: { value: .18, digits: 2 },
  damage: { value: 0, digits: 2 },
  particle_count: { value: 1000000, digits: 0 },
  show_field: { value: 1 },
  show_particles: { value: 0 },
  yaw: { value: -.55, digits: 2 },
  pitch: { value: -.35, digits: 2 },
  zoom: { value: 1, digits: 2 },
  smart_lod: { value: 1 },
  lod_bias: { value: 0, digits: 1 },
}

function renderValue(name, value) {
  const output = document.querySelector(`#${name}Out`)
  if (!output) return
  output.value = name === 'particle_count'
    ? Math.round(value).toLocaleString()
    : Number(value).toFixed(controls[name].digits)
}

function send(name, value) {
  controls[name].value = value
  renderValue(name, value)
  invoke('set_control', { name: `crystal.${name}`, value })
}

for (const name of ['growth', 'anisotropy', 'temperature', 'damage', 'particle_count', 'lod_bias']) {
  document.querySelector(`#${name}`).addEventListener('input', event => {
    send(name, Number(event.target.value))
  })
}

for (const name of ['show_field', 'show_particles', 'smart_lod']) {
  document.querySelector(`#${name}`).addEventListener('change', event => {
    send(name, event.target.checked ? 1 : 0)
  })
}

for (const button of document.querySelectorAll('[data-camera]')) {
  button.addEventListener('click', () => {
    const name = button.dataset.camera
    const bridgeName = name === 'yaw' ? 'orbit_delta_yaw' : name === 'pitch' ? 'orbit_delta_pitch' : 'zoom_delta'
    invoke('set_control', { name: `crystal.${bridgeName}`, value: Number(button.dataset.delta) })
  })
}

document.querySelector('#reset_camera').addEventListener('click', () => {
  for (const [name, value] of Object.entries({ yaw: -.55, pitch: -.35, zoom: 1 })) send(name, value)
})

document.querySelector('#reset').addEventListener('click', () => {
  for (const [name, value] of Object.entries({
    growth: .72,
    anisotropy: .68,
    temperature: .18,
    damage: 0,
    particle_count: 1000000,
    show_field: 1,
    show_particles: 0,
    yaw: -.55,
    pitch: -.35,
    zoom: 1,
    smart_lod: 1,
    lod_bias: 0,
  })) {
    const element = document.querySelector(`#${name}`)
    if (element?.type === 'checkbox') element.checked = value === 1
    else if (element) element.value = value
    send(name, value)
  }
})

async function connect() {
  try {
    if (!window.__TAURI_INTERNALS__) throw new Error('Tauri bridge unavailable')
    const snapshot = await invoke('get_snapshot')
    if (snapshot?.values) {
      for (const [key, rawValue] of Object.entries(snapshot.values)) {
        const name = key.replace('crystal.', '')
        if (!(name in controls)) continue
        const value = Number(rawValue)
        controls[name].value = value
        const element = document.querySelector(`#${name}`)
        if (element?.type === 'checkbox') element.checked = value >= .5
        else if (element) element.value = value
        renderValue(name, value)
      }
    }
    document.querySelector('#status').textContent = 'Connected'
  } catch {
    document.querySelector('#status').textContent = 'Disconnected'
  }
}

connect()
