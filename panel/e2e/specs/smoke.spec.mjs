describe('native Nook smoke', () => {
  it('blocks sends until the delayed Daemon is ready', async () => {
    await browser.tauri.switchWindow('main')
    await browser.waitUntil(async () => (await browser.getTitle()).length > 0, {
      timeout: 30_000,
      timeoutMsg: 'Nook window did not become available',
    })

    const input = await $('[aria-label="Ask anything"]')
    await input.waitForDisplayed({ timeout: 10_000 })
    await browser.waitUntil(async () => !(await input.isEnabled()), {
      timeout: 5_000,
      interval: 100,
      timeoutMsg: 'The Panel input was enabled before the delayed Daemon started.',
    })
    const sendButtonBeforeReady = await $('[aria-label="Send question"]')
    if (await sendButtonBeforeReady.isExisting() && await sendButtonBeforeReady.isEnabled()) {
      throw new Error('The send action was enabled before the Daemon became ready.')
    }

    await browser.waitUntil(async () => {
      return await browser.tauri.execute(({ core }) => core.invoke('ping_daemon')) === true
    }, {
      timeout: 90_000,
      interval: 500,
      timeoutMsg: 'The real Tauri command did not reach a ready nookd daemon',
    })

    await browser.waitUntil(async () => await input.isEnabled(), {
      timeout: 10_000,
      interval: 250,
      timeoutMsg: 'The Panel kept the input disabled after Daemon readiness.',
    })
  })
})
