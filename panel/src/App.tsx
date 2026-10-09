import { AlertCircle, X } from 'lucide-react'
import { useEffect, useRef, useState, useCallback } from 'react'
import { AttachmentChips } from './components/AttachmentChips'
import { HistoryDrawer } from './components/HistoryDrawer'
import { InputBar } from './components/InputBar'
import { SettingsModal } from './components/SettingsModal'
import { TranscriptView } from './components/TranscriptView'
import {
  loadShortcuts,
  normalizeKeyboardEvent,
  type ShortcutMap,
} from './lib/shortcuts'
import {
  cancelMessage,
  deleteChat,
  getChat,
  getSettings,
  listChats,
  pingDaemon,
  sendApprovalDecision,
  sendMessage,
  setSettings,
  setWindowSize,
  startDrag,
} from './lib/daemon'
import type {
  ApprovalRequest,
  AttachmentInput,
  ChatMessage,
  ChatSummary,
  DaemonEvent,
  MessageStatus,
  SettingsInfo,
  SettingsPayload,
} from './lib/types'

type ActivePanel = 'none' | 'history' | 'settings'

const WINDOW_WIDTH = 640
const COMPACT_HEIGHT = 56
const ATTACHMENTS_HEIGHT = 110
const DRAWER_HEIGHT = 440
const EXPANDED_HEIGHT = 520

interface ShortcutContext {
  inputRef: React.RefObject<HTMLInputElement | null>
  fileInputRef: React.RefObject<HTMLInputElement | null>
  isStreaming: boolean
  activeApproval: ApprovalRequest | null
  activePanel: ActivePanel
  onNewChat: () => void
  onToggleHistory: () => void
  onToggleSettings: () => void
  onStop: () => void
  onApprovalDecide: (action: string) => void
  onCloseAuxiliary: () => void
}

function executeShortcutAction(action: string, ctx: ShortcutContext): boolean {
  switch (action) {
    case 'focus_input':
      ctx.inputRef.current?.focus()
      return true
    case 'new_chat':
      if (!ctx.isStreaming) ctx.onNewChat()
      return true
    case 'toggle_history':
      ctx.onToggleHistory()
      return true
    case 'open_settings':
      ctx.onToggleSettings()
      return true
    case 'attach_file':
      ctx.fileInputRef.current?.click()
      return true
    case 'cancel_active_work':
      if (ctx.activeApproval) {
        ctx.onApprovalDecide('deny')
        ctx.onStop()
        return true
      }
      if (ctx.isStreaming) {
        ctx.onStop()
        return true
      }
      return false
    case 'close_auxiliary_view':
      if (ctx.activePanel !== 'none') {
        ctx.onCloseAuxiliary()
        return true
      }
      return false
    default:
      return false
  }
}

