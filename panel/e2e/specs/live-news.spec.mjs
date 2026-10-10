describe('live Worker through the native Panel', () => {
  it('answers one India news query with current source links', async () => {
    await browser.tauri.switchWindow('main')
    await browser.waitUntil(async () =>
      await browser.tauri.execute(({ core }) => core.invoke('ping_daemon')) === true,
    {
      timeout: 90_000,
      interval: 500,
      timeoutMsg: 'nookd did not become ready before the live query.',
    })

    const question = 'What are the top news headlines in India today? Include links to your sources.'
    const input = await $('[aria-label="Ask anything"]')
    await browser.waitUntil(async () => await input.isDisplayed() && await input.isEnabled(), {
      timeout: 10_000,
      interval: 250,
      timeoutMsg: 'The Panel input did not enable after Daemon readiness.',
    })
    await input.setValue(question)
    await $('[aria-label="Send question"]').click()

    const userMessage = await $(`[data-message-role="user"]*=${question}`)
    await userMessage.waitForDisplayed({ timeout: 10_000 })
    await browser.waitUntil(async () => (await $$('[data-message-role="assistant"]')).length > 0, {
      timeout: 10_000,
      timeoutMsg: 'The Panel did not create an assistant message.',
    })
    const assistants = await $$('[data-message-role="assistant"]')
    const assistant = assistants[assistants.length - 1]

    const deadline = Date.now() + 240_000
    let approvalCount = 0
    while (Date.now() < deadline) {
      const status = await assistant.getAttribute('data-message-status')
      if (status === 'complete') break
      if (status === 'error' || status === 'cancelled') {
        throw new Error(`The live Worker ended with ${status}: ${await assistant.getText()}`)
      }

      const approval = await assistant.$('[data-testid="approval-card"]')
      if (await approval.isDisplayed().catch(() => false)) {
        await (await approval.$('button=Allow once')).click()
        approvalCount += 1
        await browser.waitUntil(async () => {
          const currentStatus = await assistant.getAttribute('data-message-status')
          const currentApproval = await assistant.$('[data-testid="approval-card"]')
          return ['complete', 'error', 'cancelled'].includes(currentStatus)
            || !(await currentApproval.isDisplayed().catch(() => false))
        }, {
          timeout: 30_000,
          interval: 250,
          timeoutMsg: `Live approval ${approvalCount} did not clear after Allow once.`,
        })
      } else {
        await browser.pause(250)
      }
    }

    const status = await assistant.getAttribute('data-message-status')
    const answer = await assistant.getText()
    if (status !== 'complete') {
      throw new Error(`Expected a completed sourced news answer, got ${status}: ${answer}`)
    }
    if (answer.trim().length < 80 || /model produced no text response|failed to connect to daemon/i.test(answer)) {
      throw new Error(`The live response did not contain a usable news summary: ${answer}`)
    }
    if ((await assistant.$$('a[href^="http"]')).length === 0) {
      throw new Error(`The completed assistant answer has no source links: ${answer}`)
    }
    if (approvalCount === 0) throw new Error('The live query completed without requesting web approval.')
    if (!(await userMessage.getText()).includes(question)) {
      throw new Error('The submitted question is missing from the transcript.')
    }
  })
})
