import { existsSync } from 'node:fs'
import path from 'node:path'

describe('native command tool runs in the Daemon sandbox', () => {
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

  it('shows the command line and runs it only after Allow once', async () => {
    await send('E2E_COMMAND:echo sandboxed hello')
    const card = await waitForApproval()
    const text = await card.getText()
    if (!text.includes('Run `echo sandboxed hello`')) {
      throw new Error(`The approval did not show the command: ${text}`)
    }
    if (await card.$('button=Allow for this chat').isExisting()) {
      throw new Error('A command offered a chat-wide grant.')
    }
    await card.$('button=Allow once').click()
    const reply = await waitForLatestComplete('exit code: 0')
    if (!(await reply.getText()).includes('sandboxed hello')) {
      throw new Error('The reply did not include the command output.')
    }
    await reply.$('[data-testid="tool-call"][data-tool-status="finished"]').waitForExist({
      timeout: 10_000,
      timeoutMsg: 'No finished tool call appeared.',
    })
  })

  it('does not run a denied command', async () => {
    const marker = path.join(filesDir, 'should-not-exist.txt')
    await send(`E2E_COMMAND:touch ${marker}`)
    await (await waitForApproval()).$('button=Deny').click()
    await waitForLatestComplete('Tool denied: Denied by the user')
    if (existsSync(marker)) throw new Error('A denied command created a file.')
  })

  it('runs an approved command inside the allowed folder only', async () => {
    const inside = path.join(filesDir, 'from-command.txt')
    await send(`E2E_COMMAND:touch ${inside}`)
    await (await waitForApproval()).$('button=Allow once').click()
    await waitForLatestComplete('exit code: 0')
    if (!existsSync(inside)) throw new Error('The command did not create the file in the allowed folder.')

    const outside = '/tmp/nook-e2e-outside-marker'
    await send(`E2E_COMMAND:touch ${outside}`)
    await (await waitForApproval()).$('button=Allow once').click()
    const reply = await waitForLatestComplete('exit code:')
    if ((await reply.getText()).includes('exit code: 0')) {
      throw new Error('A command wrote outside the allowed folder.')
    }
    if (existsSync(outside)) throw new Error('A command created a file outside the allowed folder.')
  })

  it('refuses a program the policy closes even after Allow once', async () => {
    await send('E2E_COMMAND:sudo true')
    await (await waitForApproval()).$('button=Allow once').click()
    await waitForLatestComplete('Tool denied: Policy does not allow this request')
  })
})
