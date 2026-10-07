export type ShortcutAction =
  | 'focus_input'
  | 'new_chat'
  | 'toggle_history'
  | 'open_settings'
  | 'attach_file'
  | 'cancel_active_work'
  | 'close_auxiliary_view'

export interface ShortcutDefinition {
  action: ShortcutAction
  label: string
  description: string
  defaultKey: string // normalized key string
}

export type ShortcutMap = Record<ShortcutAction, string>

const IS_MAC =
  typeof navigator !== 'undefined' &&
  /Mac|iPod|iPhone|iPad/.test(navigator.platform || '')

export const DEFAULT_SHORTCUTS: ShortcutDefinition[] = [
  {
    action: 'focus_input',
    label: 'Focus input',
    description: 'Jump directly to the question input field',
    defaultKey: IS_MAC ? 'Cmd+L' : 'Ctrl+L',
  },
  {
    action: 'new_chat',
    label: 'New chat',
    description: 'Start a fresh chat conversation',
    defaultKey: IS_MAC ? 'Cmd+N' : 'Ctrl+N',
  },
  {
    action: 'toggle_history',
    label: 'Toggle history',
    description: 'Open or close the history drawer',
    defaultKey: IS_MAC ? 'Cmd+H' : 'Ctrl+H',
  },
  {
    action: 'open_settings',
    label: 'Open settings',
    description: 'Open or close the settings modal',
    defaultKey: IS_MAC ? 'Cmd+,' : 'Ctrl+,',
  },
  {
    action: 'attach_file',
    label: 'Attach file',
    description: 'Open file picker to attach documents or images',
    defaultKey: IS_MAC ? 'Cmd+U' : 'Ctrl+U',
  },
  {
    action: 'cancel_active_work',
    label: 'Cancel active work',
    description: 'Stop streaming generation or cancel pending tool approval',
    defaultKey: 'Escape',
  },
  {
    action: 'close_auxiliary_view',
    label: 'Close drawer or modal',
    description: 'Close open history drawer or settings modal',
    defaultKey: 'Escape',
  },
]

const STORAGE_KEY = 'nook_shortcuts_v1'

export function getDefaultShortcutMap(): ShortcutMap {
  const map = {} as ShortcutMap
  for (const def of DEFAULT_SHORTCUTS) {
    map[def.action] = def.defaultKey
  }
  return map
}

export function loadShortcuts(): ShortcutMap {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return getDefaultShortcutMap()
    const parsed = JSON.parse(raw) as Partial<ShortcutMap>
    return {
      ...getDefaultShortcutMap(),
      ...parsed,
    }
  } catch {
    return getDefaultShortcutMap()
  }
}

export function saveShortcuts(shortcuts: ShortcutMap): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(shortcuts))
  } catch {
    // localStorage full or restricted
  }
}

/**
 * Normalizes a KeyboardEvent into a standard string representation,
 * e.g., "Cmd+K", "Ctrl+Shift+L", "Escape".
 */
export function normalizeKeyboardEvent(e: KeyboardEvent): string | null {
  // Ignore bare modifiers
  if (['Control', 'Shift', 'Alt', 'Meta'].includes(e.key)) {
    return null
  }

  const parts: string[] = []
  if (e.metaKey) parts.push('Cmd')
  if (e.ctrlKey) parts.push('Ctrl')
  if (e.altKey) parts.push('Alt')
  if (e.shiftKey) parts.push('Shift')

  let keyName = e.key
  if (keyName === ' ') keyName = 'Space'
  else if (keyName === 'Escape') keyName = 'Escape'
  else if (keyName === 'Enter') keyName = 'Enter'
  else if (keyName.length === 1) keyName = keyName.toUpperCase()

  parts.push(keyName)
  return parts.join('+')
}

/**
 * Checks if a key combination is already used by another action.
 */
export function findShortcutConflict(
  action: ShortcutAction,
  keyCombo: string,
  currentShortcuts: ShortcutMap,
): ShortcutAction | null {
  for (const [act, key] of Object.entries(currentShortcuts)) {
    if (act !== action && key.toLowerCase() === keyCombo.toLowerCase()) {
      return act as ShortcutAction
    }
  }
  return null
}
