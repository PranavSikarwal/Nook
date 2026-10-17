import { readFile } from 'node:fs/promises'
import path from 'node:path'

const LINK = 'https://example.com/docs'
const OPENED = path.join(process.env.NOOK_E2E_RUN_DIR ?? '', 'opened-links.log')

async function openedLinks() {
  return (await readFile(OPENED, 'utf8').catch(() => '')).split('\n').filter(Boolean)
}

describe('native links in replies', () => {
  before(async () => {
    await browser.tauri.switchWindow('main')
    await browser.waitUntil(async () =>
      await browser.tauri.execute(({ core }) => core.invoke('ping_daemon')) === true,
    { timeout: 90_000, interval: 250, timeoutMsg: 'The deterministic Daemon did not become ready.' })
  })

  async function sendAndWait(prompt) {
    await $('[aria-label="Ask anything"]').setValue(prompt)
    await $('[aria-label="Send question"]').click()
    await browser.waitUntil(async () => {
      const assistants = await $$('[data-message-role="assistant"]')
      const last = assistants[assistants.length - 1]
      return last && await last.getAttribute('data-message-status') === 'complete'
    }, { timeout: 20_000, interval: 100, timeoutMsg: 'The reply did not complete.' })
  }

  it('keeps the Panel on screen when a link in a reply is clicked', async () => {
    const before = await browser.execute(() => window.location.href)
    await sendAndWait(`E2E_REPLY:Read [the docs](${LINK}) now`)

    await $(`a[href="${LINK}"]`).click()
    await browser.pause(1500)

    if (!(await $('[aria-label="Ask anything"]').isExisting())) {
      throw new Error('Clicking a link replaced the Panel.')
    }
    if (await browser.execute(() => window.location.href) !== before) {
      throw new Error('Clicking a link navigated the Panel window.')
    }
    await browser.waitUntil(async () => (await openedLinks()).includes(LINK),
      { timeout: 5_000, interval: 100, timeoutMsg: 'The clicked link was not opened.' })
  })

  it('sends a clicked link to the system opener', async () => {
    await sendAndWait(`E2E_REPLY:See [the guide](${LINK}/guide)`)

    await $(`a[href="${LINK}/guide"]`).click()

    await browser.waitUntil(async () => (await openedLinks()).includes(`${LINK}/guide`),
      { timeout: 5_000, interval: 100, timeoutMsg: 'No link reached the opener.' })
  })

  it('does not make a link out of a script or file address', async () => {
    await sendAndWait('E2E_REPLY:Bad [one](javascript:alert(1)) and [two](file:///etc/passwd)')

    const anchors = await $$('a[href^="javascript:"], a[href^="file:"]')
    if (anchors.length !== 0) throw new Error('A script or file address became a link.')
  })
})
