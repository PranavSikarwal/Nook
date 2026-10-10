describe('native History, persistence, and settings', () => {
  before(async () => {
    await browser.tauri.switchWindow('main')
    await browser.waitUntil(async () =>
      await browser.tauri.execute(({ core }) => core.invoke('ping_daemon')) === true,
    { timeout: 90_000, interval: 250 })
  })

  async function sendAndWait(prompt) {
    const input = await $('[aria-label="Ask anything"]')
    await input.setValue(prompt)
    await $('[aria-label="Send question"]').click()
    await $(`[data-message-role="user"]*=${prompt}`).waitForDisplayed({ timeout: 10_000 })
    await browser.waitUntil(async () => {
      const assistants = await $$('[data-message-role="assistant"]')
      return assistants.length > 0 && await assistants.at(-1).getAttribute('data-message-status') === 'complete'
    }, { timeout: 20_000, interval: 100 })
  }


  it('separates History select/delete controls and deletes only the selected test chat', async () => {
    await sendAndWait('E2E_REPLY:history target')
    await $('[aria-label="Toggle history drawer"]').click()
    await $('[aria-label="Close history"]').waitForDisplayed({ timeout: 10_000 })
    if ((await browser.$$('button button')).length !== 0) throw new Error('History contains nested buttons.')

    const row = await $('[data-chat-row][data-active="true"]')
    const chatId = await row.getAttribute('data-chat-row')
    await row.$('button[data-chat-select]').click()
    await browser.waitUntil(async () => await browser.execute(() =>
      document.querySelector('[data-chat-row][data-active="true"]')?.getAttribute('data-chat-row')) === chatId,
    { timeout: 10_000 })

    await $('[aria-label="Toggle history drawer"]').click()
    await $('[aria-label="Close history"]').waitForDisplayed({ timeout: 10_000 })
    await $(`[data-chat-row="${chatId}"] button[data-chat-delete]`).click()
    await browser.waitUntil(async () => (await $$(`[data-chat-row="${chatId}"]`)).length === 0, { timeout: 10_000 })
  })

  it('restores chat history after reloading the Panel', async () => {
    const prompt = 'E2E_REPLY:persist across reload'
    await sendAndWait(prompt)
    await browser.refresh()
    await browser.tauri.switchWindow('main')
    await browser.waitUntil(async () =>
      await browser.tauri.execute(({ core }) => core.invoke('ping_daemon')) === true,
    { timeout: 90_000, interval: 250 })
    await $('[aria-label="Toggle history drawer"]').click()
    const row = await $('[data-chat-row]')
    await row.$('button[data-chat-select]').click()
    await browser.waitUntil(async () => {
      const userMessages = await $$('[data-message-role="user"]')
      for (const message of userMessages) {
        if ((await message.getText()).includes(prompt)) return true
      }
      return false
    }, { timeout: 15_000 })
  })

  it('saves settings only in this E2E app-data root', async () => {
    await $('[aria-label="Toggle settings modal"]').click()
    await $('#settings-base-url').setValue('https://isolated.example.test/v1')
    await $('#settings-model').setValue('e2e-isolated-model')
    await $('[type="submit"]').click()
    await browser.waitUntil(async () => !(await $('#settings-base-url').isDisplayed().catch(() => false)), {
      timeout: 10_000,
    })
    await $('[aria-label="Toggle settings modal"]').click()
    await browser.waitUntil(async () =>
      await $('#settings-base-url').getValue() === 'https://isolated.example.test/v1'
        && await $('#settings-model').getValue() === 'e2e-isolated-model',
    { timeout: 10_000 })
    await $('[aria-label="Close settings"]').click()
  })
})
