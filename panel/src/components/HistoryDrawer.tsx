import { Trash2, X } from 'lucide-react'
import type { ChatSummary } from '../lib/types'

interface HistoryDrawerProps {
  chats: ChatSummary[]
  activeChatId: string | null
  onSelectChat: (chatId: string) => void
  onDeleteChat: (chatId: string) => void
  onClose: () => void
}

export function HistoryDrawer({
  chats,
  activeChatId,
  onSelectChat,
  onDeleteChat,
  onClose,
}: HistoryDrawerProps) {
  return (
    <div className="flex flex-col h-full bg-[#161618] border-b border-white/10 overflow-hidden text-zinc-200">
      <div className="flex items-center justify-between px-4 py-2.5 border-b border-white/10 bg-white/5">
        <span className="text-xs font-semibold uppercase tracking-wider text-zinc-400">
          Chat History
        </span>
        <button
          onClick={onClose}
          type="button"
          className="text-zinc-400 hover:text-white transition-colors p-1 rounded-md hover:bg-white/10"
        >
          <X className="size-4" />
        </button>
      </div>

      <div className="flex-1 overflow-y-auto divide-y divide-white/5">
        {chats.length === 0 ? (
          <div className="text-center py-8 text-xs text-zinc-500">
            No previous conversations
          </div>
        ) : (
          chats.map((chat) => {
            const isActive = chat.chat_id === activeChatId
            const dateStr = new Date(chat.updated_at).toLocaleDateString(undefined, {
              month: 'short',
              day: 'numeric',
              hour: '2-digit',
              minute: '2-digit',
            })

            return (
              <div
                key={chat.chat_id}
                className={`flex items-center justify-between px-4 py-2.5 hover:bg-white/5 transition-colors cursor-pointer group ${
                  isActive ? 'bg-purple-500/10 border-l-2 border-purple-500' : ''
                }`}
                onClick={() => onSelectChat(chat.chat_id)}
              >
                <div className="flex-1 min-w-0 pr-3">
                  <div className="text-xs font-medium truncate text-zinc-200 group-hover:text-white">
                    {chat.title || 'Untitled Conversation'}
                  </div>
                  <div className="text-[10px] text-zinc-500 mt-0.5">{dateStr}</div>
                </div>

                <button
                  onClick={(e) => {
                    e.stopPropagation()
                    onDeleteChat(chat.chat_id)
                  }}
                  type="button"
                  title="Delete chat"
                  className="opacity-0 group-hover:opacity-100 text-zinc-500 hover:text-red-400 p-1.5 rounded transition-all hover:bg-white/10"
                >
                  <Trash2 className="size-3.5" />
                </button>
              </div>
            )
          })
        )}
      </div>
    </div>
  )
}
