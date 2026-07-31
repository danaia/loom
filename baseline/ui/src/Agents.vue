<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue'
import {
  ApiOutlined,
  ArrowUpOutlined,
  CheckCircleFilled,
  DeleteOutlined,
  KeyOutlined,
  PlusOutlined,
  SafetyCertificateOutlined,
} from '@ant-design/icons-vue'
import {
  createAgentChat,
  connectAndStartAgent,
  deleteAgentChat,
  getSnapshot,
  hasAgentApiKey,
  loadAgentChats,
  saveAgentChat,
  sendAgentMessage,
  setControl,
  startSavedAgent,
} from './bridge'
import type { AgentChat, AgentChatMessage } from './bridge'
import {
  loadParticleAgents,
  saveParticleAgents,
  subscribeBaselineReset,
  subscribeParticleAgents,
} from './agentRoster'
import type { ParticleAgent } from './agentRoster'

type ChatMessage = AgentChatMessage

const apiKey = ref('')
const keyVisible = ref(false)
const showKeySetup = ref(false)
const connectionPhase = ref<'idle' | 'connecting' | 'connected' | 'error'>('idle')
const error = ref('')
const responseId = ref<string | null>(null)
const messages = ref<ChatMessage[]>([])
const chats = ref<AgentChat[]>([])
const activeChatId = ref<string | null>(null)
const prompt = ref('')
const sending = ref(false)
const particleAgents = ref<ParticleAgent[]>([])
const selectedParticleId = ref(0)
const removalCandidate = ref<ParticleAgent | null>(null)
const removeAgentLink = ref(true)
const removeMetalParticle = ref(false)
const agentActivity = ref('Thinking')
const conversation = ref<HTMLElement | null>(null)
let activityTimer: ReturnType<typeof setInterval> | null = null
let rosterTimer: ReturnType<typeof setInterval> | null = null
let rosterUnlisten: (() => void) | undefined
let resetUnlisten: (() => void) | undefined

const modelId = 'gpt-5.6-terra'
const activeModel = ref(modelId)
const projectName = ref('baseline')
const projectRoot = ref('')
const projectFileCount = ref(0)
const minimumActivityMs = 900
const modelLabel = computed(() => activeModel.value)
const linkedParticleAgents = computed(
  () => particleAgents.value.filter((agent) => agent.agentLinked),
)
const canSaveKey = computed(() => apiKey.value.trim().length > 0)
const connected = computed(() => connectionPhase.value === 'connected')
const statusLabel = computed(() => {
  if (connectionPhase.value === 'connecting') return 'Connecting'
  if (connected.value) return 'Connected'
  if (connectionPhase.value === 'error') return 'Connection failed'
  return 'Not connected'
})

function describeError(value: unknown, fallback: string) {
  if (value instanceof Error && value.message) return value.message
  if (typeof value === 'string' && value) return value
  return fallback
}

async function scrollToLatest() {
  await nextTick()
  conversation.value?.scrollTo({
    top: conversation.value.scrollHeight,
    behavior: 'smooth',
  })
}

function waitForPaint() {
  return new Promise<void>((resolve) => {
    requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
  })
}

function wait(milliseconds: number) {
  return new Promise<void>((resolve) => window.setTimeout(resolve, milliseconds))
}

function startAgentActivity() {
  const stages = [
    'I’m on it',
    'Reading project context',
    'Working through the request',
    'Checking the result',
  ]
  let stage = 0
  agentActivity.value = stages[stage]
  activityTimer = setInterval(() => {
    stage = Math.min(stage + 1, stages.length - 1)
    agentActivity.value = stages[stage]
  }, 1400)
}

function stopAgentActivity() {
  if (activityTimer !== null) {
    clearInterval(activityTimer)
    activityTimer = null
  }
}

function activeChat() {
  return chats.value.find((chat) => chat.id === activeChatId.value) ?? null
}

function threadTitle(text: string) {
  const title = text.replace(/\s+/g, ' ').trim()
  return title.length > 42 ? `${title.slice(0, 41)}…` : title || 'New agent'
}

