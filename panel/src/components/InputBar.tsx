import { getCurrentWindow } from '@tauri-apps/api/window'
import { ArrowUp, History, Paperclip, Plus, Settings, Square } from 'lucide-react'
import { useRef } from 'react'

interface InputBarProps {
  input: string
  setInput: (value: string) => void
  onSend: () => void
  onStop?: () => void
  onNewChat?: () => void
  onToggleHistory: () => void
  onToggleSettings: () => void
  onAttachFiles: (files: FileList) => void
  isStreaming: boolean
  disabled?: boolean
  standalone?: boolean
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
  isStreaming,
  disabled = false,
  standalone = false,
}: InputBarProps) {
  const fileInputRef = useRef<HTMLInputElement>(null)

  const handleMouseDown = (e: React.MouseEvent) => {
    if (e.button === 0 && !(e.target as HTMLElement).closest('button, input, textarea, a, select')) {
      const appWindow = getCurrentWindow()
      appWindow.startDragging()
    }
  }

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

  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files && e.target.files.length > 0) {
      onAttachFiles(e.target.files)
      e.target.value = ''
    }
  }

  const hasContent = input.trim().length > 0

  const containerClasses = standalone
    ? 'flex items-center gap-2.5 px-3.5 py-2.5 rounded-2xl bg-[#161618]/95 backdrop-blur-xl border border-white/10 shadow-2xl select-none w-full'
    : 'flex items-center gap-2.5 px-3.5 py-2.5 bg-transparent border-t border-white/5 select-none w-full'

  return (
    <div
      onMouseDown={handleMouseDown}
      className={containerClasses}
    >
      <input
        ref={fileInputRef}
        type="file"
        multiple
        accept="image/*,.pdf,.txt,.md,.json,.csv,.py,.rs,.swift,.ts,.tsx,.js"
        onChange={handleFileChange}
        className="hidden"
      />

      {/* Attachment paperclip button */}
      <button
        onClick={() => fileInputRef.current?.click()}
        type="button"
        title="Attach file or image"
        className="text-zinc-400 hover:text-white transition-colors p-1.5 rounded-lg hover:bg-white/5 shrink-0"
      >
        <Paperclip className="size-4" />
      </button>

      {/* Text input with purple caret */}
      <input
        type="text"
        value={input}
        onChange={(e) => setInput(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder="Ask anything..."
        disabled={disabled}
        autoFocus
        className="flex-1 bg-transparent text-sm text-zinc-100 placeholder:text-zinc-500 focus:outline-none caret-[#a855f7] tracking-normal font-normal"
      />

      {/* Right controls */}
      <div className="flex items-center gap-1 shrink-0">
        {onNewChat && (
          <button
            onClick={onNewChat}
            type="button"
            title="New chat"
            className="text-zinc-400 hover:text-white transition-colors p-1.5 rounded-lg hover:bg-white/5"
          >
            <Plus className="size-4" />
          </button>
        )}

        <button
          onClick={onToggleHistory}
          type="button"
          title="History"
          className="text-zinc-400 hover:text-white transition-colors p-1.5 rounded-lg hover:bg-white/5"
        >
          <History className="size-4" />
        </button>

        <button
          onClick={onToggleSettings}
          type="button"
          title="Settings"
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
