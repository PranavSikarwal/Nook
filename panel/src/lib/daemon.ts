import { Channel, invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type {
  AttachmentInput,
  ChatSummary,
  ChatTranscript,
  DaemonEvent,
  SettingsInfo,
  SettingsPayload,
} from './types'

export async function pingDaemon(): Promise<boolean> {
  try {
    return await invoke<boolean>('ping_daemon')
  } catch {
    return false
  }
}

export async function listChats(): Promise<ChatSummary[]> {
  return await invoke<ChatSummary[]>('list_chats')
}

export async function getChat(chatId: string): Promise<ChatTranscript> {
  return await invoke<ChatTranscript>('get_chat', { chatId })
}

export async function deleteChat(chatId: string): Promise<boolean> {
  return await invoke<boolean>('delete_chat', { chatId })
}

export async function sendMessage(
  chatId: string,
  text: string,
  attachments: AttachmentInput[] = [],
  onEvent?: (event: DaemonEvent) => void,
): Promise<void> {
  const channel = new Channel<DaemonEvent>()
  if (onEvent) {
    channel.onmessage = onEvent
  }
  await invoke('send_message', {
    chatId,
    text,
    attachments,
    onEvent: channel,
  })
}

export async function startDrag(): Promise<void> {
  try {
    await invoke('start_drag')
  } catch {
    // fallback
  }
}

export async function cancelMessage(targetId?: string): Promise<void> {
  await invoke('cancel_message', { targetId })
}

export async function sendApprovalDecision(
  chatId: string,
  callId: string,
  action: string,
): Promise<void> {
  await invoke('send_approval_decision', { chatId, callId, action })
}

export async function getSettings(): Promise<SettingsInfo> {
  return await invoke<SettingsInfo>('get_settings')
}

export async function setSettings(payload: SettingsPayload): Promise<SettingsInfo> {
  return await invoke<SettingsInfo>('set_settings', { payload })
}

export async function setWindowSize(width: number, height: number): Promise<void> {
  try {
    await invoke('set_window_size', { width, height })
  } catch {
    // Ignore in non-tauri preview environments
  }
}

export async function toggleWindow(): Promise<void> {
  try {
    await invoke('toggle_window')
  } catch {
    // Ignore in non-tauri preview environments
  }
}

export function subscribeToDaemonEvents(
  callback: (event: DaemonEvent) => void,
): Promise<UnlistenFn> {
  return listen<DaemonEvent>('daemon_event', (event) => {
    callback(event.payload)
  })
}