async function persistActiveChat() {
  const chat = activeChat()
  if (!chat) return
  const saved = await saveAgentChat({
    ...chat,
    title: chat.title === 'New agent' && messages.value.some((message) => message.role === 'user')
      ? threadTitle(messages.value.find((message) => message.role === 'user')?.text ?? '')
      : chat.title,
    messages: messages.value.map((message) => ({ ...message })),
    responseId: responseId.value,
    model: activeModel.value,
  })
  const index = chats.value.findIndex((item) => item.id === saved.id)
  if (index >= 0) chats.value[index] = saved
  chats.value.sort((left, right) => right.updatedAt - left.updatedAt)
}

async function selectChat(chat: AgentChat) {
  if (sending.value || chat.id === activeChatId.value) return
  activeChatId.value = chat.id
  messages.value = chat.messages.map((message) => ({ ...message }))
  responseId.value = chat.responseId
  activeModel.value = chat.model || modelId
  error.value = ''
  void scrollToLatest()
}

async function createNewChat() {
  const chat = await createAgentChat()
  chats.value.unshift(chat)
  activeChatId.value = chat.id
  messages.value = []
  responseId.value = null
  activeModel.value = modelId
  return chat
}

async function removeChat(event: MouseEvent, chat: AgentChat) {
  event.stopPropagation()
  if (sending.value) return
  try {
    await deleteAgentChat(chat.id)
    chats.value = chats.value.filter((item) => item.id !== chat.id)
    if (activeChatId.value === chat.id) {
      const next = chats.value[0]
      if (next) {
        await selectChat(next)
      } else {
        activeChatId.value = null
        messages.value = []
        responseId.value = null
      }
    }
  } catch (reason) {
    error.value = describeError(reason, 'This chat could not be removed.')
  }
}

function acceptReply(reply: {
  responseId: string
  text: string
  model: string
  projectName: string
  projectRoot: string
  projectFileCount: number
}) {
  responseId.value = reply.responseId
  activeModel.value = reply.model
  projectName.value = reply.projectName
  projectRoot.value = reply.projectRoot
  projectFileCount.value = reply.projectFileCount
  messages.value.push({ role: 'agent', text: reply.text })
  connectionPhase.value = 'connected'
  error.value = ''
  showKeySetup.value = false
  void persistActiveChat()
  void scrollToLatest()
}

async function saveAndConnect() {
  if (!canSaveKey.value || connectionPhase.value === 'connecting') return
  connectionPhase.value = 'connecting'
  error.value = ''
  try {
    const reply = await connectAndStartAgent(apiKey.value)
    apiKey.value = ''
    keyVisible.value = false
    acceptReply(reply)
  } catch (reason) {
    connectionPhase.value = 'error'
    error.value = describeError(reason, 'OpenAI could not validate this key.')
  }
}

async function startSavedSession(clearThread = false, promptForKeyOnError = true) {
  connectionPhase.value = 'connecting'
  error.value = ''
  if (clearThread) {
    messages.value = []
    responseId.value = null
  }
  try {
    acceptReply(await startSavedAgent())
  } catch (reason) {
    connectionPhase.value = 'error'
    showKeySetup.value = promptForKeyOnError
    error.value = describeError(reason, 'The saved OpenAI key could not start an agent.')
  }
}

async function newAgent() {
  try {
    await createNewChat()
  } catch (reason) {
    error.value = describeError(reason, 'The new agent chat could not be saved.')
    return
  }
  if (!connected.value) {
    showKeySetup.value = true
    return
  }
  await startSavedSession(false)
}

