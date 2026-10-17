export type MessageRole = 'user' | 'assistant'
export type MessageStatus = 'streaming' | 'complete' | 'cancelled' | 'error'
export type AttachmentKind = 'image' | 'text' | 'pdf'

export interface Attachment {
  id: string
  kind: AttachmentKind
  name: string
  mime: string
  size_bytes: number
  path?: string
}

export interface AttachmentInput {
  name: string
  mime: string
  data_base64?: string
  file_path?: string
}

export interface ErrorInfo {
  code: string
  message: string
  retryable: boolean
}

export type ToolCallStatus = 'running' | 'finished'

export interface ToolCall {
  call_id: string
  name: string
  arguments: string
  status: ToolCallStatus
  result?: string
}

export interface ChatMessage {
  id: string
  message_id?: string
  role: MessageRole
  text: string
  status: MessageStatus
  error?: ErrorInfo
  attachments: Attachment[]
  tool_calls?: ToolCall[]
  created_at: string
}

export interface ChatSummary {
  chat_id: string
  title: string
  updated_at: string
}

export interface ChatTranscript {
  chat_id: string
  title: string
  messages: ChatMessage[]
}

export interface SettingsInfo {
  base_url: string
  model: string
  has_api_key: boolean
}

export interface SettingsPayload {
  base_url: string
  model: string
  api_key?: string
}

export interface ApprovalAction {
  id: string
  label: string
  description: string
}

export interface ApprovalRequest {
  id: string
  chat_id: string
  message_id: string
  call_id: string
  tool_name: string
  arguments: string
  explanation: string
  resource_summary: string
  actions: ApprovalAction[]
}

export interface GrantInfo {
  grant_id: string
  tool_id: string
  tool_name: string
  scope: string
  description: string
}

export interface TierChoice {
  tier: string
  label: string
  effect: string
}

export interface ToolPolicyInfo {
  tool_id: string
  display_name: string
  default_tier: string
  tier: string
  choices: TierChoice[]
}

export interface Permissions {
  grants: GrantInfo[]
  tools: ToolPolicyInfo[]
}

export type DaemonEvent =
  | { type: 'message_started'; id: string; chat_id: string; message_id: string }
  | { type: 'text_delta'; id: string; chat_id: string; message_id: string; text: string }
  | { type: 'tool_call_started'; id: string; chat_id: string; message_id: string; call_id: string; name: string; arguments: string }
  | { type: 'tool_call_finished'; id: string; chat_id: string; message_id: string; call_id: string; result: string }
  | ({ type: 'approval_requested' } & ApprovalRequest)
  | { type: 'message_finished'; id: string; chat_id: string; message_id: string; status: 'complete' | 'cancelled' | 'error' }
  | { type: 'chat_titled'; chat_id: string; title: string }
  | { type: 'error'; id: string; error: ErrorInfo }
