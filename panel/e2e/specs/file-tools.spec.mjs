import { existsSync, readFileSync } from 'node:fs'
import path from 'node:path'

describe('native file tools run in the Daemon', () => {
  const filesDir = process.env.NOOK_E2E_FILES_DIR

  before(async () => {
    await browser.tauri.switchWindow('main')
    await browser.waitUntil(async () =>
      await browser.tauri.execute(({ core }) => core.invoke('ping_daemon')) === true,
    { timeout: 90_000, interval: 250 })
  })

  const waitForApproval = async () => {
    const card = await $('[data-testid="approval-card"]')
    await card.waitForDisplayed({ timeout: 10_000 })
    return card
  }

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
    return (await $$('[data-message-role="assistant"]')).at(-1)
  }

  it('reads a file after the user allows it once', async () => {
    const file = path.join(filesDir, 'notes.txt')
    await send(`E2E_FILE_READ:${file}`)
    const card = await waitForApproval()
    const text = await card.getText()
    if (!text.includes('notes.txt') || !text.includes('Read ')) {
      throw new Error(`The approval did not name the file: ${text}`)
    }
    await card.$('button=Allow once').click()
    const reply = await waitForLatestComplete('Tool ok: e2e notes')
    await reply.$('[data-testid="tool-call"][data-tool-status="finished"]').waitForExist({
      timeout: 10_000,
      timeoutMsg: 'No finished tool call appeared.',
    })
    if (!(await reply.$('[data-testid="tool-call"]').getText()).includes('read local file')) {
      throw new Error('The tool call line did not name the tool.')
    }
  })

  it('refuses a path outside the allowed folder without asking', async () => {
    await send('E2E_FILE_READ:/etc/hosts')
    await waitForLatestComplete('Tool denied: Path is outside the allowed folders')
    if (await $('[data-testid="approval-card"]').isExisting()) {
      throw new Error('An approval card appeared for a path outside the allowed folder.')
    }
  })

  it('writes nothing after Deny and writes the file after Allow once', async () => {
    const target = path.join(filesDir, 'out.txt')
    await send(`E2E_FILE_WRITE:${target}|hello from e2e`)
    const card = await waitForApproval()
    if (!(await card.getText()).includes('14 bytes')) {
      throw new Error('The write approval did not show the size.')
    }
    await card.$('button=Deny').click()
    await waitForLatestComplete('Tool denied: Denied by the user')
    if (existsSync(target)) throw new Error('The file exists after Deny.')

    await send(`E2E_FILE_WRITE:${target}|hello from e2e`)
    await (await waitForApproval()).$('button=Allow once').click()
    await waitForLatestComplete('Tool ok: Wrote 14 bytes')
    if (readFileSync(target, 'utf8') !== 'hello from e2e') {
      throw new Error('The file has the wrong content.')
    }

    await send(`E2E_FILE_WRITE:${target}|second try`)
    await (await waitForApproval()).$('button=Allow once').click()
    await waitForLatestComplete('Tool error: Error: file already exists')
    if (readFileSync(target, 'utf8') !== 'hello from e2e') {
      throw new Error('A write without overwrite replaced the file.')
    }
  })

  it('does not ask again inside a folder the user allowed for the chat', async () => {
    await $('[aria-label="Start new chat"]').click()
    await browser.waitUntil(async () => (await $$('[data-message-role="user"]')).length === 0, { timeout: 10_000 })

    await send(`E2E_FILE_READ:${path.join(filesDir, 'notes.txt')}`)
    await (await waitForApproval()).$('button=Allow this folder for this chat').click()
    await waitForLatestComplete('Tool ok: e2e notes')

    await send(`E2E_FILE_READ:${path.join(filesDir, 'more.txt')}`)
    await waitForLatestComplete('Tool ok: more notes')
    if (await $('[data-testid="approval-card"]').isExisting()) {
      throw new Error('An approval card appeared inside a folder that already had a grant.')
    }

    await send(`E2E_FILE_LIST:${filesDir}`)
    await (await waitForApproval()).$('button=Deny').click()
    await waitForLatestComplete('Tool denied: Denied by the user')
  })

  it('ends the reply as cancelled when Escape is pressed during a file approval', async () => {
    const target = path.join(filesDir, 'escaped.txt')
    await send(`E2E_FILE_WRITE:${target}|never written`)
    await waitForApproval()
    await browser.keys('Escape')
    await browser.waitUntil(async () => {
      const assistants = await $$('[data-message-role="assistant"]')
      return assistants.length > 0 &&
        await assistants.at(-1).getAttribute('data-message-status') === 'cancelled'
    }, { timeout: 10_000, interval: 100, timeoutMsg: 'Escape did not end the reply as cancelled.' })
    if (await $('[data-testid="approval-card"]').isExisting()) {
      throw new Error('The approval card stayed after Escape.')
    }
    if (existsSync(target)) throw new Error('A cancelled write created the file.')
  })
})
