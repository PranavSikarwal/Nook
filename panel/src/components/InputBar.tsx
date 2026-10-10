import { ArrowUp, History, Paperclip, Plus, Settings, Square } from 'lucide-react'
import { useRef } from 'react'

interface InputBarProps {
  readonly input: string
  readonly setInput: (value: string) => void
  readonly onSend: () => void
  readonly onStop?: () => void
  readonly onNewChat?: () => void
  readonly onToggleHistory: () => void
  readonly onToggleSettings: () => void
  readonly onAttachFiles: (files: FileList) => void
  readonly onAttachPath?: (path: string) => void
  readonly isStreaming: boolean
  readonly disabled?: boolean
  readonly standalone?: boolean
  readonly inputRef?: React.RefObject<HTMLInputElement | null>
  readonly fileInputRef?: React.RefObject<HTMLInputElement | null>
}

function getMimeFromExtension(filename: string): string | null {
  const ext = filename.split('.').pop()?.toLowerCase()
  switch (ext) {
    case 'png':
      return 'image/png'
    case 'jpg':
    case 'jpeg':
      return 'image/jpeg'
    case 'webp':
      return 'image/webp'
    case 'gif':
      return 'image/gif'
    case 'pdf':
      return 'application/pdf'
    case 'txt':
      return 'text/plain'
    case 'md':
      return 'text/markdown'
    case 'json':
      return 'application/json'
    default:
      return null
  }
}

export function InputBar({
  input,
  setInput,
  onSend,
  onStop,
  onNewChat,
  onToggleHistory,
  onToggleSettings,
  onAttachFiles,
  onAttachPath,
  isStreaming,
  disabled = false,
  standalone = false,
  inputRef,
  fileInputRef,
}: InputBarProps) {
  const localFileInputRef = useRef<HTMLInputElement>(null)
  const actualFileInputRef = fileInputRef ?? localFileInputRef

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      if (isStreaming && onStop) {
        onStop()
      } else if (input.trim()) {
        onSend()
      }
    }
  }

  const handlePaste = (e: React.ClipboardEvent<HTMLInputElement>) => {
    const clipboardData = e.clipboardData
    if (!clipboardData) return

    // 1. Direct file/image data in clipboard (e.g. screenshots or copied images)
    const items = Array.from(clipboardData.items)
    const fileItems = items.filter((item) => item.kind === 'file')

    if (fileItems.length > 0) {
      e.preventDefault()
      const files: File[] = []
      for (const item of fileItems) {
        const file = item.getAsFile()
        if (file) files.push(file)
      }
      if (files.length > 0) {
        const dataTransfer = new DataTransfer()
        for (const f of files) {
          dataTransfer.items.add(f)
        }
        onAttachFiles(dataTransfer.files)
        return
      }
    }

    // 2. File path in clipboard (e.g. copied file from Finder)
    const text = clipboardData.getData('text/plain').trim()
    const cleanPath = text.replace(/^file:\/\//, '')
    const mime = getMimeFromExtension(cleanPath)

    if (mime && (cleanPath.startsWith('/') || cleanPath.startsWith('~'))) {
      e.preventDefault()
      onAttachPath?.(cleanPath)
    }
  }

  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files && e.target.files.length > 0) {
      onAttachFiles(e.target.files)
      e.target.value = ''
    }
  }

  const hasContent = input.trim().length > 0

  const containerClasses = standalone
    ? 'flex items-center gap-2.5 px-3.5 py-2.5 rounded-2xl bg-[#161618]/95 backdrop-blur-xl border border-white/10 shadow-2xl select-none w-full cursor-move'
    : 'flex items-center gap-2.5 px-3.5 py-2.5 bg-transparent border-t border-white/5 select-none w-full'

  return (
    <div
      data-tauri-drag-region
      className={containerClasses}
    >
      <input
        ref={actualFileInputRef}
        type="file"
        multiple
        accept="image/*,.pdf,.txt,.md,.json,.csv,.py,.rs,.swift,.ts,.tsx,.js"
        onChange={handleFileChange}
        className="hidden"
      />

      {/* Attachment paperclip button */}
      <button
        onClick={() => actualFileInputRef.current?.click()}
        type="button"
        title="Attach file or image"
        aria-label="Attach file or image"
        className="text-zinc-400 hover:text-white transition-colors p-1.5 rounded-lg hover:bg-white/5 shrink-0"
      >
        <Paperclip className="size-4" />
      </button>

      {/* Text input with purple caret */}
      <input
        ref={inputRef}
        type="text"
        value={input}
        onChange={(e) => setInput(e.target.value)}
        onKeyDown={handleKeyDown}
        onPaste={handlePaste}
        placeholder="Ask anything..."
        disabled={disabled}
        aria-label="Ask anything"
        className="flex-1 bg-transparent text-sm text-zinc-100 placeholder:text-zinc-500 focus:outline-none caret-[#a855f7] tracking-normal font-normal cursor-text"
      />

      {/* Right controls */}
      <div className="flex items-center gap-1 shrink-0">
        {onNewChat && (
          <button
            onClick={onNewChat}
            type="button"
            title="New chat"
            aria-label="Start new chat"
            disabled={isStreaming}
            className="text-zinc-400 hover:text-white transition-colors p-1.5 rounded-lg hover:bg-white/5 disabled:opacity-40 disabled:cursor-not-allowed"
          >
            <Plus className="size-4" />
          </button>
        )}

        <button
          onClick={onToggleHistory}
          type="button"
          title="History"
          aria-label="Toggle history drawer"
          className="text-zinc-400 hover:text-white transition-colors p-1.5 rounded-lg hover:bg-white/5"
        >
          <History className="size-4" />
        </button>

        <button
          onClick={onToggleSettings}
          type="button"
          title="Settings"
          aria-label="Toggle settings modal"
          className="text-zinc-400 hover:text-white transition-colors p-1.5 rounded-lg hover:bg-white/5"
        >
          <Settings className="size-4" />
        </button>

        {/* Send / Stop button */}
        {isStreaming ? (
          <button
            onClick={onStop}
            type="button"
            title="Stop generation"
            aria-label="Stop generation"
            className="flex items-center justify-center size-7 rounded-full bg-red-500/20 hover:bg-red-500/30 text-red-400 transition-colors ml-1"
          >
            <Square className="size-3 fill-current" />
          </button>
        ) : (
          <button
            onClick={() => {
              if (hasContent) onSend()
            }}
            type="button"
            disabled={!hasContent || disabled}
            title="Send question"
            aria-label="Send question"
            className={`flex items-center justify-center size-7 rounded-full transition-all ml-1 ${
              hasContent
                ? 'bg-purple-600 hover:bg-purple-500 text-white shadow-sm shadow-purple-600/30 scale-100'
                : 'bg-white/10 text-zinc-500 cursor-not-allowed opacity-60'
            }`}
          >
            <ArrowUp className="size-3.5 stroke-[2.5]" />
          </button>
        )}
      </div>
    </div>
  )
}
