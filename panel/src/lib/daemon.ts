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

const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

// Browser mock state for interactive preview & Playwright automated testing
let mockPendingApproval: ((action: string) => void) | null = null

export async function pingDaemon(): Promise<boolean> {
  if (!isTauri) return true
  try {
    return await invoke<boolean>('ping_daemon')
  } catch {
    return false
  }
}

export async function listChats(): Promise<ChatSummary[]> {
  if (!isTauri) {
    return [
      {
        chat_id: 'preview-chat-1',
        title: 'Claude Memory Tool Documentation Summary',
        updated_at: new Date().toISOString(),
      },
      {
        chat_id: 'preview-chat-2',
        title: 'Rust Release Notes Summary',
        updated_at: new Date(Date.now() - 3600000).toISOString(),
      },
    ]
  }
  return await invoke<ChatSummary[]>('list_chats')
}

export async function getChat(chatId: string): Promise<ChatTranscript> {
  if (!isTauri) {
    return {
      chat_id: chatId,
      title: 'Previous Conversation',
      messages: [
        {
          id: 'prev-user-msg',
          role: 'user',
          text: 'What are the main features of Nook?',
          status: 'complete',
          attachments: [],
          created_at: new Date(Date.now() - 3600000).toISOString(),
        },
        {
          id: 'prev-asst-msg',
          role: 'assistant',
          text: 'Nook is a concise desktop assistant with web tools, Cedar authorization, and focused keyboard shortcuts.',
          status: 'complete',
          attachments: [],
          created_at: new Date(Date.now() - 3590000).toISOString(),
        },
      ],
    }
  }
  return await invoke<ChatTranscript>('get_chat', { chatId })
}

export async function deleteChat(chatId: string): Promise<boolean> {
  if (!isTauri) return true
  return await invoke<boolean>('delete_chat', { chatId })
}

export async function sendMessage(
  chatId: string,
  text: string,
  attachments: AttachmentInput[] = [],
  onEvent?: (event: DaemonEvent) => void,
): Promise<string> {
  if (!isTauri) {
    const reqId = crypto.randomUUID()
    const msgId = crypto.randomUUID()
    setTimeout(() => {
      onEvent?.({
        type: 'message_started',
        id: reqId,
        chat_id: chatId,
        message_id: msgId,
      })

      if (text.includes('http://') || text.includes('https://')) {
        const urlMatch = /https?:\/\/[^\s]+/.exec(text)
        const url = urlMatch?.[0] ?? 'https://platform.claude.com'
        const host = new URL(url).hostname

        // Emit approval request event
        onEvent?.({
          type: 'approval_requested',
          id: reqId,
          chat_id: chatId,
          message_id: msgId,
          call_id: 'call_preview_1',
          tool_name: 'nook:web_fetch',
          arguments: JSON.stringify({ url }),
          explanation: `Web fetch requires approval to access ${host}`,
          resource_summary: url,
          actions: ['allow_once', 'allow_for_chat_host', 'deny'],
        })

        mockPendingApproval = (action: string) => {
          if (action === 'deny') {
            onEvent?.({
              type: 'text_delta',
              id: reqId,
              chat_id: chatId,
              message_id: msgId,
              text: `*(User denied permission to access ${host}.)*`,
            })
          } else {
            onEvent?.({
              type: 'text_delta',
              id: reqId,
              chat_id: chatId,
              message_id: msgId,
              text: `Here is the summary of **${url}**:\n\nThe Claude Memory Tool allows persistence across conversation sessions.`,
            })
          }
          onEvent?.({
            type: 'message_finished',
            id: reqId,
            chat_id: chatId,
            message_id: msgId,
            status: 'complete',
          })
          mockPendingApproval = null
        }
      } else {
        // Plain text response stream
        setTimeout(() => {
          onEvent?.({
            type: 'text_delta',
            id: reqId,
            chat_id: chatId,
            message_id: msgId,
            text: `Hello! I received your message: "${text}"`,
          })
          onEvent?.({
            type: 'message_finished',
            id: reqId,
            chat_id: chatId,
            message_id: msgId,
            status: 'complete',
          })
        }, 300)
      }
    }, 100)

    return reqId
  }

  const channel = new Channel<DaemonEvent>()
  if (onEvent) {
    channel.onmessage = onEvent
  }
  return await invoke<string>('send_message', {
    chatId,
    text,
    attachments,
    onEvent: channel,
  })
}

export async function startDrag(): Promise<void> {
  if (!isTauri) return
  try {
    await invoke('start_drag')
  } catch {
    // fallback
  }
}

export async function cancelMessage(targetId?: string): Promise<void> {
  if (!isTauri) {
    if (mockPendingApproval) {
      mockPendingApproval('deny')
    }
    return
  }
  await invoke('cancel_message', { targetId })
}

export async function sendApprovalDecision(
  chatId: string,
  callId: string,
  action: string,
): Promise<void> {
  if (!isTauri) {
    if (mockPendingApproval) {
      mockPendingApproval(action)
    }
    return
  }
  await invoke('send_approval_decision', { chatId, callId, action })
}

export async function getSettings(): Promise<SettingsInfo> {
  if (!isTauri) {
    return {
      base_url: 'https://proxy-foundry.centralindia.cloudapp.azure.com/v1',
      model: 'gemini-3.8-flash-high',
      has_api_key: true,
    }
  }
  return await invoke<SettingsInfo>('get_settings')
}

export async function setSettings(payload: SettingsPayload): Promise<SettingsInfo> {
  if (!isTauri) {
    return {
      base_url: payload.base_url,
      model: payload.model,
      has_api_key: Boolean(payload.api_key),
    }
  }
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
