import { useEffect, useState } from 'react'
import { getNotesSettings, getToolSources, setNotesFolder } from '../lib/daemon'
import { canSaveNotesFolder, isNotesFolderVisible } from '../lib/notesFolder'
import type { NotesSettings, ToolSource } from '../lib/types'

const STATE_STYLE: Record<ToolSource['state'], { label: string; className: string }> = {
  ready: { label: 'Ready', className: 'bg-emerald-500/15 text-emerald-300 border-emerald-500/30' },
  stopped: { label: 'Stopped', className: 'bg-amber-500/15 text-amber-300 border-amber-500/30' },
  off: { label: 'Off', className: 'bg-red-500/15 text-red-300 border-red-500/30' },
}

export function ToolsTab() {
  const [sources, setSources] = useState<ToolSource[] | null>(null)
  const [notesSettings, setNotesSettings] = useState<NotesSettings | null>(null)
  const [folderInput, setFolderInput] = useState('')
  const [savingFolder, setSavingFolder] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)

  useEffect(() => {
    let cancelled = false
    Promise.all([getToolSources(), getNotesSettings()])
      .then(([loadedSources, loadedNotes]) => {
        if (!cancelled) {
          setSources(loadedSources)
          setNotesSettings(loadedNotes)
          setFolderInput(loadedNotes.folder)
        }
      })
      .catch((err: unknown) => {
        if (!cancelled) setError(err instanceof Error ? err.message : String(err))
      })
    return () => {
      cancelled = true
    }
  }, [])

  const refresh = async () => {
    setLoading(true)
    try {
      const [loadedSources, loadedNotes] = await Promise.all([
        getToolSources(),
        getNotesSettings(),
      ])
      setSources(loadedSources)
      setNotesSettings(loadedNotes)
      setFolderInput(loadedNotes.folder)
      setError(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
    }
  }

  const handleSaveNotesFolder = async () => {
    setSavingFolder(true)
    try {
      const updated = await setNotesFolder(folderInput.trim())
      setNotesSettings(updated)
      setFolderInput(updated.folder)
      setError(null)
      const loadedSources = await getToolSources()
      setSources(loadedSources)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setSavingFolder(false)
    }
  }

  const hasNotesSource = sources?.some((source) => source.source_id === 'notes')

  return (
    <div data-testid="tools-tab" className="flex-1 p-4 space-y-3 overflow-y-auto text-xs">
      {error && (
        <div className="p-2 bg-red-500/10 border border-red-500/20 text-red-400 rounded-md text-[11px]">
          {error}
        </div>
      )}
      <div className="flex items-center justify-between">
        <h3 className="text-zinc-300 font-semibold text-[11px]">Tool sources</h3>
        <button
          type="button"
          disabled={loading}
          onClick={() => void refresh()}
          className="px-2 py-1 rounded-md text-[11px] text-zinc-300 bg-white/5 hover:bg-white/10 disabled:opacity-60"
        >
          Refresh
        </button>
      </div>
      {sources?.length === 0 && (
        <div className="text-[11px] text-zinc-500">
          No file, command, or MCP tools are set up. Add them to config.toml and restart Nook.
        </div>
      )}
      <ul className="space-y-1.5">
        {sources?.map((source, index) => {
          const style = STATE_STYLE[source.state]
          const isNotes = source.source_id === 'notes'
          const showFolderField = isNotes && isNotesFolderVisible(notesSettings)
          return (
            <li
              key={`${source.source_id}-${index}`}
              data-testid="tool-source-row"
              data-source-id={source.source_id}
              data-source-state={source.state}
              className="p-2 rounded-lg border border-white/10 bg-white/5"
            >
              <div className="flex items-center justify-between gap-2">
                <span className="text-zinc-200 text-[11px] font-medium">{source.display_name}</span>
                <span className="flex items-center gap-2">
                  <span className="text-zinc-500 text-[10px]">
                    {source.tools === 1 ? '1 tool' : `${source.tools} tools`}
                  </span>
                  <span className={`px-1.5 py-0.5 rounded border text-[10px] ${style.className}`}>
                    {style.label}
                  </span>
                </span>
              </div>
              {source.reason && (
                <div className="mt-1 text-[10px] text-zinc-400" data-testid="tool-source-reason">
                  {source.reason}
                </div>
              )}
              {showFolderField && (
                <form
                  onSubmit={(e) => {
                    e.preventDefault()
                    if (canSaveNotesFolder(folderInput, notesSettings?.folder ?? '')) {
                      void handleSaveNotesFolder()
                    }
                  }}
                  className="mt-2 pt-2 border-t border-white/10 space-y-1.5"
                  data-testid="notes-folder-field"
                >
                  <div className="flex items-center justify-between gap-2">
                    <label htmlFor="notes-folder-input" className="text-zinc-300 font-medium text-[11px]">
                      Notes folder
                    </label>
                    <button
                      type="submit"
                      data-testid="notes-folder-save"
                      disabled={savingFolder || !canSaveNotesFolder(folderInput, notesSettings?.folder ?? '')}
                      className="px-2 py-0.5 rounded text-[10px] font-medium bg-purple-600 hover:bg-purple-500 text-white disabled:opacity-40 transition-colors"
                    >
                      {savingFolder ? 'Saving…' : 'Save'}
                    </button>
                  </div>
                  <input
                    id="notes-folder-input"
                    data-testid="notes-folder-input"
                    type="text"
                    value={folderInput}
                    onChange={(e) => setFolderInput(e.target.value)}
                    placeholder="Folder name"
                    disabled={savingFolder}
                    className="w-full bg-white/5 border border-white/10 rounded-md px-2 py-1 text-zinc-200 focus:outline-none focus:border-purple-500/50 transition-colors font-mono text-[11px]"
                  />
                  {notesSettings?.reason && (
                    <div className="text-[10px] text-zinc-400" data-testid="notes-folder-reason">
                      {notesSettings.reason}
                    </div>
                  )}
                  <p className="text-[10px] text-zinc-500">
                    macOS asks for permission on the first call and does not ask again after a refusal.
                  </p>
                </form>
              )}
            </li>
          )
        })}
      </ul>
      {sources && !hasNotesSource && isNotesFolderVisible(notesSettings) && (
        <div className="p-2 rounded-lg border border-white/10 bg-white/5" data-testid="notes-folder-standalone">
          <form
            onSubmit={(e) => {
              e.preventDefault()
              if (canSaveNotesFolder(folderInput, notesSettings?.folder ?? '')) {
                void handleSaveNotesFolder()
              }
            }}
            className="space-y-1.5"
            data-testid="notes-folder-field"
          >
            <div className="flex items-center justify-between gap-2">
              <label htmlFor="notes-folder-input-standalone" className="text-zinc-300 font-medium text-[11px]">
                Notes folder
              </label>
              <button
                type="submit"
                data-testid="notes-folder-save"
                disabled={savingFolder || !canSaveNotesFolder(folderInput, notesSettings?.folder ?? '')}
                className="px-2 py-0.5 rounded text-[10px] font-medium bg-purple-600 hover:bg-purple-500 text-white disabled:opacity-40 transition-colors"
              >
                {savingFolder ? 'Saving…' : 'Save'}
              </button>
            </div>
            <input
              id="notes-folder-input-standalone"
              data-testid="notes-folder-input"
              type="text"
              value={folderInput}
              onChange={(e) => setFolderInput(e.target.value)}
              placeholder="Folder name"
              disabled={savingFolder}
              className="w-full bg-white/5 border border-white/10 rounded-md px-2 py-1 text-zinc-200 focus:outline-none focus:border-purple-500/50 transition-colors font-mono text-[11px]"
            />
            {notesSettings?.reason && (
              <div className="text-[10px] text-zinc-400" data-testid="notes-folder-reason">
                {notesSettings.reason}
              </div>
            )}
            <p className="text-[10px] text-zinc-500">
              macOS asks for permission on the first call and does not ask again after a refusal.
            </p>
          </form>
        </div>
      )}
    </div>
  )
}
