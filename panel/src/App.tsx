import { AlertCircle, X } from 'lucide-react'
import { useEffect, useState } from 'react'
import { AttachmentChips } from './components/AttachmentChips'
import { HistoryDrawer } from './components/HistoryDrawer'
import { InputBar } from './components/InputBar'
import { SettingsModal } from './components/SettingsModal'
import { TranscriptView } from './components/TranscriptView'
import {
  cancelMessage,
  deleteChat,
  getChat,
  getSettings,
  listChats,
  sendMessage,
  setSettings,
  setWindowSize,
  startDrag,
} from './lib/daemon'
import type {
  AttachmentInput,
  ChatMessage,
  ChatSummary,
  DaemonEvent,
  SettingsInfo,
  SettingsPayload,
} from './lib/types'

type ActivePanel = 'none' | 'history' | 'settings'

const WINDOW_WIDTH = 640
const COMPACT_HEIGHT = 56
const ATTACHMENTS_HEIGHT = 110
const DRAWER_HEIGHT = 440
const EXPANDED_HEIGHT = 520

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

  const isCompact = activePanel === 'none' && messages.length === 0 && attachments.length === 0

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

  // Load initial settings and history
  useEffect(() => {
    void getSettings().then(setSettingsInfo).catch(() => {})
    void listChats().then(setChats).catch(() => {})
  }, [])

  // Process incoming daemon events
  const handleDaemonEvent = (event: DaemonEvent) => {
    if (event.type === 'text_delta') {
      setMessages((prev) => {
        const last = prev[prev.length - 1]
        if (last?.role === 'assistant') {
          return [
            ...prev.slice(0, -1),
            { ...last, text: last.text + event.text },
          ]
        }
        return [
          ...prev,
          {
            id: event.message_id,
            role: 'assistant',
            text: event.text,
            status: 'streaming',
            attachments: [],
            created_at: new Date().toISOString(),
          },
        ]
      })
    } else if (event.type === 'message_finished') {
      setIsStreaming(false)
      setMessages((prev) => {
        const last = prev[prev.length - 1]
        if (last?.role === 'assistant') {
          return [
            ...prev.slice(0, -1),
            { ...last, status: event.status === 'error' ? 'error' : 'complete' },
          ]
        }
        return prev
      })
      void listChats().then(setChats).catch(() => {})
    } else if (event.type === 'chat_titled') {
      void listChats().then(setChats).catch(() => {})
    } else if (event.type === 'error') {
      setIsStreaming(false)
      setMessages((prev) => {
        const last = prev[prev.length - 1]
        if (last?.role === 'assistant') {
          return [
            ...prev.slice(0, -1),
            { ...last, status: 'error', error: event.error },
          ]
        }
        return [
          ...prev,
          {
            id: crypto.randomUUID(),
            role: 'assistant',
            text: 'An error occurred while communicating with the model endpoint.',
            status: 'error',
            error: event.error,
            attachments: [],
            created_at: new Date().toISOString(),
          },
        ]
      })
    }
  }

  // Escape key handler
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        if (activePanel !== 'none') {
          setActivePanel('none')
        }
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [activePanel])

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

    const assistantMsg: ChatMessage = {
      id: crypto.randomUUID(),
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
    setActivePanel('none')

    try {
      await sendMessage(chatId, question, currentAtts, handleDaemonEvent)
    } catch (err) {
      setIsStreaming(false)
      setMessages((prev) => [
        ...prev.slice(0, -1),
        {
          ...assistantMsg,
          status: 'error',
          error: {
            code: 'internal',
            message: err instanceof Error ? err.message : String(err),
            retryable: true,
          },
        },
      ])
    }
  }

  const handleRetry = () => {
    const lastUser = [...messages].reverse().find((m) => m.role === 'user')
    if (lastUser) {
      const attsInput: AttachmentInput[] = lastUser.attachments.map((a) => ({
        name: a.name,
        mime: a.mime,
        file_path: a.path,
      }))
      void handleSend(lastUser.text, attsInput)
    }
  }

  const handleStop = async () => {
    setIsStreaming(false)
    try {
      await cancelMessage()
    } catch {
      // Ignore cancel error
    }
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
    try {
      const transcript = await getChat(selectedId)
      setChatId(transcript.chat_id)
      setMessages(transcript.messages)
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

  const handleNewChat = () => {
    setChatId(crypto.randomUUID())
    setMessages([])
    setAttachments([])
    setInput('')
    setActivePanel('none')
    setIsStreaming(false)
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
          standalone={true}
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
            />
          </div>
        )}

        {activePanel === 'none' && messages.length > 0 && (
          <TranscriptView
            messages={messages}
            isStreaming={isStreaming}
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
          standalone={false}
        />
      </div>
    </div>
  )
}