export default function App() {
  const [activePanel, setActivePanel] = useState<ActivePanel>('none')
  const [chatId, setChatId] = useState<string>(() => crypto.randomUUID())
  const [messages, setMessages] = useState<ChatMessage[]>([])
  const [input, setInput] = useState('')
  const [attachments, setAttachments] = useState<AttachmentInput[]>([])
  const [isStreaming, setIsStreaming] = useState(false)
  const [chats, setChats] = useState<ChatSummary[]>([])
  const [settingsInfo, setSettingsInfo] = useState<SettingsInfo | null>(null)
  const [errorMessage, setErrorMessage] = useState<string | null>(null)
  const [shortcuts, setShortcuts] = useState<ShortcutMap>(loadShortcuts)
  const [activeApproval, setActiveApproval] = useState<ApprovalRequest | null>(null)
  const [isDaemonReady, setIsDaemonReady] = useState(false)

  const activeRequestIdRef = useRef<string | null>(null)
  const activeSendRef = useRef<{ chatId: string; localMsgId: string; requestId?: string } | null>(null)

  const inputRef = useRef<HTMLInputElement>(null)
  const fileInputRef = useRef<HTMLInputElement>(null)

  const isCompact = activePanel === 'none' && messages.length === 0 && attachments.length === 0

  const handleStop = useCallback(async () => {
    setIsStreaming(false)
    setActiveApproval(null)
    const targetId = activeRequestIdRef.current
    const activeMessageId = activeSendRef.current?.localMsgId
    activeRequestIdRef.current = null
    activeSendRef.current = null
    if (activeMessageId) {
      setMessages((prev) =>
        prev.map((message) =>
          message.id === activeMessageId ? { ...message, status: 'cancelled' } : message
        )
      )
    }
    try {
      await cancelMessage(targetId ?? undefined)
    } catch {
      // Ignore cancel error
    }
  }, [])

  const handleNewChat = useCallback(() => {
    if (isStreaming) return
    setChatId(crypto.randomUUID())
    setMessages([])
    setAttachments([])
    setInput('')
    setActivePanel('none')
    setIsStreaming(false)
    setActiveApproval(null)
    activeRequestIdRef.current = null
    activeSendRef.current = null
  }, [isStreaming])

  useEffect(() => {
    let isCancelled = false
    let retryTimer: ReturnType<typeof setTimeout>

    const checkDaemon = async () => {
      const isReady = await pingDaemon()
      if (isCancelled) return
      if (isReady) {
        setIsDaemonReady(true)
        return
      }
      retryTimer = setTimeout(() => void checkDaemon(), 250)
    }

    void checkDaemon()
    return () => {
      isCancelled = true
      clearTimeout(retryTimer)
    }
  }, [])

  // Sync window size with view state
  useEffect(() => {
    if (activePanel !== 'none') {
      void setWindowSize(WINDOW_WIDTH, DRAWER_HEIGHT)
    } else if (messages.length > 0) {
      void setWindowSize(WINDOW_WIDTH, EXPANDED_HEIGHT)
    } else if (attachments.length > 0) {
      void setWindowSize(WINDOW_WIDTH, ATTACHMENTS_HEIGHT)
    } else {
      void setWindowSize(WINDOW_WIDTH, COMPACT_HEIGHT)
    }
  }, [activePanel, messages.length, attachments.length])

  // Load initial settings and history, and refresh when switching panels
  useEffect(() => {
    let isCancelled = false
    if (activePanel === 'settings') {
      void getSettings()
        .then((s) => {
          if (!isCancelled) setSettingsInfo(s)
        })
        .catch((err) => {
          if (!isCancelled) {
            setErrorMessage(err instanceof Error ? err.message : String(err))
          }
        })
    } else if (activePanel === 'history') {
      void listChats()
        .then((c) => {
          if (!isCancelled) setChats(c)
        })
        .catch((err) => {
          if (!isCancelled) {
            setErrorMessage(err instanceof Error ? err.message : String(err))
          }
        })
    }
    return () => {
      isCancelled = true
    }
  }, [activePanel])

  useEffect(() => {
    void getSettings().then(setSettingsInfo).catch(() => {})
    void listChats().then(setChats).catch(() => {})
  }, [])

  const handleApprovalDecide = useCallback(
    async (action: string) => {
      if (!activeApproval) return
      const callId = activeApproval.call_id
      setActiveApproval(null)
      try {
        await sendApprovalDecision(chatId, callId, action)
      } catch (err) {
        setErrorMessage(
          `Failed to submit approval: ${err instanceof Error ? err.message : String(err)}`
        )
      }
    },
    [activeApproval, chatId]
  )

  // Focused in-window keyboard shortcuts dispatcher
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const activeEl = document.activeElement
      if (
        (activeEl as HTMLElement)?.dataset?.recording === 'true' ||
        (activeEl?.tagName === 'BUTTON' && activeEl.textContent?.includes('Press key combo'))
      ) {
        return
      }

      // Universal fallback for Escape
      if (e.key === 'Escape') {
        if (activeApproval) {
          e.preventDefault()
          setActiveApproval(null)
          void handleStop()
          return
        }
        if (activePanel !== 'none') {
          e.preventDefault()
          setActivePanel('none')
          return
        }
      }

      const combo = normalizeKeyboardEvent(e)
      if (!combo) return

      const ctx: ShortcutContext = {
        inputRef,
        fileInputRef,
        isStreaming,
        activeApproval,
        activePanel,
        onNewChat: handleNewChat,
        onToggleHistory: () => setActivePanel((prev) => (prev === 'history' ? 'none' : 'history')),
        onToggleSettings: () => setActivePanel((prev) => (prev === 'settings' ? 'none' : 'settings')),
        onStop: () => void handleStop(),
        onApprovalDecide: (action) => void handleApprovalDecide(action),
        onCloseAuxiliary: () => setActivePanel('none'),
      }

      for (const [action, key] of Object.entries(shortcuts)) {
        if (key.toLowerCase() === combo.toLowerCase() && executeShortcutAction(action, ctx)) {
          e.preventDefault()
          return
        }
      }
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [shortcuts, isStreaming, activePanel, activeApproval, handleApprovalDecide, handleStop, handleNewChat])

  // Native window dragging on non-interactive background areas
  useEffect(() => {
    const handleGlobalMouseDown = (e: MouseEvent) => {
      if (e.button === 0 && !(e.target as HTMLElement).closest('button, input, textarea, a, select')) {
        void startDrag()
      }
    }
    window.addEventListener('mousedown', handleGlobalMouseDown)
    return () => window.removeEventListener('mousedown', handleGlobalMouseDown)
  }, [])

  const handleSend = async (overrideText?: string, overrideAttachments?: readonly AttachmentInput[]) => {
    const textToSend = overrideText ?? input
    if (!textToSend.trim() || isStreaming) return

    const question = textToSend.trim()
    const currentAtts = overrideAttachments ? [...overrideAttachments] : [...attachments]
    const userMsg: ChatMessage = {
      id: crypto.randomUUID(),
      role: 'user',
      text: question,
      status: 'complete',
      attachments: currentAtts.map((a) => ({
        id: crypto.randomUUID(),
        kind: a.mime.startsWith('image/') ? 'image' : 'text',
        name: a.name,
        mime: a.mime,
        size_bytes: 0,
        path: a.file_path,
      })),
      created_at: new Date().toISOString(),
    }

    const localMsgId = crypto.randomUUID()
    const sendChatId = chatId
    const assistantMsg: ChatMessage = {
      id: localMsgId,
      role: 'assistant',
      text: '',
      status: 'streaming',
      attachments: [],
      created_at: new Date().toISOString(),
    }

    setMessages((prev) => [...prev, userMsg, assistantMsg])
    if (overrideText === undefined) {
      setInput('')
      setAttachments([])
    }
    setIsStreaming(true)
    setActiveApproval(null)
    setActivePanel('none')

    activeSendRef.current = { chatId: sendChatId, localMsgId }
    const pendingEvents: DaemonEvent[] = []

    const applyDaemonEvent = (event: DaemonEvent) => {
      if (event.type === 'message_started') {
        setMessages((prev) =>
          prev.map((m) =>
            m.id === localMsgId ? { ...m, message_id: event.message_id } : m
          )
        )
      } else if (event.type === 'text_delta') {
        setMessages((prev) =>
          prev.map((m) =>
            m.id === localMsgId
              ? {
                  ...m,
                  message_id: event.message_id || m.message_id,
                  text: m.text + event.text,
                }
              : m
          )
        )
      } else if (event.type === 'approval_requested') {
        setActiveApproval(event)
        setMessages((prev) =>
          prev.map((m) =>
            m.id === localMsgId ? { ...m, message_id: event.message_id } : m
          )
        )
      } else if (event.type === 'message_finished') {
        setIsStreaming(false)
        setActiveApproval(null)
        activeRequestIdRef.current = null
        activeSendRef.current = null
        let finalStatus: MessageStatus = 'complete'
        if (event.status === 'error') {
          finalStatus = 'error'
        } else if (event.status === 'cancelled') {
          finalStatus = 'cancelled'
        }

        setMessages((prev) =>
          prev.map((m) =>
            m.id === localMsgId
              ? {
                  ...m,
                  status: finalStatus,
                }
              : m
          )
        )
        void listChats().then(setChats).catch(() => {})
      } else if (event.type === 'chat_titled') {
        void listChats().then(setChats).catch(() => {})
      } else if (event.type === 'error') {
        setIsStreaming(false)
        setActiveApproval(null)
        activeRequestIdRef.current = null
        activeSendRef.current = null
        setMessages((prev) =>
          prev.map((m) =>
            m.id === localMsgId
              ? {
                  ...m,
                  status: 'error',
                  error: event.error,
                }
              : m
          )
        )
      }
    }

    const onDaemonEvent = (event: DaemonEvent) => {
      const activeSend = activeSendRef.current
      if (activeSend?.chatId !== sendChatId || activeSend.localMsgId !== localMsgId) return
      if (!activeSend.requestId) {
        pendingEvents.push(event)
        return
      }
      if ('id' in event && event.id !== activeSend.requestId) return
      if (event.type === 'chat_titled' && event.chat_id !== sendChatId) return
      applyDaemonEvent(event)
    }

    try {
      const reqId = await sendMessage(sendChatId, question, currentAtts, onDaemonEvent)
      activeRequestIdRef.current = reqId
      if (activeSendRef.current?.localMsgId === localMsgId) {
        activeSendRef.current.requestId = reqId
        for (const event of pendingEvents) {
          const belongsToRequest = 'id' in event && event.id === reqId
          const belongsToChat = event.type === 'chat_titled' && event.chat_id === sendChatId
          if (belongsToRequest || belongsToChat) applyDaemonEvent(event)
        }
      }
      pendingEvents.length = 0
    } catch (err) {
      setIsStreaming(false)
      setActiveApproval(null)
      activeRequestIdRef.current = null
      activeSendRef.current = null
      setMessages((prev) =>
        prev.map((m) =>
          m.id === localMsgId
            ? {
                ...m,
                status: 'error',
                error: {
                  code: 'internal',
                  message: err instanceof Error ? err.message : String(err),
                  retryable: true,
                },
              }
            : m
        )
      )
    }
  }

  const handleRetry = () => {
    if (isStreaming) return

    const failedIdx = messages.findLastIndex((m) => m.role === 'assistant' && m.status === 'error')
    if (failedIdx === -1) return

    const userMsg = messages[failedIdx - 1]
    if (userMsg?.role !== 'user') return

    const attsInput: AttachmentInput[] = userMsg.attachments
      .filter((a) => a.path)
      .map((a) => ({
        name: a.name,
        mime: a.mime,
        file_path: a.path,
      }))

    setMessages((prev) => prev.slice(0, failedIdx))
    void handleSend(userMsg.text, attsInput)
  }

  const handleAttachFiles = async (files: FileList) => {
    const fileList = Array.from(files).slice(0, 5 - attachments.length)
    const validFiles = fileList.filter((file) => {
      if (file.size > 10 * 1024 * 1024) {
        setErrorMessage(`File ${file.name} exceeds 10MB limit`)
        return false
      }
      return true
    })

    const newItems = await Promise.all(
      validFiles.map((file) => {
        return new Promise<AttachmentInput>((resolve) => {
          const reader = new FileReader()
          reader.onload = () => {
            const res = reader.result as string
            const b64 = res.split(',')[1] ?? ''
            resolve({
              name: file.name,
              mime: file.type || 'text/plain',
              data_base64: b64,
            })
          }
          reader.readAsDataURL(file)
        })
      })
    )
    setAttachments((prev) => [...prev, ...newItems].slice(0, 5))
  }

  const handleAttachPath = (filePath: string) => {
    if (attachments.length >= 5) {
      setErrorMessage('Maximum 5 attachments allowed')
      return
    }
    const name = filePath.split('/').pop() ?? 'attachment'
    const ext = name.split('.').pop()?.toLowerCase()
    let mime = 'text/plain'
    if (ext === 'png') mime = 'image/png'
    else if (ext === 'jpg' || ext === 'jpeg') mime = 'image/jpeg'
    else if (ext === 'webp') mime = 'image/webp'
    else if (ext === 'gif') mime = 'image/gif'
    else if (ext === 'pdf') mime = 'application/pdf'

    setAttachments((prev) => [
      ...prev,
      {
        name,
        mime,
        file_path: filePath,
      },
    ].slice(0, 5))
  }

  const handleSelectChat = async (selectedId: string) => {
    if (isStreaming) {
      await handleStop()
    }
    try {
      const transcript = await getChat(selectedId)
      setChatId(transcript.chat_id)
      const loadedMessages: ChatMessage[] = (transcript.messages || []).map((m) => ({
        id: m.id || m.message_id || crypto.randomUUID(),
        message_id: m.message_id || m.id,
        role: m.role || 'assistant',
        text: m.text ?? '',
        status: m.status ?? 'complete',
        error: m.error,
        attachments: Array.isArray(m.attachments) ? m.attachments : [],
        created_at: m.created_at ?? new Date().toISOString(),
      }))
      setMessages(loadedMessages)
      setActivePanel('none')
    } catch (err) {
      setErrorMessage(`Failed to load chat: ${err instanceof Error ? err.message : String(err)}`)
    }
  }

  const handleDeleteChat = async (targetId: string) => {
    try {
      await deleteChat(targetId)
      setChats((prev) => prev.filter((c) => c.chat_id !== targetId))
      if (chatId === targetId) {
        setChatId(crypto.randomUUID())
        setMessages([])
      }
    } catch (err) {
      setErrorMessage(`Failed to delete chat: ${err instanceof Error ? err.message : String(err)}`)
    }
  }

  const handleSaveSettings = async (payload: SettingsPayload) => {
    const updated = await setSettings(payload)
    setSettingsInfo(updated)
  }

  // In compact mode: only render the single pill without any outer wrapper
  if (isCompact) {
    return (
      <div className="w-full h-full flex items-center justify-center p-0.5 select-none bg-transparent">
        <InputBar
          input={input}
          setInput={setInput}
          onSend={() => void handleSend()}
          onStop={() => void handleStop()}
          onNewChat={handleNewChat}
          onToggleHistory={() => setActivePanel('history')}
          onToggleSettings={() => setActivePanel('settings')}
          onAttachFiles={(files) => void handleAttachFiles(files)}
          onAttachPath={handleAttachPath}
          isStreaming={isStreaming}
          disabled={!isDaemonReady}
          inputRef={inputRef}
          fileInputRef={fileInputRef}
          standalone
        />
      </div>
    )
  }

  // In expanded mode: render unified card with transcript/drawer and input at bottom
  return (
    <div
      data-tauri-drag-region
      className="flex flex-col h-full w-full p-1 select-none bg-transparent"
    >
      <div className="flex flex-col flex-1 rounded-2xl bg-[#161618]/95 backdrop-blur-2xl border border-white/10 shadow-[0_20px_50px_rgba(0,0,0,0.6)] overflow-hidden">
        {errorMessage && (
          <div className="flex items-center justify-between px-3 py-1.5 bg-red-500/10 border-b border-red-500/20 text-red-300 text-xs">
            <div className="flex items-center gap-1.5">
              <AlertCircle className="size-3.5 text-red-400 shrink-0" />
              <span>{errorMessage}</span>
            </div>
            <button
              onClick={() => setErrorMessage(null)}
              type="button"
              className="text-red-400 hover:text-white"
            >
              <X className="size-3" />
            </button>
          </div>
        )}

        {/* Expanded Drawer Area */}
        {activePanel === 'history' && (
          <div className="flex-1 min-h-0">
            <HistoryDrawer
              chats={chats}
              activeChatId={chatId}
              onSelectChat={(id) => void handleSelectChat(id)}
              onDeleteChat={(id) => void handleDeleteChat(id)}
              onClose={() => setActivePanel('none')}
            />
          </div>
        )}

        {activePanel === 'settings' && (
          <div className="flex-1 min-h-0">
            <SettingsModal
              currentSettings={settingsInfo}
              onSave={handleSaveSettings}
              onClose={() => setActivePanel('none')}
              onShortcutsChanged={setShortcuts}
            />
          </div>
        )}

        {activePanel === 'none' && messages.length > 0 && (
          <TranscriptView
            messages={messages}
            isStreaming={isStreaming}
            activeApproval={activeApproval}
            onApprovalDecide={(act) => void handleApprovalDecide(act)}
            onRetry={handleRetry}
          />
        )}

        {/* Attachment chips preview */}
        <AttachmentChips
          attachments={attachments}
          onRemove={(idx) => setAttachments((prev) => prev.filter((_, i) => i !== idx))}
        />

        {/* Unified Input Bar at bottom */}
        <InputBar
          input={input}
          setInput={setInput}
          onSend={() => void handleSend()}
          onStop={() => void handleStop()}
          onNewChat={handleNewChat}
          onToggleHistory={() =>
            setActivePanel((prev) => (prev === 'history' ? 'none' : 'history'))
          }
          onToggleSettings={() =>
            setActivePanel((prev) => (prev === 'settings' ? 'none' : 'settings'))
          }
          onAttachFiles={(files) => void handleAttachFiles(files)}
          onAttachPath={handleAttachPath}
          isStreaming={isStreaming}
          disabled={!isDaemonReady}
          inputRef={inputRef}
          fileInputRef={fileInputRef}
          standalone={false}
        />
      </div>
    </div>
  )
}
