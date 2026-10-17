import { useEffect, useState } from 'react'
import { getToolSources } from '../lib/daemon'
import type { ToolSource } from '../lib/types'

const STATE_STYLE: Record<ToolSource['state'], { label: string; className: string }> = {
  ready: { label: 'Ready', className: 'bg-emerald-500/15 text-emerald-300 border-emerald-500/30' },
  stopped: { label: 'Stopped', className: 'bg-amber-500/15 text-amber-300 border-amber-500/30' },
  off: { label: 'Off', className: 'bg-red-500/15 text-red-300 border-red-500/30' },
}

export function ToolsTab() {
  const [sources, setSources] = useState<ToolSource[] | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)

  useEffect(() => {
    let cancelled = false
    getToolSources()
      .then((loaded) => {
        if (!cancelled) setSources(loaded)
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
      setSources(await getToolSources())
      setError(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
    }
  }

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
            </li>
          )
        })}
      </ul>
    </div>
  )
}
