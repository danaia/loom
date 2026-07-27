import { invoke } from '@tauri-apps/api/core'

export interface PanelSnapshot {
  connected: boolean
  values: Record<string, number>
}

export function getSnapshot(): Promise<PanelSnapshot> {
  return invoke<PanelSnapshot>('get_snapshot')
}

export function setControl(name: string, value: number): Promise<void> {
  return invoke<void>('set_control', { name, value })
}

export function openAgentsWindow(): Promise<void> {
  return invoke<void>('open_agents_window')
}

export interface ParticleAgentRecord {
  id: number
  schemaVersion: number
  name: string
  type: string
  agentLinked: boolean
  skills: string[]
  fields: Record<string, unknown>
}

export function loadParticleAgentRecords(): Promise<ParticleAgentRecord[]> {
  return invoke<ParticleAgentRecord[]>('load_particle_agents')
}

export function saveParticleAgentRecords(
  particles: ParticleAgentRecord[],
): Promise<ParticleAgentRecord[]> {
  return invoke<ParticleAgentRecord[]>('save_particle_agents', { particles })
}

export function resetBaselineData(): Promise<ParticleAgentRecord[]> {
  return invoke<ParticleAgentRecord[]>('reset_baseline_data')
}

export interface AgentReply {
  responseId: string
  text: string
  model: string
  projectName: string
  projectRoot: string
  projectFileCount: number
}

export interface AgentChatMessage {
  role: 'user' | 'agent'
  text: string
}

export interface AgentChat {
  id: string
  title: string
  messages: AgentChatMessage[]
  responseId: string | null
  model: string
  createdAt: number
  updatedAt: number
}

export function loadAgentChats(): Promise<AgentChat[]> {
  return invoke<AgentChat[]>('load_agent_chats')
}

export function createAgentChat(): Promise<AgentChat> {
  return invoke<AgentChat>('create_agent_chat')
}

export function saveAgentChat(chat: AgentChat): Promise<AgentChat> {
  return invoke<AgentChat>('save_agent_chat', { chat })
}

export function deleteAgentChat(chatId: string): Promise<void> {
  return invoke<void>('delete_agent_chat', { chatId })
}

export function connectAndStartAgent(apiKey: string): Promise<AgentReply> {
  return invoke<AgentReply>('connect_and_start_agent', { apiKey })
}

export function hasAgentApiKey(): Promise<boolean> {
  return invoke<boolean>('has_agent_api_key')
}

export function startSavedAgent(): Promise<AgentReply> {
  return invoke<AgentReply>('start_saved_agent')
}

export function sendAgentMessage(
  message: string,
  previousResponseId: string | null,
): Promise<AgentReply> {
  return invoke<AgentReply>('send_agent_message', { message, previousResponseId })
}
