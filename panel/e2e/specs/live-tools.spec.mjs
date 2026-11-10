import { existsSync, readFileSync } from 'node:fs'
import os from 'node:os'
import path from 'node:path'

describe('live file and terminal tools through the native Panel', () => {
  const filesDir = process.env.NOOK_E2E_FILES_DIR
  const note = path.join(filesDir, 'live-note.txt')
  const desktopName = `nook-e2e-${process.env.NOOK_E2E_RUN_ID}.txt`

  before(async () => {
    await browser.tauri.switchWindow('main')
    await browser.waitUntil(async () =>
      await browser.tauri.execute(({ core }) => core.invoke('ping_daemon')) === true,
    { timeout: 90_000, interval: 500, timeoutMsg: 'nookd did not become ready.' })
  })

  const send = async (text) => {
    const input = await $('[aria-label="Ask anything"]')
    await browser.waitUntil(async () => await input.isDisplayed() && await input.isEnabled(), { timeout: 10_000 })
    await input.setValue(text)
    await $('[aria-label="Send question"]').click()
  }

  const latestAssistant = async () => {
    await browser.waitUntil(async () => (await $$('[data-message-role="assistant"]')).length > 0, { timeout: 10_000 })
    return (await $$('[data-message-role="assistant"]')).at(-1)
  }

  const allowEveryApproval = async () => {
    const assistant = await latestAssistant()
    let approvals = 0
    const deadline = Date.now() + 240_000
    while (Date.now() < deadline) {
      const card = await $('[data-testid="approval-card"]')
      if (await card.isExisting()) {
        await card.$('button=Allow once').click()
        approvals += 1
      }
      const status = await assistant.getAttribute('data-message-status')
      if (status === 'complete') return { approvals, text: await assistant.getText() }
      if (status === 'error' || status === 'cancelled') {
        throw new Error(`The reply ended with ${status}: ${await assistant.getText()}`)
      }
      await browser.pause(250)
    }
    throw new Error(`The reply did not finish: ${await assistant.getText()}`)
  }

  const newChat = async () => {
    await $('[aria-label="Start new chat"]').click()
    await browser.waitUntil(async () => (await $$('[data-message-role="user"]')).length === 0, { timeout: 10_000 })
  }

  it('creates a file in the allowed folder when the prompt gives no path', async () => {
    await send('Create a file named live-note.txt with the text "hello from nook" in my allowed folder.')
    const { approvals, text } = await allowEveryApproval()
    if (approvals < 1) throw new Error(`The write never asked for approval. Reply: ${text}`)
    if (!existsSync(note)) throw new Error(`The file was not created. Reply: ${text}`)
    if (!readFileSync(note, 'utf8').includes('hello from nook')) {
      throw new Error(`The file has the wrong content: ${readFileSync(note, 'utf8')}`)
    }
  })

  it('replaces the file when asked to write over it', async () => {
    await newChat()
    await send('Replace the whole content of live-note.txt in my allowed folder with the text "second version".')
    const { text } = await allowEveryApproval()
    const content = readFileSync(note, 'utf8')
    if (!content.includes('second version') || content.includes('hello from nook')) {
      throw new Error(`The file was not replaced. Content: ${content}. Reply: ${text}`)
    }
  })

  it('writes nothing outside the allowed folder', async () => {
    await newChat()
    await send(`Create a file named ${desktopName} with the text "outside" on my Desktop.`)
    await allowEveryApproval()
    if (existsSync(path.join(os.homedir(), 'Desktop', desktopName))) {
      throw new Error('A file was written outside the allowed folder.')
    }
  })

  it('runs a command in the allowed folder after approval', async () => {
    await newChat()
    await send('Run ls in my allowed folder and tell me the file names you see.')
    const { approvals, text } = await allowEveryApproval()
    if (approvals < 1) throw new Error(`The command never asked for approval. Reply: ${text}`)
    if (!text.includes('live-note.txt')) throw new Error(`The reply does not list live-note.txt: ${text}`)
  })
})
