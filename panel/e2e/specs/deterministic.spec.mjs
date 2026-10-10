describe('native request lifecycle', () => {
  before(async () => {
    await browser.tauri.switchWindow('main')
    await browser.waitUntil(async () =>
      await browser.tauri.execute(({ core }) => core.invoke('ping_daemon')) === true,
    { timeout: 90_000, interval: 250, timeoutMsg: 'The deterministic Daemon did not become ready.' })
  })

  async function latestAssistant() {
    const assistants = await $$('[data-message-role="assistant"]')
    return assistants[assistants.length - 1]
  }

  async function waitForAssistantStatus(status) {
    await browser.waitUntil(async () =>
      (await latestAssistant()).getAttribute('data-message-status').then((value) => value === status),
    { timeout: 20_000, interval: 100, timeoutMsg: `Assistant did not reach ${status}.` })
    return latestAssistant()
  }

  it('completes a plain request and preserves a second-send draft', async () => {
    const input = await $('[aria-label="Ask anything"]')
    const firstPrompt = 'E2E_REPLY:first reply'
    await input.setValue(firstPrompt)
    await $('[aria-label="Send question"]').click()
    await $(`[data-message-role="user"]*=${firstPrompt}`).waitForDisplayed({ timeout: 10_000 })
    if (!(await (await waitForAssistantStatus('complete')).getText()).includes('first reply')) {
      throw new Error('The first deterministic response did not complete.')
    }

    const slowPrompt = 'E2E_WAIT:slow reply'
    await input.setValue(slowPrompt)
    await $('[aria-label="Send question"]').click()
    await $(`[data-message-role="user"]*=${slowPrompt}`).waitForDisplayed({ timeout: 10_000 })
    await waitForAssistantStatus('streaming')
    const draft = 'E2E_REPLY:second reply'
    await input.setValue(draft)
    if (await $('[aria-label="Send question"]').isExisting()) {
      throw new Error('A second send control appeared while a request was streaming.')
    }
    await waitForAssistantStatus('complete')
    if (await input.getValue() !== draft) throw new Error('The queued draft was lost.')
    await $('[aria-label="Send question"]').click()
    await $(`[data-message-role="user"]*=${draft}`).waitForDisplayed({ timeout: 10_000 })
    if (!(await (await waitForAssistantStatus('complete')).getText()).includes('second reply')) {
      throw new Error('The follow-up request did not receive its own response.')
    }
  })

  it('cancels a request without letting its later events change the next reply', async () => {
    const input = await $('[aria-label="Ask anything"]')
    const slowPrompt = 'E2E_WAIT:cancel this reply'
    await input.setValue(slowPrompt)
    await $('[aria-label="Send question"]').click()
    await $(`[data-message-role="user"]*=${slowPrompt}`).waitForDisplayed({ timeout: 10_000 })
    await waitForAssistantStatus('streaming')
    await $('[aria-label="Stop generation"]').click()
    const cancelledReply = await latestAssistant()
    await browser.waitUntil(async () =>
      ['cancelled', 'complete'].includes(await cancelledReply.getAttribute('data-message-status')),
    { timeout: 10_000, interval: 100 })

    const nextPrompt = 'E2E_REPLY:reply after cancel'
    await input.setValue(nextPrompt)
    await $('[aria-label="Send question"]').click()
    await $(`[data-message-role="user"]*=${nextPrompt}`).waitForDisplayed({ timeout: 10_000 })
    if (!(await (await waitForAssistantStatus('complete')).getText()).includes('reply after cancel')) {
      throw new Error('A stale cancellation event changed the next response.')
    }
  })

  it('reaches a visible error terminal state for scripted failures', async () => {
    const input = await $('[aria-label="Ask anything"]')
    const prompt = 'E2E_ERROR:scripted worker error'
    await input.setValue(prompt)
    await $('[aria-label="Send question"]').click()
    await $(`[data-message-role="user"]*=${prompt}`).waitForDisplayed({ timeout: 10_000 })
    const reply = await waitForAssistantStatus('error')
    if (!(await reply.getText()).includes('scripted worker error')) {
      throw new Error('The scripted Worker error was not shown on its assistant message.')
    }
  })
})
