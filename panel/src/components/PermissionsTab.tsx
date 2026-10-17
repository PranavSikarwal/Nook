import { useEffect, useState } from 'react'
import { getPermissions, revokeGrant, setToolPreference } from '../lib/daemon'
import type { Permissions } from '../lib/types'

export function PermissionsTab() {
  const [permissions, setPermissions] = useState<Permissions | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    let cancelled = false
    getPermissions()
      .then((loaded) => {
        if (!cancelled) setPermissions(loaded)
      })
      .catch((err: unknown) => {
        if (!cancelled) setError(err instanceof Error ? err.message : String(err))
      })
    return () => {
      cancelled = true
    }
  }, [])

  const change = async (action: () => Promise<Permissions>) => {
    setBusy(true)
    try {
      setPermissions(await action())
      setError(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  return (
    <div data-testid="permissions-tab" className="flex-1 p-4 space-y-4 overflow-y-auto text-xs">
      {error && (
        <div className="p-2 bg-red-500/10 border border-red-500/20 text-red-400 rounded-md text-[11px]">
          {error}
        </div>
      )}

      <section>
        <h3 className="text-zinc-300 font-semibold text-[11px] mb-1.5">Always allowed</h3>
        {permissions && permissions.grants.length === 0 && (
          <div className="text-[11px] text-zinc-500">No permanent permissions yet.</div>
        )}
        <ul className="space-y-1.5">
          {permissions?.grants.map((grant) => (
            <li
              key={grant.grant_id}
              data-testid="grant-row"
              className="flex items-center justify-between gap-2 p-2 rounded-lg border border-white/10 bg-white/5"
            >
              <span className="text-zinc-200 text-[11px]">{grant.description}</span>
              <button
                type="button"
                disabled={busy}
                aria-label={`Revoke: ${grant.description}`}
                onClick={() => void change(() => revokeGrant(grant.grant_id))}
                className="shrink-0 px-2 py-1 rounded-md text-[11px] border border-red-500/30 bg-red-500/10 text-red-300 hover:bg-red-500/20 transition-colors disabled:opacity-50"
              >
                Revoke
              </button>
            </li>
          ))}
        </ul>
      </section>

      <section>
        <h3 className="text-zinc-300 font-semibold text-[11px] mb-1.5">Tools</h3>
        <ul className="space-y-1.5">
          {permissions?.tools.map((tool) => {
            const selected = tool.choices.find((choice) => choice.tier === tool.tier)
            return (
              <li
                key={tool.tool_id}
                data-testid="tool-row"
                className="p-2 rounded-lg border border-white/10 bg-white/5"
              >
                <div className="flex items-center justify-between gap-2">
                  <span className="text-zinc-200 text-[11px] font-medium">{tool.display_name}</span>
                  <div
                    role="group"
                    aria-label={`Approval for ${tool.display_name}`}
                    className="flex rounded-md border border-white/10 overflow-hidden"
                  >
                    {tool.choices.map((choice) => (
                      <button
                        key={choice.tier}
                        type="button"
                        aria-pressed={choice.tier === tool.tier}
                        disabled={busy || tool.choices.length < 2}
                        onClick={() => void change(() => setToolPreference(tool.tool_id, choice.tier))}
                        className={`px-2 py-1 text-[11px] transition-colors disabled:opacity-60 ${
                          choice.tier === tool.tier
                            ? 'bg-purple-600 text-white'
                            : 'bg-black/30 text-zinc-300 hover:bg-white/10'
                        }`}
                      >
                        {choice.label}
                      </button>
                    ))}
                  </div>
                </div>
                {selected && <div className="mt-1 text-[10px] text-zinc-500">{selected.effect}</div>}
              </li>
            )
          })}
        </ul>
      </section>
    </div>
  )
}
