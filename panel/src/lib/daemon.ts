import { Channel, invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type {
  AttachmentInput,
  ChatSummary,
  ChatTranscript,
  DaemonEvent,
  Permissions,
  SettingsInfo,
  SettingsPayload,
} from './types'

const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
const isBrowserMock = !isTauri && import.meta.env.VITE_ENABLE_BROWSER_MOCK === 'true'

let mockPendingApproval: ((action: string) => void) | null = null
const previewSettings: SettingsInfo = {
  base_url: 'https://example.invalid/v1',
  model: 'preview-model',
  has_api_key: false,
}

export async function pingDaemon(): Promise<boolean> {
  if (isBrowserMock) return true
  if (!isTauri) return false
  try {
    return await invoke<boolean>('ping_daemon')
  } catch {
    return false
  }
}

export async function listChats(): Promise<ChatSummary[]> {
  if (isBrowserMock) {
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
  if (isBrowserMock) {
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
  if (isBrowserMock) return true
  return await invoke<boolean>('delete_chat', { chatId })
}

export async function sendMessage(
  chatId: string,
  text: string,
  attachments: AttachmentInput[] = [],
  onEvent?: (event: DaemonEvent) => void,
): Promise<string> {
  if (isBrowserMock) {
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
          actions: [
            { id: 'allow_once', label: 'Allow once', description: 'Allow this exact request one time.' },
            {
              id: 'allow_for_chat_host',
              label: 'Allow for this chat & host',
              description: 'Allow this tool to fetch from this host for the rest of this chat.',
            },
            { id: 'deny', label: 'Deny', description: 'Reject this request. The tool does not run.' },
          ],
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
              type: 'tool_call_started',
              id: reqId,
              chat_id: chatId,
              message_id: msgId,
              call_id: 'call_preview_1',
              name: 'nook:web_fetch',
              arguments: JSON.stringify({ url }),
            })
            onEvent?.({
              type: 'tool_call_finished',
              id: reqId,
              chat_id: chatId,
              message_id: msgId,
              call_id: 'call_preview_1',
              result: 'Preview page text',
            })
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
  if (isBrowserMock) return
  try {
    await invoke('start_drag')
  } catch {
    // fallback
  }
}

export async function cancelMessage(targetId?: string): Promise<void> {
  if (isBrowserMock) {
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
  if (isBrowserMock) {
    if (mockPendingApproval) {
      mockPendingApproval(action)
    }
    return
  }
  await invoke('send_approval_decision', { chatId, callId, action })
}

const previewPermissions: Permissions = {
  grants: [
    {
      grant_id: 'preview-grant',
      tool_id: 'web_fetch',
      tool_name: 'Web fetch',
      scope: 'persistent_host',
      description: 'Always allow Web fetch for docs.example.com, in every chat',
    },
  ],
  tools: [
    {
      tool_id: 'nook:web_search',
      display_name: 'Web search',
      default_tier: 'allow',
      tier: 'default',
      choices: [
        { tier: 'default', label: 'Default', effect: 'Nook runs this tool without asking.' },
        { tier: 'ask', label: 'Ask every time', effect: 'Nook asks before it uses this tool.' },
        { tier: 'deny', label: 'Never allow', effect: 'Nook never runs this tool.' },
      ],
    },
  ],
}

export async function getPermissions(): Promise<Permissions> {
  if (isBrowserMock) return structuredClone(previewPermissions)
  return await invoke<Permissions>('get_permissions')
}

export async function revokeGrant(grantId: string): Promise<Permissions> {
  if (isBrowserMock) {
    previewPermissions.grants = previewPermissions.grants.filter((grant) => grant.grant_id !== grantId)
    return structuredClone(previewPermissions)
  }
  return await invoke<Permissions>('revoke_grant', { grantId })
}

export async function setToolPreference(toolId: string, tier: string): Promise<Permissions> {
  if (isBrowserMock) {
    previewPermissions.tools = previewPermissions.tools.map((tool) =>
      tool.tool_id === toolId ? { ...tool, tier } : tool,
    )
    return structuredClone(previewPermissions)
  }
  return await invoke<Permissions>('set_tool_preference', { toolId, tier })
}

export async function getSettings(): Promise<SettingsInfo> {
  if (isBrowserMock) return { ...previewSettings }
  return await invoke<SettingsInfo>('get_settings')
}

export async function setSettings(payload: SettingsPayload): Promise<SettingsInfo> {
  if (isBrowserMock) {
    return {
      base_url: payload.base_url,
      model: payload.model,
      has_api_key: false,
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
