import { existsSync, readFileSync, realpathSync } from 'node:fs'
import path from 'node:path'

describe('native Settings: allowed folders', () => {
  const filesDir = process.env.NOOK_E2E_FILES_DIR
  const extraDir = process.env.NOOK_E2E_PICK_FOLDER
  const listedExtra = realpathSync(extraDir)
  const target = path.join(listedExtra, 'added.txt')

  before(async () => {
    await browser.tauri.switchWindow('main')
    await browser.waitUntil(async () =>
      await browser.tauri.execute(({ core }) => core.invoke('ping_daemon')) === true,
    { timeout: 90_000, interval: 250 })
  })

  const openFoldersFromMenu = async () => {
    await browser.tauri.execute(({ core }) =>
      core.invoke('plugin:event|emit', { event: 'open_settings', payload: 'folders' }))
    await $('[data-testid="folders-tab"]').waitForDisplayed({ timeout: 10_000 })
    await browser.waitUntil(async () => (await $$('[data-testid="folder-row"]')).length > 0,
      { timeout: 10_000, interval: 100, timeoutMsg: 'No folder was listed.' })
  }

  const closeSettings = async () => {
    await $('[aria-label="Close settings"]').click()
    await $('[data-testid="folders-tab"]').waitForDisplayed({ reverse: true, timeout: 10_000 })
  }

  const row = (folder) => $(`[data-testid="folder-row"][data-folder-path="${folder}"]`)

  const send = async (prompt) => {
    await $('[aria-label="Ask anything"]').setValue(prompt)
    await $('[aria-label="Send question"]').click()
    await $(`[data-message-role="user"]*=${prompt}`).waitForDisplayed({ timeout: 10_000 })
  }

  const waitForLatestComplete = async (containing) => {
    await browser.waitUntil(async () => {
      const assistants = await $$('[data-message-role="assistant"]')
      if (assistants.length === 0) return false
      const latest = assistants.at(-1)
      return await latest.getAttribute('data-message-status') === 'complete' &&
        (await latest.getText()).includes(containing)
    }, { timeout: 20_000, interval: 100, timeoutMsg: `The reply never contained "${containing}".` })
  }

  it('opens on the Folders tab from the menu event and lists the configured folder', async () => {
    await openFoldersFromMenu()

    const configured = row(filesDir)
    if (await configured.getAttribute('data-folder-access') !== 'read_write') {
      throw new Error('The configured folder is not listed as read and write.')
    }
  })

  it('adds a folder as read only and refuses a write there at once', async () => {
    await $('[data-testid="folders-add"]').click()
    await row(listedExtra).waitForDisplayed({ timeout: 10_000 })
    if (await row(listedExtra).getAttribute('data-folder-access') !== 'read') {
      throw new Error('A new folder did not start as read only.')
    }
    await closeSettings()

    await send(`E2E_FILE_WRITE:${target}|hi`)
    await waitForLatestComplete('Tool denied: The folder is read-only')
    if (existsSync(target)) throw new Error('A read-only folder received a write.')
  })

  it('lets the user write after the access changes to read and write', async () => {
    await openFoldersFromMenu()
    await row(listedExtra).$('[data-testid="folder-access-read_write"]').click()
    await browser.waitUntil(async () =>
      await row(listedExtra).getAttribute('data-folder-access') === 'read_write',
    { timeout: 10_000, interval: 100, timeoutMsg: 'The access did not change.' })
    await closeSettings()

    await send(`E2E_FILE_WRITE:${target}|hi`)
    const card = await $('[data-testid="approval-card"]')
    await card.waitForDisplayed({ timeout: 10_000 })
    await card.$('button=Allow once').click()
    await waitForLatestComplete('Tool ok: Wrote 2 bytes')
    if (readFileSync(target, 'utf8') !== 'hi') throw new Error('The file has the wrong content.')
  })

  it('refuses a path again after the folder is removed', async () => {
    await openFoldersFromMenu()
    await row(listedExtra).$('[data-testid="folder-remove"]').click()
    await row(listedExtra).waitForExist({ reverse: true, timeout: 10_000 })
    if (!(await row(filesDir).isExisting())) throw new Error('The configured folder vanished.')
    await closeSettings()

    await send(`E2E_FILE_WRITE:${path.join(listedExtra, 'late.txt')}|no`)
    await waitForLatestComplete('Tool denied: Path is outside the allowed folders')
    if (existsSync(path.join(listedExtra, 'late.txt'))) {
      throw new Error('A removed folder received a write.')
    }
  })
})
