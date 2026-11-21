import { readFileSync } from 'node:fs'
import path from 'node:path'

describe('native Notes tools', () => {
  const osascriptLog = process.env.NOOK_E2E_OSASCRIPT_LOG ??
    path.join(process.env.NOOK_E2E_RUN_DIR ?? '', 'osascript.log')

  before(async () => {
    await browser.tauri.switchWindow('main')
    await browser.waitUntil(async () =>
      await browser.tauri.execute(({ core }) => core.invoke('ping_daemon')) === true,
    { timeout: 90_000, interval: 250 })
  })

  const send = async (prompt) => {
    await $('[aria-label="Ask anything"]').setValue(prompt)
    await $('[aria-label="Send question"]').click()
    await $(`[data-message-role="user"]*=${prompt}`).waitForDisplayed({ timeout: 10_000 })
  }

  const waitForApproval = async () => {
    const card = await $('[data-testid="approval-card"]')
    await card.waitForDisplayed({ timeout: 10_000 })
    return card
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

  const openTools = async () => {
    await $('[aria-label="Toggle settings modal"]').click()
    await $('button*=Tools').click()
    await $('[data-testid="tools-tab"]').waitForDisplayed({ timeout: 10_000 })
    await browser.waitUntil(async () => (await $$('[data-testid="tool-source-row"]')).length > 0,
      { timeout: 10_000, interval: 100, timeoutMsg: 'No tool source was listed.' })
  }

  const closeSettings = async () => {
    await $('[aria-label="Close settings"]').click()
    await $('[data-testid="tools-tab"]').waitForDisplayed({ reverse: true, timeout: 10_000 })
  }

  it('leaves the file empty when an approval is denied', async () => {
    if (readFileSync(osascriptLog, 'utf8') !== '') {
      throw new Error('The log file was not empty at start.')
    }

    await send('E2E_NOTES:create|Denied Note|Body of denied note')
    const card = await waitForApproval()
    await card.$('button=Deny').click()
    await waitForLatestComplete('Tool denied: Denied by the user')

    if (readFileSync(osascriptLog, 'utf8') !== '') {
      throw new Error('A denied call ran the program.')
    }
  })

  it('does not run the program when cancelled before approval', async () => {
    await send('E2E_NOTES:create|Cancelled Note|Body of cancelled note')
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

    if (readFileSync(osascriptLog, 'utf8') !== '') {
      throw new Error('A cancelled call ran the program.')
    }
  })

  it('runs the program only after Allow', async () => {
    await send('E2E_NOTES:create|Allowed Note|Hello Notes')
    const card = await waitForApproval()

    if (readFileSync(osascriptLog, 'utf8') !== '') {
      throw new Error('The program ran before Allow.')
    }

    await card.$('button=Allow once').click()
    await waitForLatestComplete('Tool ok:')

    const logged = readFileSync(osascriptLog, 'utf8')
    if (!logged.includes(':Nook') || !logged.includes('Allowed Note')) {
      throw new Error(`The program did not run with expected arguments after Allow: ${logged}`)
    }
  })

  it('shows no chat button on the delete approval card', async () => {
    await send('E2E_NOTES:delete|x-coredata://e2e-note-id/1|Allowed Note')
    const card = await waitForApproval()

    if (await card.$('button*=chat').isExisting()) {
      throw new Error('The delete approval card offered a chat grant.')
    }

    await card.$('button=Deny').click()
    await waitForLatestComplete('Tool denied: Denied by the user')
  })

  it('passes a new folder from the Settings field to the next call', async () => {
    await openTools()
    const field = await $('[data-testid="notes-folder-field"]')
    await field.waitForDisplayed({ timeout: 10_000 })

    if (await field.$('[data-testid="notes-folder-reason"]').isExisting()) {
      throw new Error('The Notes folder field unexpectedly shows an error reason.')
    }

    const input = await field.$('[data-testid="notes-folder-input"]')
    await input.setValue('CustomFolder')

    const saveBtn = await field.$('[data-testid="notes-folder-save"]')
    await browser.waitUntil(async () => await saveBtn.isEnabled(),
      { timeout: 5_000, timeoutMsg: 'The save button was not enabled after changing folder name.' })
    await saveBtn.click()

    await browser.waitUntil(async () => !(await saveBtn.isEnabled()),
      { timeout: 10_000, interval: 100, timeoutMsg: 'The save button did not disable after saving.' })

    await closeSettings()

    await send('E2E_NOTES:create|Custom Note|Body in custom folder')
    const card = await waitForApproval()
    await card.$('button=Allow once').click()
    await waitForLatestComplete('Tool ok:')

    const logged = readFileSync(osascriptLog, 'utf8')
    if (!logged.includes(':CustomFolder')) {
      throw new Error(`The new folder did not reach the next call: ${logged}`)
    }
  })
})
