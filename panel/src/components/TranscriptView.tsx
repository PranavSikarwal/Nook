import { AlertCircle, RotateCcw } from 'lucide-react'
import { useEffect, useRef } from 'react'
import type { ChatMessage } from '../lib/types'
import { MarkdownRenderer } from './MarkdownRenderer'

interface TranscriptViewProps {
  messages: ChatMessage[]
  isStreaming: boolean
  onRetry?: () => void
}

export function TranscriptView({ messages, isStreaming, onRetry }: TranscriptViewProps) {
  const bottomRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages, isStreaming])

  if (messages.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center text-xs text-zinc-500">
        No messages yet. Ask anything to begin.
      </div>
    )
  }

  return (
    <div className="flex-1 overflow-y-auto px-4 py-3 space-y-4">
      {messages.map((msg) => {
        const isUser = msg.role === 'user'

        if (isUser) {
          return (
            <div key={msg.id} className="flex justify-end">
              <div className="max-w-[85%] bg-[#8a38f5] text-white px-4 py-2 rounded-2xl rounded-tr-sm text-sm shadow-sm leading-relaxed whitespace-pre-wrap break-words">
                {msg.text}
                {msg.attachments && msg.attachments.length > 0 && (
                  <div className="mt-2 pt-2 border-t border-white/20 flex flex-wrap gap-1">
                    {msg.attachments.map((att) => (
                      <span
                        key={att.id}
                        className="text-[11px] bg-black/20 px-2 py-0.5 rounded text-white/90"
                      >
                        {att.name}
                      </span>
                    ))}
                  </div>
                )}
              </div>
            </div>
          )
        }

        return (
          <div key={msg.id} className="flex justify-start">
            <div className="max-w-[95%] w-full bg-white/[0.04] border border-white/[0.08] px-4 py-3 rounded-2xl rounded-tl-sm text-sm">
              <MarkdownRenderer content={msg.text} />

              {msg.status === 'streaming' && (
                <div className="flex items-center gap-1.5 mt-2 text-xs text-purple-400 font-medium">
                  <span className="size-1.5 rounded-full bg-purple-400 animate-pulse" />
                  <span>Thinking...</span>
                </div>
              )}

              {msg.status === 'error' && msg.error && (
                <div className="mt-3 p-2.5 rounded-lg bg-red-500/10 border border-red-500/20 text-red-300 text-xs flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <AlertCircle className="size-4 text-red-400 shrink-0" />
                    <span>{msg.error.message}</span>
                  </div>
                  {onRetry && (
                    <button
                      onClick={onRetry}
                      type="button"
                      className="flex items-center gap-1 text-[11px] bg-red-500/20 hover:bg-red-500/30 px-2 py-1 rounded transition-colors text-white"
                    >
                      <RotateCcw className="size-3" />
                      <span>Retry</span>
                    </button>
                  )}
                </div>
              )}
            </div>
          </div>
        )
      })}
      <div ref={bottomRef} />
    </div>
  )
}
