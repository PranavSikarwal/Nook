import { Eye, EyeOff, Keyboard, RotateCcw, Save, Settings as SettingsIcon, X } from 'lucide-react'
import { useEffect, useState } from 'react'
import {
  DEFAULT_SHORTCUTS,
  findShortcutConflict,
  getDefaultShortcutMap,
  loadShortcuts,
  normalizeKeyboardEvent,
  saveShortcuts,
  type ShortcutAction,
  type ShortcutMap,
} from '../lib/shortcuts'
import type { SettingsInfo, SettingsPayload } from '../lib/types'

interface SettingsModalProps {
  readonly currentSettings: SettingsInfo | null
  readonly onSave: (payload: SettingsPayload) => Promise<void>
  readonly onClose: () => void
  readonly onShortcutsChanged?: (shortcuts: ShortcutMap) => void
}

interface ModelSettingsTabProps {
  readonly currentSettings: SettingsInfo | null
  readonly onSave: (payload: SettingsPayload) => Promise<void>
  readonly onClose: () => void
}

interface SettingInputFieldProps {
  readonly id: string
  readonly label: string
  readonly value: string
  readonly onChange: (value: string) => void
  readonly placeholder?: string
  readonly required?: boolean
}

function SettingInputField({
  id,
  label,
  value,
  onChange,
  placeholder,
  required,
}: SettingInputFieldProps) {
  return (
    <div>
      <label htmlFor={id} className="block text-zinc-400 font-medium mb-1 text-[11px]">
        {label}
      </label>
      <input
        id={id}
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        className="w-full bg-white/5 border border-white/10 rounded-lg px-2.5 py-1.5 text-zinc-200 focus:outline-none focus:border-purple-500/50 transition-colors font-mono text-[11px]"
        required={required}
      />
    </div>
  )
}

function ModelSettingsTab({ currentSettings, onSave, onClose }: ModelSettingsTabProps) {
  const [baseUrl, setBaseUrl] = useState(currentSettings?.base_url ?? '')
  const [model, setModel] = useState(currentSettings?.model ?? '')
  const [apiKey, setApiKey] = useState('')
  const [showApiKey, setShowApiKey] = useState(false)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)

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
    <form onSubmit={handleSubmit} className="flex-1 p-4 space-y-3.5 overflow-y-auto text-xs">
      {error && (
        <div className="p-2 bg-red-500/10 border border-red-500/20 text-red-400 rounded-md text-[11px]">
          {error}
        </div>
      )}

      <SettingInputField
        id="settings-base-url"
        label="Base URL"
        value={baseUrl}
        onChange={setBaseUrl}
        placeholder="https://models.example.internal/v1"
        required
      />

      <SettingInputField
        id="settings-model"
        label="Model Name"
        value={model}
        onChange={setModel}
        placeholder="e.g. meta-llama/Llama-3-70b-chat"
        required
      />

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
  )
}

interface ShortcutsTabProps {
  readonly onShortcutsChanged?: (shortcuts: ShortcutMap) => void
}

