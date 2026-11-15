import type { NotesSettings } from './types'

export interface NotesFolderDisplay {
  folder: string
  reason?: string
}

/** Whether the Notes folder field should be visible. */
export function isNotesFolderVisible(settings: NotesSettings | null | undefined): boolean {
  return Boolean(settings?.supported)
}

/** Returns the display folder and optional reason from settings. */
export function getNotesFolderDisplay(settings: NotesSettings | null | undefined): NotesFolderDisplay {
  return {
    folder: settings?.folder ?? '',
    ...(settings?.reason ? { reason: settings.reason } : {}),
  }
}

/** Returns the reason from notes settings if present. */
export function getNotesFolderReason(settings: NotesSettings | null | undefined): string | undefined {
  return settings?.reason
}

/** Whether the Save button is enabled for the edited folder value against the stored folder. */
export function canSaveNotesFolder(current: string, stored: string): boolean {
  const trimmed = current.trim()
  return trimmed.length > 0 && trimmed !== stored.trim()
}
