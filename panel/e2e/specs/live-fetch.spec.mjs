const FETCH_URL = 'https://huggingface.co/docs/transformers/index'
const SECOND_URL = 'https://huggingface.co/docs/transformers/quicktour'
const THIRD_URL = 'https://huggingface.co/docs/transformers/installation'

const promptFor = (url) =>
  `Use the web fetch tool to read ${url} and summarize it in two sentences. Do not use web search.`

describe('live web fetch approval through the native Panel', () => {
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
    const assistants = await $$('[data-message-role="assistant"]')
    return assistants.at(-1)
  }

  const waitForApproval = async () => {
    const card = await $('[data-testid="approval-card"]')
    await card.waitForDisplayed({
      timeout: 120_000,
      timeoutMsg: 'The model did not request web fetch approval, or the request stalled before the approval card.',
    })
    return card
  }

  const waitForStatus = async (wanted) => {
    const assistant = await latestAssistant()
    const deadline = Date.now() + 240_000
    while (Date.now() < deadline) {
      const status = await assistant.getAttribute('data-message-status')
      if (status === wanted) return assistant
      if (status === 'error') throw new Error(`The reply ended with an error: ${await assistant.getText()}`)
      if (wanted !== 'cancelled' && status === 'cancelled') throw new Error('The reply was cancelled.')
      await browser.pause(250)
    }
    throw new Error(`The reply did not reach ${wanted}: ${await assistant.getText()}`)
  }

  const newChat = async () => {
    await $('[aria-label="Start new chat"]').click()
    await browser.waitUntil(async () => (await $$('[data-message-role="user"]')).length === 0, { timeout: 10_000 })
  }

  it('shows an approval card and ends cleanly after Deny', async () => {
    await send(promptFor(FETCH_URL))
    const card = await waitForApproval()
    await card.$('button=Deny').click()
    const assistant = await waitForStatus('complete')
    if (!(await assistant.getText()).includes('Request denied')) {
      throw new Error('The reply did not end with the denial notice after Deny.')
    }
  })

  it('fetches the page after Allow once', async () => {
    await newChat()
    await send(promptFor(FETCH_URL))
    const card = await waitForApproval()
    await card.$('button=Allow once').click()
    const assistant = await waitForStatus('complete')
    if ((await assistant.getText()).trim().length < 40) throw new Error('The reply after Allow once has no summary.')
  })

  it('does not ask again for the same host after a chat and host grant', async () => {
    await newChat()
    await send(promptFor(SECOND_URL))
    const card = await waitForApproval()
    await card.$('button=Allow for this chat & host').click()
    await waitForStatus('complete')

    await send(promptFor(THIRD_URL))
    const assistant = await latestAssistant()
    const deadline = Date.now() + 240_000
    while (Date.now() < deadline) {
      if (await $('[data-testid="approval-card"]').isExisting()) {
        throw new Error('An approval card appeared for a host that already had a grant.')
      }
      const status = await assistant.getAttribute('data-message-status')
      if (status === 'complete') return
      if (status === 'error' || status === 'cancelled') throw new Error(`The reply ended with ${status}.`)
      await browser.pause(250)
    }
    throw new Error('The second request on the granted host did not finish.')
  })

  it('cancels a reply that waits for approval', async () => {
    await newChat()
    await send(promptFor(FETCH_URL))
    await waitForApproval()
    await browser.keys('Escape')
    await waitForStatus('cancelled')
    if (await $('[data-testid="approval-card"]').isExisting()) {
      throw new Error('The approval card stayed after cancel.')
    }
  })
})
