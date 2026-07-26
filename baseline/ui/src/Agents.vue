<script setup lang="ts">
import { computed, ref } from 'vue'
import {
  Badge as ABadge,
  Button as AButton,
  ConfigProvider as AConfigProvider,
  Input as AInput,
  Select as ASelect,
  theme,
} from 'ant-design-vue'
import {
  ApiOutlined,
  ArrowUpOutlined,
  KeyOutlined,
  PlusOutlined,
  SafetyCertificateOutlined,
} from '@ant-design/icons-vue'

const apiKey = ref('')
const model = ref('GPT-5')
const prompt = ref('')
const keyVisible = ref(false)

const readyForConnection = computed(() => apiKey.value.trim().length > 0)
const { compactAlgorithm, darkAlgorithm } = theme
const panelTheme = {
  algorithm: [darkAlgorithm, compactAlgorithm],
  token: {
    colorPrimary: '#26a8e8',
    colorInfo: '#26a8e8',
    colorSuccess: '#89d185',
    colorBgBase: '#111315',
    colorBgContainer: '#1b1f22',
    colorBgElevated: '#24292d',
    colorBorder: '#343b40',
    colorText: '#e2e6e9',
    colorTextSecondary: '#929ba4',
    fontSize: 13,
    borderRadius: 5,
  },
}
</script>

<template>
  <a-config-provider :theme="panelTheme">
    <main class="agents-shell">
      <header class="agents-header">
        <div class="brand">
          <span class="brand-mark"><api-outlined /></span>
          <div>
            <h1>Loom Agents</h1>
            <p>Build, inspect, and evolve GPU systems</p>
          </div>
        </div>
        <a-badge status="processing" text="Agent workspace" />
      </header>

      <section class="workspace">
        <aside class="rail">
          <a-button class="new-thread" block>
            <template #icon><plus-outlined /></template>
            New agent
          </a-button>
          <p>WORKSPACE</p>
          <button class="thread thread--active">
            <span>Baseline particle</span>
            <small>New session</small>
          </button>
          <button class="thread" disabled>
            <span>GPU diagnostics</span>
            <small>Coming soon</small>
          </button>
          <footer><safety-certificate-outlined /> Local-first agent shell</footer>
        </aside>

        <section class="chat">
          <div class="chat-topline">
            <div>
              <span>ACTIVE AGENT</span>
              <h2>Baseline builder</h2>
            </div>
            <a-select v-model:value="model" :options="[{ value: 'GPT-5', label: 'GPT-5' }]" />
          </div>

          <div class="empty-state">
            <span class="empty-icon"><api-outlined /></span>
            <h3>Your Loom agent is ready</h3>
            <p>
              Connect an OpenAI API key to start an agent session with project-aware
              prompts, code actions, and GPU diagnostics.
            </p>
            <div class="capabilities">
              <span>Project context</span><span>GPU inspection</span><span>Code actions</span>
            </div>
          </div>

          <div class="connection-card">
            <div class="connection-heading">
              <div>
                <key-outlined />
                <div><strong>Connect OpenAI</strong><span>Session-only key · no network call in this stub</span></div>
              </div>
              <small :class="{ ready: readyForConnection }">
                {{ readyForConnection ? 'Ready to connect' : 'Key required' }}
              </small>
            </div>
            <a-input
              v-model:value="apiKey"
              :type="keyVisible ? 'text' : 'password'"
              placeholder="sk-..."
              autocomplete="off"
            >
              <template #suffix>
                <button class="show-key" type="button" @click="keyVisible = !keyVisible">
                  {{ keyVisible ? 'Hide' : 'Show' }}
                </button>
              </template>
            </a-input>
          </div>

          <div class="composer">
            <a-input
              v-model:value="prompt"
              placeholder="Ask your Loom agent to inspect, design, or build…"
              disabled
            />
            <a-button type="primary" disabled aria-label="Send prompt">
              <template #icon><arrow-up-outlined /></template>
            </a-button>
          </div>
          <p class="composer-note">Chat transport and tool execution will be connected in the next step.</p>
        </section>
      </section>
    </main>
  </a-config-provider>
</template>
