describe('native approval and grant scope', () => {
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

  const waitForLatestComplete = async (containing = '') => {
    await browser.waitUntil(async () => {
      const assistants = await $$('[data-message-role="assistant"]')
      if (assistants.length === 0) return false
      const latest = assistants.at(-1)
      return await latest.getAttribute('data-message-status') === 'complete' &&
        (await latest.getText()).includes(containing)
    }, { timeout: 20_000, interval: 100 })
    const assistants = await $$('[data-message-role="assistant"]')
    return assistants[assistants.length - 1]
  }

  const submitUrl = async (url) => {
    const prompt = `E2E_APPROVAL:${url}`
    await $('[aria-label="Ask anything"]').setValue(prompt)
    await $('[aria-label="Send question"]').click()
    await $(`[data-message-role="user"]*=${prompt}`).waitForDisplayed({ timeout: 10_000 })
  }

  it('denies a request and allows a later request once', async () => {
    await submitUrl('https://deny.example.test/page')
    await (await waitForApproval()).$('button=Deny').click()
    if (!(await (await waitForLatestComplete('Approval denied')).getText()).includes('Approval denied')) {
      throw new Error('Deny did not produce a denied result.')
    }

    await submitUrl('https://once.example.test/page')
    await (await waitForApproval()).$('button=Allow once').click()
    if (!(await (await waitForLatestComplete('Approved deterministic fetch')).getText()).includes('once.example.test')) {
      throw new Error('Allow once did not resume the requested action.')
    }

    await submitUrl('https://once.example.test/page')
    await waitForApproval()
    await (await $('[data-testid="approval-card"]')).$('button=Deny').click()
    await waitForLatestComplete('Approval denied')
  })

  it('limits host grants by chat and host', async () => {
    await submitUrl('https://host-grant.example.test/first')
    await (await waitForApproval()).$('button=Allow for this chat & host').click()
    await waitForLatestComplete('host-grant.example.test/first')
    await submitUrl('https://host-grant.example.test/second')
    const granted = await waitForLatestComplete('host-grant.example.test/second')
    if (!(await granted.getText()).includes('Approved deterministic fetch')) {
      throw new Error('The host grant did not apply to a second request for the same host.')
    }
    if (await $('[data-testid="approval-card"]').isExisting()) {
      throw new Error('An approval card appeared for a host that already had a grant.')
    }
    await submitUrl('https://different-host.example.test/page')
    await waitForApproval()
    await (await $('[data-testid="approval-card"]')).$('button=Deny').click()
    await waitForLatestComplete('Approval denied')
  })

  it('limits a host grant to the current chat and offers no chat-wide grant', async () => {
    await submitUrl('https://chat-grant.example.test/first')
    const card = await waitForApproval()
    if (await card.$('button=Allow for this chat').isExisting()) {
      throw new Error('The fetch approval offered a chat-wide grant that the policy does not list.')
    }
    await card.$('button=Allow for this chat & host').click()
    await waitForLatestComplete('chat-grant.example.test/first')

    await $('[aria-label="Start new chat"]').click()
    await browser.waitUntil(async () => (await $$('[data-message-role="user"]')).length === 0, { timeout: 10_000 })
    await submitUrl('https://chat-grant.example.test/new-chat')
    await waitForApproval()
    await (await $('[data-testid="approval-card"]')).$('button=Deny').click()
    await waitForLatestComplete('Approval denied')
  })

  it('keeps approval attached to the original message and blocks new chat', async () => {
    await submitUrl('https://pending-chat.example.test/page')
    const approval = await waitForApproval()
    if (await $('[aria-label="Start new chat"]').isEnabled()) {
      throw new Error('Start new chat remained enabled while approval was pending.')
    }
    if (!(await approval.isDisplayed())) throw new Error('The pending approval card disappeared.')
    await approval.$('button=Deny').click()
    await waitForLatestComplete('Approval denied')
  })

  it('keeps a second URL unsent while the first approval is pending', async () => {
    const first = 'https://pending-url.example.test/first'
    await submitUrl(first)
    await waitForApproval()
    await $('[aria-label="Ask anything"]').setValue('https://pending-url.example.test/second')
    if (await $('[aria-label="Send question"]').isExisting()) {
      throw new Error('Send appeared while the first request awaited approval.')
    }
    const secondUrlMessage = await $(`[data-message-role="user"]*=${'pending-url.example.test/second'}`)
    if (await secondUrlMessage.isExisting()) {
      throw new Error('The second URL was submitted while another request was active.')
    }
    await $('[aria-label="Stop generation"]').click()
  })
})
