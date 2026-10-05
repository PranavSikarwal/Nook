import { FileText, Image as ImageIcon, X } from 'lucide-react'
import type { AttachmentInput } from '../lib/types'

interface AttachmentChipsProps {
  readonly attachments: readonly AttachmentInput[]
  readonly onRemove: (index: number) => void
}

function renderAttachmentIcon(att: AttachmentInput) {
  if (att.mime.startsWith('image/')) {
    if (att.data_base64) {
      return (
        <img
          src={`data:${att.mime};base64,${att.data_base64}`}
          alt={att.name}
          className="size-4 object-cover rounded"
        />
      )
    }
    return <ImageIcon className="size-3.5 text-purple-400" />
  }
  return <FileText className="size-3.5 text-zinc-400" />
}

export function AttachmentChips({ attachments, onRemove }: AttachmentChipsProps) {
  if (attachments.length === 0) return null

  return (
    <div className="flex flex-wrap gap-2 px-3 py-2 border-b border-white/5 max-h-24 overflow-y-auto">
      {attachments.map((att, idx) => (
        <div
          key={`${att.name}-${idx}`}
          className="flex items-center gap-1.5 bg-white/10 hover:bg-white/15 border border-white/10 rounded-lg px-2 py-1 text-xs text-zinc-200 transition-colors"
        >
          {renderAttachmentIcon(att)}
          <span className="max-w-[120px] truncate text-[11px] font-medium">{att.name}</span>
          <button
            onClick={() => onRemove(idx)}
            type="button"
            aria-label={`Remove attachment ${att.name}`}
            className="text-zinc-400 hover:text-white transition-colors p-0.5 rounded-full hover:bg-white/10"
          >
            <X className="size-3" />
          </button>
        </div>
      ))}
    </div>
  )
}
