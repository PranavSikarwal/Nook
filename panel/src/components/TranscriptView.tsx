import { AlertCircle, Check, Copy, RotateCcw } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import type { ApprovalRequest, ChatMessage } from '../lib/types'
import { ApprovalCard } from './ApprovalCard'
import { MarkdownRenderer } from './MarkdownRenderer'

interface TranscriptViewProps {
  readonly messages: readonly ChatMessage[]
  readonly isStreaming: boolean
  readonly activeApproval?: ApprovalRequest | null
  readonly onApprovalDecide?: (action: string) => void
  readonly onRetry?: () => void
}

interface CopyMessageButtonProps {
  readonly text: string
}

function CopyMessageButton({ text }: CopyMessageButtonProps) {
  const [copied, setCopied] = useState(false)

  const handleCopy = () => {
    void navigator.clipboard
      .writeText(text)
      .then(() => {
        setCopied(true)
        setTimeout(() => setCopied(false), 2000)
      })
      .catch(() => {})
  }

  return (
    <button
      onClick={handleCopy}
      type="button"
      title="Copy response"
      aria-label="Copy full response"
      className="flex items-center gap-1.5 px-2 py-1 rounded text-xs text-zinc-400 hover:text-white hover:bg-white/10 transition-colors select-none"
    >
      {copied ? (
        <>
          <Check className="size-3.5 text-emerald-400" />
          <span className="text-emerald-400 text-[11px] font-medium">Copied</span>
        </>
      ) : (
        <>
          <Copy className="size-3.5" />
          <span className="text-[11px]">Copy</span>
        </>
      )}
    </button>
  )
}

function renderAssistantContent(msg: ChatMessage) {
  if (msg.text.trim().length > 0) {
    return <MarkdownRenderer content={msg.text} />
  }
  if (msg.status === 'streaming') {
    return null
  }
  return (
    <p className="text-zinc-500 italic text-xs">
      (Model produced no text response)
    </p>
  )
}

export function TranscriptView({
  messages,
  isStreaming,
  activeApproval,
  onApprovalDecide,
  onRetry,
}: TranscriptViewProps) {
  const bottomRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages, isStreaming, activeApproval])

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
              <div className="max-w-[85%] bg-[#8a38f5] text-white px-4 py-2 rounded-2xl rounded-tr-sm text-sm shadow-sm leading-relaxed whitespace-pre-wrap break-words select-text">
                {msg.text}
                {msg.attachments.length > 0 && (
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
            <div className="max-w-[95%] w-full bg-white/[0.04] border border-white/[0.08] px-4 py-3 rounded-2xl rounded-tl-sm text-sm select-text">
              {renderAssistantContent(msg)}

              {msg.status === 'streaming' && (
                <div className="flex items-center gap-1.5 mt-2 text-xs text-purple-400 font-medium select-none">
                  <span className="size-1.5 rounded-full bg-purple-400 animate-pulse" />
                  <span>Thinking...</span>
                </div>
              )}

              {activeApproval?.message_id === msg.id && onApprovalDecide && (
                <ApprovalCard
                  request={activeApproval}
                  onDecide={onApprovalDecide}
                />
              )}

              {/* Action bar for completed assistant response */}
              {msg.text.trim().length > 0 && msg.status !== 'streaming' && (
                <div className="flex items-center justify-end mt-2 pt-2 border-t border-white/5">
                  <CopyMessageButton text={msg.text} />
                </div>
              )}

              {msg.status === 'error' && msg.error && (
                <div className="mt-3 p-2.5 rounded-lg bg-red-500/10 border border-red-500/20 text-red-300 text-xs flex items-center justify-between select-none">
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
