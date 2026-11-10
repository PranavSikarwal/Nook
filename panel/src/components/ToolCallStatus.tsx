import { Check, Loader2 } from 'lucide-react'
import { describeToolCall } from '../lib/toolCalls'
import type { ToolCall } from '../lib/types'

interface ToolCallStatusProps {
  readonly calls: readonly ToolCall[]
  readonly replyIsActive: boolean
}

export function ToolCallStatus({ calls, replyIsActive }: ToolCallStatusProps) {
  if (calls.length === 0) return null

  return (
    <ul className="mt-2 space-y-1 select-none">
      {calls.map((call) => {
        const isRunning = call.status === 'running' && replyIsActive
        return (
          <li
            key={call.call_id}
            data-testid="tool-call"
            data-tool-status={call.status}
            className="flex items-center gap-1.5 text-[11px] text-zinc-400"
          >
            {isRunning ? (
              <Loader2 className="size-3 animate-spin text-purple-400" />
            ) : (
              <Check className="size-3 text-zinc-500" />
            )}
            <span>{describeToolCall(call, replyIsActive)}</span>
          </li>
        )
      })}
    </ul>
  )
}
