import { Eye, EyeOff, Save, X } from 'lucide-react'
import { useEffect, useState } from 'react'
import type { SettingsInfo, SettingsPayload } from '../lib/types'

interface SettingsModalProps {
  readonly currentSettings: SettingsInfo | null
  readonly onSave: (payload: SettingsPayload) => Promise<void>
  readonly onClose: () => void
}

export function SettingsModal({
  currentSettings,
  onSave,
  onClose,
}: SettingsModalProps) {
  const [baseUrl, setBaseUrl] = useState('')
  const [model, setModel] = useState('')
  const [apiKey, setApiKey] = useState('')
  const [showApiKey, setShowApiKey] = useState(false)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (currentSettings) {
      setBaseUrl(currentSettings.base_url)
      setModel(currentSettings.model)
    }
  }, [currentSettings])

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setError(null)
    setSaving(true)
    try {
      await onSave({
        base_url: baseUrl.trim(),
        model: model.trim(),
        api_key: apiKey.trim() ? apiKey.trim() : undefined,
      })
      onClose()
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="flex flex-col h-full bg-[#161618] border-b border-white/10 overflow-hidden text-zinc-200">
      <div className="flex items-center justify-between px-4 py-2.5 border-b border-white/10 bg-white/5">
        <span className="text-xs font-semibold uppercase tracking-wider text-zinc-400">
          Model Settings
        </span>
        <button
          onClick={onClose}
          type="button"
          aria-label="Close settings"
          className="text-zinc-400 hover:text-white transition-colors p-1 rounded-md hover:bg-white/10"
        >
          <X className="size-4" />
        </button>
      </div>

      <form onSubmit={handleSubmit} className="flex-1 p-4 space-y-3.5 overflow-y-auto text-xs">
        {error && (
          <div className="p-2 bg-red-500/10 border border-red-500/20 text-red-400 rounded-md text-[11px]">
            {error}
          </div>
        )}

        <div>
          <label htmlFor="settings-base-url" className="block text-zinc-400 font-medium mb-1 text-[11px]">
            Base URL
          </label>
          <input
            id="settings-base-url"
            type="text"
            value={baseUrl}
            onChange={(e) => setBaseUrl(e.target.value)}
            placeholder="https://models.example.internal/v1"
            className="w-full bg-white/5 border border-white/10 rounded-lg px-2.5 py-1.5 text-zinc-200 focus:outline-none focus:border-purple-500/50 transition-colors font-mono text-[11px]"
            required
          />
        </div>

        <div>
          <label htmlFor="settings-model" className="block text-zinc-400 font-medium mb-1 text-[11px]">
            Model Name
          </label>
          <input
            id="settings-model"
            type="text"
            value={model}
            onChange={(e) => setModel(e.target.value)}
            placeholder="e.g. meta-llama/Llama-3-70b-chat"
            className="w-full bg-white/5 border border-white/10 rounded-lg px-2.5 py-1.5 text-zinc-200 focus:outline-none focus:border-purple-500/50 transition-colors font-mono text-[11px]"
            required
          />
        </div>

        <div>
          <div className="flex items-center justify-between mb-1">
            <label htmlFor="settings-api-key" className="text-zinc-400 font-medium text-[11px]">
              API Key
            </label>
            {currentSettings?.has_api_key && (
              <span className="text-[10px] text-emerald-400">Configured in Keychain</span>
            )}
          </div>
          <div className="relative">
            <input
              id="settings-api-key"
              type={showApiKey ? 'text' : 'password'}
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder={currentSettings?.has_api_key ? 'Leave empty to keep existing' : 'Enter API Key'}
              className="w-full bg-white/5 border border-white/10 rounded-lg pl-2.5 pr-8 py-1.5 text-zinc-200 focus:outline-none focus:border-purple-500/50 transition-colors font-mono text-[11px]"
            />
            <button
              type="button"
              onClick={() => setShowApiKey(!showApiKey)}
              aria-label={showApiKey ? 'Hide API key' : 'Show API key'}
              className="absolute right-2 top-1/2 -translate-y-1/2 text-zinc-400 hover:text-white"
            >
              {showApiKey ? <EyeOff className="size-3.5" /> : <Eye className="size-3.5" />}
            </button>
          </div>
        </div>

        <div className="pt-2 flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="px-3 py-1.5 rounded-lg border border-white/10 text-zinc-400 hover:text-white hover:bg-white/5 transition-colors text-xs"
          >
            Cancel
          </button>
          <button
            type="submit"
            disabled={saving}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-purple-600 hover:bg-purple-500 text-white font-medium transition-colors text-xs disabled:opacity-50"
          >
            <Save className="size-3" />
            <span>{saving ? 'Saving...' : 'Save Settings'}</span>
          </button>
        </div>
      </form>
    </div>
  )
}
