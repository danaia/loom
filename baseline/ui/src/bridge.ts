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

export interface AgentReply {
  responseId: string
  text: string
  model: string
  projectName: string
  projectRoot: string
  projectFileCount: number
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
