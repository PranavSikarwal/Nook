import { FolderPlus, Trash2 } from 'lucide-react'
import { useEffect, useState } from 'react'
import { addRoot, getRoots, pickFolder, removeRoot, setRootAccess } from '../lib/daemon'
import type { RootAccess, RootInfo } from '../lib/types'

const ACCESS_LABEL: Record<RootAccess, string> = {
  read: 'Read only',
  read_write: 'Read and write',
}

function messageOf(err: unknown): string {
  return err instanceof Error ? err.message : String(err)
}

export function FoldersTab() {
  const [roots, setRoots] = useState<RootInfo[] | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    let cancelled = false
    getRoots()
      .then((loaded) => {
        if (!cancelled) setRoots(loaded)
      })
      .catch((err: unknown) => {
        if (!cancelled) setError(messageOf(err))
      })
    return () => {
      cancelled = true
    }
  }, [])

  const change = async (action: () => Promise<RootInfo[]>) => {
    setBusy(true)
    try {
      setRoots(await action())
      setError(null)
    } catch (err) {
      setError(messageOf(err))
    } finally {
      setBusy(false)
    }
  }

  const handleAdd = async () => {
    setBusy(true)
    try {
      const path = await pickFolder()
      if (path) setRoots(await addRoot(path, 'read'))
      setError(null)
    } catch (err) {
      setError(messageOf(err))
    } finally {
      setBusy(false)
    }
  }

  return (
    <div data-testid="folders-tab" className="flex-1 p-4 space-y-3 overflow-y-auto text-xs">
      {error && (
        <div
          data-testid="folders-error"
          className="p-2 bg-red-500/10 border border-red-500/20 text-red-400 rounded-md text-[11px]"
        >
          {error}
        </div>
      )}
      <div className="flex items-center justify-between">
        <h3 className="text-zinc-300 font-semibold text-[11px]">Allowed folders</h3>
        <button
          type="button"
          data-testid="folders-add"
          disabled={busy}
          onClick={() => void handleAdd()}
          className="flex items-center gap-1 px-2 py-1 rounded-md text-[11px] text-zinc-300 bg-white/5 hover:bg-white/10 disabled:opacity-60"
        >
          <FolderPlus className="size-3.5" />
          Add folder…
        </button>
      </div>
      <p className="text-[11px] text-zinc-500">
        The file and command tools work only inside these folders. Nook asks before it uses a tool
        in them. A change applies at once.
      </p>
      {roots?.length === 0 && (
        <div data-testid="folders-empty" className="text-[11px] text-zinc-500">
          No folder is allowed. Nook cannot read, write, or run commands on your computer.
        </div>
      )}
      <ul className="space-y-1.5">
        {roots?.map((root) => (
          <li
            key={root.path}
            data-testid="folder-row"
            data-folder-path={root.path}
            data-folder-access={root.access}
            className="p-2 rounded-lg border border-white/10 bg-white/5"
          >
            <div className="flex items-center justify-between gap-2">
              <span className="text-zinc-200 text-[11px] font-medium break-all">{root.path}</span>
              <span className="flex items-center gap-2 shrink-0">
                <span
                  role="group"
                  aria-label={`Access for ${root.path}`}
                  className="flex rounded-md border border-white/10 overflow-hidden"
                >
                  {(Object.keys(ACCESS_LABEL) as RootAccess[]).map((access) => (
                    <button
                      key={access}
                      type="button"
                      data-testid={`folder-access-${access}`}
                      aria-pressed={root.access === access}
                      disabled={busy}
                      onClick={() => {
                        if (root.access !== access) void change(() => setRootAccess(root.path, access))
                      }}
                      className={`px-1.5 py-0.5 text-[10px] disabled:opacity-60 ${
                        root.access === access
                          ? 'bg-purple-600 text-white'
                          : 'text-zinc-400 hover:text-white hover:bg-white/10'
                      }`}
                    >
                      {ACCESS_LABEL[access]}
                    </button>
                  ))}
                </span>
                <button
                  type="button"
                  data-testid="folder-remove"
                  aria-label={`Remove ${root.path}`}
                  disabled={busy}
                  onClick={() => void change(() => removeRoot(root.path))}
                  className="p-1 rounded-md text-zinc-400 hover:text-red-300 hover:bg-white/10 disabled:opacity-60"
                >
                  <Trash2 className="size-3.5" />
                </button>
              </span>
            </div>
            {!root.usable && (
              <div className="mt-1 text-[10px] text-amber-300" data-testid="folder-reason">
                {root.reason ?? 'This folder cannot be used.'} Nook skips it until it works again.
              </div>
            )}
          </li>
        ))}
      </ul>
    </div>
  )
}