async function sendMessage() {
  const text = prompt.value.trim()
  if (!connected.value || !text || sending.value) return
  if (!activeChat()) {
    try {
      await createNewChat()
    } catch (reason) {
      error.value = describeError(reason, 'A new chat could not be created.')
      return
    }
  }
  messages.value.push({ role: 'user', text })
  try {
    await persistActiveChat()
  } catch (reason) {
    error.value = describeError(reason, 'This message could not be saved to the project history.')
    return
  }
  prompt.value = ''
  sending.value = true
  startAgentActivity()
  error.value = ''
  await scrollToLatest()
  await waitForPaint()
  const activityStartedAt = performance.now()
  try {
    const reply = await sendAgentMessage(text, responseId.value)
    const remainingActivity = minimumActivityMs - (performance.now() - activityStartedAt)
    if (remainingActivity > 0) await wait(remainingActivity)
    acceptReply(reply)
  } catch (reason) {
    error.value = describeError(reason, 'The agent could not complete that request.')
  } finally {
    stopAgentActivity()
    sending.value = false
    void scrollToLatest()
  }
}

function handleComposerKeydown(event: KeyboardEvent) {
  if (event.key === 'Enter' && !event.shiftKey) {
    event.preventDefault()
    void sendMessage()
  }
}

async function refreshSelection() {
  const snapshot = await getSnapshot()
  selectedParticleId.value = Math.max(
    0,
    Math.min(31, Math.round(snapshot.values['interaction.selected'] ?? selectedParticleId.value)),
  )
}

async function selectParticle(agent: ParticleAgent) {
  selectedParticleId.value = agent.id
  await setControl('interaction.select_particle', agent.id)
}

function openParticleRemoval(event: MouseEvent, agent: ParticleAgent) {
  event.stopPropagation()
  removalCandidate.value = agent
  removeAgentLink.value = true
  removeMetalParticle.value = false
}

async function confirmParticleRemoval() {
  const particle = removalCandidate.value
  if (!particle || (!removeAgentLink.value && !removeMetalParticle.value)) return
  try {
    const nextRoster = particleAgents.value.map((agent) => {
      if (agent.id !== particle.id) return agent
      return {
        ...agent,
        agentLinked: removeAgentLink.value ? false : agent.agentLinked,
        fields: {
          ...agent.fields,
          ...(removeMetalParticle.value ? { metalActive: false } : {}),
        },
      }
    })
    if (removeMetalParticle.value) {
      await setControl('interaction.remove_particle', particle.id)
    }
    particleAgents.value = await saveParticleAgents(nextRoster)
    removalCandidate.value = null
  } catch (reason) {
    error.value = describeError(reason, 'The particle could not be removed.')
  }
}

onMounted(async () => {
  rosterTimer = window.setInterval(() => {
    if (!removalCandidate.value) {
      void refreshSelection().catch(() => undefined)
    }
  }, 100)
  try {
    rosterUnlisten = await subscribeParticleAgents((roster) => {
      particleAgents.value = roster
    })
    resetUnlisten = await subscribeBaselineReset(() => {
      chats.value = []
      activeChatId.value = null
      messages.value = []
      responseId.value = null
      error.value = ''
    })
    particleAgents.value = await loadParticleAgents()
    await refreshSelection()
    chats.value = await loadAgentChats()
    if (chats.value.length) {
      await selectChat(chats.value[0])
    }
    if (await hasAgentApiKey()) {
      if (!messages.value.length) await startSavedSession(false, false)
      else connectionPhase.value = 'connected'
    } else {
      showKeySetup.value = true
    }
  } catch (reason) {
    connectionPhase.value = 'error'
    showKeySetup.value = false
    error.value = describeError(reason, 'The macOS Keychain could not be read.')
  }
})

onBeforeUnmount(() => {
  stopAgentActivity()
  if (rosterTimer !== null) clearInterval(rosterTimer)
  rosterUnlisten?.()
  resetUnlisten?.()
})
</script>

