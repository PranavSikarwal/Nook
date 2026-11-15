import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  canSaveNotesFolder,
  getNotesFolderDisplay,
  getNotesFolderReason,
  isNotesFolderVisible,
} from '../../src/lib/notesFolder.ts'

test('the field is hidden when supported is false', () => {
  assert.equal(isNotesFolderVisible({ folder: 'Nook', supported: false }), false)
})

test('the field is hidden when settings are absent', () => {
  assert.equal(isNotesFolderVisible(null), false)
  assert.equal(isNotesFolderVisible(undefined), false)
})

test('the field is visible when supported is true', () => {
  assert.equal(isNotesFolderVisible({ folder: 'Nook', supported: true }), true)
})

test('shows the stored folder from settings', () => {
  const display = getNotesFolderDisplay({ folder: 'Custom Notes', supported: true })
  assert.equal(display.folder, 'Custom Notes')
})

test('shows the reason when present in settings', () => {
  const settings = {
    folder: 'Bad:Folder',
    supported: true,
    reason: 'The Notes folder name is not valid',
  }
  const display = getNotesFolderDisplay(settings)
  assert.equal(display.reason, 'The Notes folder name is not valid')
  assert.equal(getNotesFolderReason(settings), 'The Notes folder name is not valid')
})

test('reason is omitted when not present in settings', () => {
  const settings = { folder: 'Nook', supported: true }
  const display = getNotesFolderDisplay(settings)
  assert.equal(display.reason, undefined)
  assert.equal(getNotesFolderReason(settings), undefined)
})

test('save is disabled when the input is empty', () => {
  assert.equal(canSaveNotesFolder('', 'Nook'), false)
})

test('save is disabled when the input is only whitespace', () => {
  assert.equal(canSaveNotesFolder('   ', 'Nook'), false)
})

test('save is disabled when the value has not changed', () => {
  assert.equal(canSaveNotesFolder('Nook', 'Nook'), false)
})

test('save is disabled when the value has not changed after trimming', () => {
  assert.equal(canSaveNotesFolder('  Nook  ', 'Nook'), false)
})

test('save is enabled when the trimmed value has changed and is not empty', () => {
  assert.equal(canSaveNotesFolder('Work', 'Nook'), true)
  assert.equal(canSaveNotesFolder('  Personal Notes  ', 'Nook'), true)
})
