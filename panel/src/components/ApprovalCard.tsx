import { Globe, ShieldAlert } from 'lucide-react'
import type { ApprovalRequest } from '../lib/types'

interface ApprovalCardProps {
  readonly request: ApprovalRequest
  readonly onDecide: (action: string) => void
  readonly disabled?: boolean
}

export function ApprovalCard({ request, onDecide, disabled = false }: ApprovalCardProps) {
  const formatActionLabel = (action: string): string => {
    switch (action) {
      case 'allow_once':
        return 'Allow once'
      case 'allow_for_chat_host':
        return 'Allow for this chat & host'
      case 'allow_for_chat':
        return 'Allow for this chat'
      case 'always_allow':
        return 'Always allow'
      case 'deny':
        return 'Deny'
      default:
        return action.replaceAll('_', ' ')
    }
  }

  const getActionColor = (action: string): string => {
    if (action === 'deny') {
      return 'bg-red-500/20 hover:bg-red-500/30 text-red-300 border-red-500/30'
    }
    if (action.includes('chat') || action === 'always_allow') {
      return 'bg-purple-600/30 hover:bg-purple-600/50 text-purple-200 border-purple-500/40'
    }
    return 'bg-white/10 hover:bg-white/20 text-zinc-200 border-white/20'
  }

  return (
    <div className="mt-3 p-3 rounded-xl bg-purple-950/30 border border-purple-500/30 text-zinc-200 text-xs shadow-lg">
      <div className="flex items-center gap-2 mb-2 text-purple-300 font-semibold text-xs">
        <ShieldAlert className="size-4 text-purple-400 shrink-0" />
        <span>Tool Approval Required</span>
        <span className="font-mono text-[10px] bg-purple-500/20 px-1.5 py-0.5 rounded text-purple-200">
          {request.tool_name}
        </span>
      </div>

      <div className="text-[11px] text-zinc-400 mb-1">
        {request.explanation}
      </div>

      <div className="flex items-center gap-1.5 font-mono text-[10px] text-zinc-300 bg-black/40 px-2 py-1 rounded border border-white/5 mb-3 break-all">
        <Globe className="size-3 text-purple-400 shrink-0" />
        <span>{request.resource_summary}</span>
      </div>

      <div className="flex flex-wrap gap-1.5 justify-end pt-1 border-t border-white/10">
        {request.actions.map((act) => (
          <button
            key={act}
            type="button"
            disabled={disabled}
            onClick={() => onDecide(act)}
            className={`px-2.5 py-1 rounded-md text-xs font-medium border transition-colors disabled:opacity-50 ${getActionColor(
              act
            )}`}
          >
            {formatActionLabel(act)}
          </button>
        ))}
      </div>
    </div>
  )
}
