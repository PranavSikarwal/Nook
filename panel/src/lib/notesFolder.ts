import type { NotesSettings } from './types'

/** Whether the Notes folder field should be visible. */
export function isNotesFolderVisible(settings: NotesSettings | null | undefined): boolean {
  return Boolean(settings?.supported)
}

/** Whether the Save button is enabled for the edited folder value against the stored folder. */
export function canSaveNotesFolder(current: string, stored: string): boolean {
  const trimmed = current.trim()
  return trimmed.length > 0 && trimmed !== stored.trim()
}