<template>
  <main class="agents-shell">
    <header class="app-header">
      <div class="brand">
        <span class="brand-mark"><api-outlined /></span>
        <div>
          <h1>Pqo Agents</h1>
          <p>AI workspace for GPU systems</p>
        </div>
      </div>
      <div class="header-actions">
        <button class="new-agent new-agent--header" type="button" aria-label="New agent" title="New agent" @click="newAgent">
          <plus-outlined />
        </button>
        <div class="connection-status" :data-state="connectionPhase">
          <i></i>{{ statusLabel }}
        </div>
      </div>
    </header>

    <div class="workspace">
      <aside class="sidebar">
        <div class="sidebar-section particle-roster">
          <p class="eyebrow">PARTICLES</p>
          <div
            v-for="agent in linkedParticleAgents"
            :key="agent.id"
            class="particle-agent"
            :class="{ 'particle-agent--active': agent.id === selectedParticleId }"
          >
            <button class="particle-agent-select" type="button" @click="selectParticle(agent)">
              <i :data-type="agent.type"></i>
              <span>
                <strong>{{ agent.name }}</strong>
                <small>{{ agent.type }}{{ agent.fields.metalActive === false ? ' · removed from view' : '' }}</small>
              </span>
            </button>
            <button
              class="particle-agent-remove"
              type="button"
              :aria-label="`Remove ${agent.name}`"
              :title="`Remove ${agent.name}`"
              @click="openParticleRemoval($event, agent)"
            >
              <delete-outlined />
            </button>
          </div>
        </div>
        <div class="sidebar-section">
          <p class="eyebrow">RECENT</p>
          <div
            v-for="chat in chats"
            :key="chat.id"
            class="thread"
            :class="{ 'thread--active': chat.id === activeChatId }"
          >
            <button class="thread-select" type="button" @click="selectChat(chat)">
              <span class="thread-icon"><api-outlined /></span>
              <span>
                <strong>{{ chat.title }}</strong>
                <small>{{ chat.messages.length ? chat.model : 'New session' }}</small>
              </span>
            </button>
            <button class="thread-delete" type="button" :aria-label="`Delete ${chat.title}`" :title="`Delete ${chat.title}`" @click="removeChat($event, chat)">
              <delete-outlined />
            </button>
          </div>
        </div>

        <div class="sidebar-footer">
          <button class="key-button" type="button" @click="showKeySetup = true">
            <key-outlined />
            API key
            <check-circle-filled v-if="connected" class="key-valid" />
          </button>
          <p><safety-certificate-outlined /> Stored in macOS Keychain</p>
        </div>
      </aside>

      <section class="chat">
        <header class="chat-header">
          <div class="model-pill">
            <span>{{ modelLabel }}</span>
            <small>medium</small>
            <small class="live-edit">live edit</small>
          </div>
        </header>

        <section ref="conversation" class="conversation" aria-live="polite">
          <div v-if="connectionPhase === 'connecting' && !messages.length" class="loading-state">
            <span class="agent-avatar"><api-outlined /></span>
            <div class="loading-dots"><i></i><i></i><i></i></div>
            <p>Starting Pqo Agent with {{ modelLabel }}…</p>
          </div>

          <div v-else-if="!messages.length && !connected" class="welcome">
            <span class="welcome-mark"><api-outlined /></span>
            <h3>Connect Pqo Agents</h3>
            <p>Use your OpenAI API key to start a project-aware assistant for building and diagnosing Pqo applications.</p>
            <button type="button" @click="showKeySetup = true">
              <key-outlined />
              Connect OpenAI
            </button>
          </div>

          <div v-else class="message-list">
            <article
              v-for="(message, index) in messages"
              :key="index"
              class="message"
              :class="`message--${message.role}`"
            >
              <span v-if="message.role === 'agent'" class="message-avatar"><api-outlined /></span>
              <div>
                <strong>{{ message.role === 'agent' ? 'Pqo Agent' : 'You' }}</strong>
                <p>{{ message.text }}</p>
              </div>
            </article>

            <article v-if="sending" class="message message--agent">
              <span class="message-avatar"><api-outlined /></span>
              <div>
                <strong>Pqo Agent</strong>
                <div class="agent-activity">
                  <span>{{ agentActivity }}</span>
                  <div class="loading-dots loading-dots--inline"><i></i><i></i><i></i></div>
                </div>
              </div>
            </article>
          </div>
        </section>

        <footer class="composer-area">
          <div v-if="error" class="error-banner">
            <span>{{ error }}</span>
            <button type="button" @click="showKeySetup = true">Review API key</button>
          </div>
          <div class="composer" :class="{ 'composer--disabled': !connected }">
            <textarea
              v-model="prompt"
              :disabled="!connected || sending"
              :placeholder="connected ? 'Message Pqo Agent' : 'Connect OpenAI to begin'"
              rows="1"
              @keydown="handleComposerKeydown"
            ></textarea>
            <button
              type="button"
              :disabled="!connected || !prompt.trim() || sending"
              aria-label="Send message"
              @click="sendMessage"
            >
              <arrow-up-outlined />
            </button>
          </div>
          <p>{{ activeModel }} · medium reasoning · read/write · Metal hot reload · {{ projectFileCount }} files</p>
        </footer>
      </section>
    </div>

    <div v-if="removalCandidate" class="modal-backdrop" @click.self="removalCandidate = null">
      <section class="key-dialog removal-dialog" role="dialog" aria-modal="true" aria-labelledby="particle-removal-title">
        <button class="dialog-close" type="button" aria-label="Close" @click="removalCandidate = null">×</button>
        <span class="dialog-icon"><delete-outlined /></span>
        <h3 id="particle-removal-title">Remove {{ removalCandidate.name }}?</h3>
        <p>Choose where this particle should be removed. These actions are independent.</p>
        <label class="removal-option">
          <input v-model="removeAgentLink" type="checkbox" />
          <span>
            <strong>Remove link to Agents</strong>
            <small>Hide it here while preserving metadata for relinking.</small>
          </span>
        </label>
        <label class="removal-option">
          <input v-model="removeMetalParticle" type="checkbox" />
          <span>
            <strong>Remove from Metal view</strong>
            <small>Deactivate the particle and make its slot reusable.</small>
          </span>
        </label>
        <div class="removal-actions">
          <button type="button" @click="removalCandidate = null">Cancel</button>
          <button
            class="remove-confirm"
            type="button"
            :disabled="!removeAgentLink && !removeMetalParticle"
            @click="confirmParticleRemoval"
          >
            Remove
          </button>
        </div>
      </section>
    </div>

    <div v-if="showKeySetup" class="modal-backdrop" @click.self="connected && (showKeySetup = false)">
      <section class="key-dialog" role="dialog" aria-modal="true" aria-labelledby="key-dialog-title">
        <button v-if="connected" class="dialog-close" type="button" aria-label="Close" @click="showKeySetup = false">×</button>
        <span class="dialog-icon"><key-outlined /></span>
        <h3 id="key-dialog-title">{{ connected ? 'Update API key' : 'Connect to OpenAI' }}</h3>
        <p>Your key is validated directly with OpenAI, then stored in macOS Keychain. It is never written into this project.</p>
        <label for="openai-key">OpenAI API key</label>
        <div class="key-input">
          <input
            id="openai-key"
            v-model="apiKey"
            :type="keyVisible ? 'text' : 'password'"
            placeholder="sk-…"
            autocomplete="off"
            @keydown.enter="saveAndConnect"
          />
          <button type="button" @click="keyVisible = !keyVisible">{{ keyVisible ? 'Hide' : 'Show' }}</button>
        </div>
        <p v-if="error" class="dialog-error">{{ error }}</p>
        <button
          class="connect-button"
          type="button"
          :disabled="!canSaveKey || connectionPhase === 'connecting'"
          @click="saveAndConnect"
        >
          <span v-if="connectionPhase === 'connecting'" class="spinner"></span>
          {{ connectionPhase === 'connecting' ? 'Validating…' : connected ? 'Validate & reconnect' : 'Validate & connect' }}
        </button>
        <small>Requests use {{ modelId }} with medium reasoning.</small>
      </section>
    </div>
  </main>
</template>
