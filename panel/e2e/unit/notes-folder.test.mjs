import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  canSaveNotesFolder,
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