function ShortcutsTab({ onShortcutsChanged }: ShortcutsTabProps) {
  const [shortcuts, setShortcuts] = useState<ShortcutMap>(loadShortcuts)
  const [recordingAction, setRecordingAction] = useState<ShortcutAction | null>(null)
  const [shortcutError, setShortcutError] = useState<string | null>(null)

  useEffect(() => {
    if (!recordingAction) return

    const handleKeyDown = (e: KeyboardEvent) => {
      e.preventDefault()
      e.stopPropagation()

      if (e.key === 'Escape') {
        setRecordingAction(null)
        setShortcutError(null)
        return
      }

      const combo = normalizeKeyboardEvent(e)
      if (!combo) return

      const conflict = findShortcutConflict(recordingAction, combo, shortcuts)
      if (conflict) {
        setShortcutError(`Conflict: ${combo} is already assigned to ${conflict}`)
        return
      }

      const updated = { ...shortcuts, [recordingAction]: combo }
      setShortcuts(updated)
      saveShortcuts(updated)
      onShortcutsChanged?.(updated)
      setRecordingAction(null)
      setShortcutError(null)
    }

    window.addEventListener('keydown', handleKeyDown, true)
    return () => window.removeEventListener('keydown', handleKeyDown, true)
  }, [recordingAction, shortcuts, onShortcutsChanged])

  const handleReset = () => {
    const defaults = getDefaultShortcutMap()
    setShortcuts(defaults)
    saveShortcuts(defaults)
    onShortcutsChanged?.(defaults)
    setRecordingAction(null)
    setShortcutError(null)
  }

  return (
    <div className="flex-1 p-4 space-y-3 overflow-y-auto text-xs">
      <div className="flex items-center justify-between">
        <span className="text-zinc-400 text-[11px]">
          Focused shortcuts trigger only when the Nook panel is active.
        </span>
        <button
          type="button"
          onClick={handleReset}
          className="flex items-center gap-1 text-[11px] text-zinc-400 hover:text-white px-2 py-1 rounded bg-white/5 hover:bg-white/10 transition-colors"
        >
          <RotateCcw className="size-3" />
          <span>Reset defaults</span>
        </button>
      </div>

      {shortcutError && (
        <div className="p-2 bg-amber-500/10 border border-amber-500/20 text-amber-400 rounded-md text-[11px]">
          {shortcutError}
        </div>
      )}

      <div className="divide-y divide-white/5 rounded-lg border border-white/10 bg-white/5">
        {DEFAULT_SHORTCUTS.map((def) => {
          const currentKey = shortcuts[def.action] ?? def.defaultKey
          const isRecording = recordingAction === def.action
          return (
            <div key={def.action} className="flex items-center justify-between p-2.5">
              <div>
                <div className="font-medium text-zinc-200 text-xs">{def.label}</div>
                <div className="text-[10px] text-zinc-500">{def.description}</div>
              </div>
              <button
                type="button"
                data-recording={isRecording ? 'true' : undefined}
                onClick={() => {
                  setShortcutError(null)
                  setRecordingAction(isRecording ? null : def.action)
                }}
                className={`px-2.5 py-1 rounded font-mono text-[11px] border transition-colors ${
                  isRecording
                    ? 'bg-purple-600 border-purple-500 text-white animate-pulse'
                    : 'bg-black/30 border-white/10 text-zinc-300 hover:border-purple-500/50 hover:text-white'
                }`}
              >
                {isRecording ? 'Press key combo...' : currentKey || 'Not set'}
              </button>
            </div>
          )
        })}
      </div>
    </div>
  )
}

export function SettingsModal({
  currentSettings,
  onSave,
  onClose,
  onShortcutsChanged,
}: SettingsModalProps) {
  const [activeTab, setActiveTab] = useState<'model' | 'shortcuts'>('model')

  return (
    <div className="flex flex-col h-full bg-[#161618] border-b border-white/10 overflow-hidden text-zinc-200">
      <div className="flex items-center justify-between px-4 py-2 border-b border-white/10 bg-white/5">
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => setActiveTab('model')}
            className={`flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-semibold transition-colors ${
              activeTab === 'model'
                ? 'bg-purple-600 text-white'
                : 'text-zinc-400 hover:text-white hover:bg-white/5'
            }`}
          >
            <SettingsIcon className="size-3.5" />
            <span>Model Settings</span>
          </button>
          <button
            type="button"
            onClick={() => setActiveTab('shortcuts')}
            className={`flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-semibold transition-colors ${
              activeTab === 'shortcuts'
                ? 'bg-purple-600 text-white'
                : 'text-zinc-400 hover:text-white hover:bg-white/5'
            }`}
          >
            <Keyboard className="size-3.5" />
            <span>Shortcuts</span>
          </button>
        </div>
        <button
          onClick={onClose}
          type="button"
          aria-label="Close settings"
          className="text-zinc-400 hover:text-white transition-colors p-1 rounded-md hover:bg-white/10"
        >
          <X className="size-4" />
        </button>
      </div>

      {activeTab === 'model' ? (
        <ModelSettingsTab
          currentSettings={currentSettings}
          onSave={onSave}
          onClose={onClose}
        />
      ) : (
        <ShortcutsTab onShortcutsChanged={onShortcutsChanged} />
      )}
    </div>
  )
}
