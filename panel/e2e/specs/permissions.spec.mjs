describe('native permissions: permanent grants and tool preferences', () => {
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
  }

  const newChat = async () => {
    await $('[aria-label="Start new chat"]').click()
    await browser.waitUntil(async () => (await $$('[data-message-role="user"]')).length === 0, { timeout: 10_000 })
  }

  const openPermissions = async () => {
    await $('[aria-label="Toggle settings modal"]').click()
    await $('button*=Permissions').click()
    await $('[data-testid="permissions-tab"]').waitForDisplayed({ timeout: 10_000 })
  }

  const closeSettings = async () => {
    await $('[aria-label="Close settings"]').click()
    await $('[data-testid="permissions-tab"]').waitForDisplayed({ reverse: true, timeout: 10_000 })
  }

  const grantRows = () => $$('[data-testid="grant-row"]')

  it('keeps an always-allowed host across chats and lists it in Settings', async () => {
    await send('E2E_APPROVAL:https://perm.example.test/a')
    const card = await waitForApproval()
    if (!(await card.$('[data-testid="approval-action-descriptions"]').getText()).includes('until you revoke it in Settings')) {
      throw new Error('The card did not explain that the grant lasts until it is revoked.')
    }
    await card.$('button=Always allow this host').click()
    await waitForLatestComplete('Approved deterministic fetch')

    await newChat()
    await send('E2E_APPROVAL:https://perm.example.test/b')
    await waitForLatestComplete('Approved deterministic fetch')
    if (await $('[data-testid="approval-card"]').isExisting()) {
      throw new Error('A permanent grant still asked in a new chat.')
    }

    await openPermissions()
    await browser.waitUntil(async () => (await grantRows()).length === 1, {
      timeout: 10_000,
      timeoutMsg: 'The permanent grant never appeared in the list.',
    })
    const text = await (await grantRows())[0].getText()
    if (!text.includes('Web fetch') || !text.includes('perm.example.test')) {
      throw new Error(`The grant row did not name the tool and host: ${text}`)
    }
    await closeSettings()
  })

  it('asks again after the grant is revoked', async () => {
    await openPermissions()
    await $('button*=Revoke').click()
    await browser.waitUntil(async () => (await grantRows()).length === 0, {
      timeout: 10_000,
      timeoutMsg: 'The grant row stayed after Revoke.',
    })
    if (!(await $('[data-testid="permissions-tab"]').getText()).includes('No permanent permissions yet.')) {
      throw new Error('The empty state did not show.')
    }
    await closeSettings()

    await newChat()
    await send('E2E_APPROVAL:https://perm.example.test/c')
    await (await waitForApproval()).$('button=Deny').click()
    await waitForLatestComplete('Approval denied')
  })

  it('stops a tool the user set to Never allow, and restores it with Default', async () => {
    await openPermissions()
    await $('[aria-label="Approval for Web search"]').$('button=Never allow').click()
    await browser.waitUntil(async () =>
      (await $('[data-testid="permissions-tab"]').getText()).includes('Nook never runs this tool.'),
    { timeout: 10_000, timeoutMsg: 'The effect text did not update.' })
    await closeSettings()

    await newChat()
    await send('E2E_SEARCH:cats')
    await waitForLatestComplete('Approval denied for cats: The tool is denied by policy')

    await openPermissions()
    await $('[aria-label="Approval for Web search"]').$('button=Default').click()
    await closeSettings()
    await newChat()
    await send('E2E_SEARCH:dogs')
    await waitForLatestComplete('Approved deterministic fetch for dogs')
  })

  it('turns an automatic tool into a prompt with Ask every time', async () => {
    await openPermissions()
    await $('[aria-label="Approval for Web search"]').$('button=Ask every time').click()
    await browser.waitUntil(async () =>
      (await $('[data-testid="permissions-tab"]').getText()).includes('asks before it uses this tool'),
    { timeout: 10_000 })
    await closeSettings()

    await newChat()
    await send('E2E_SEARCH:birds')
    const card = await waitForApproval()
    await card.$('button=Allow for this chat').click()
    await waitForLatestComplete('Approved deterministic fetch for birds')

    await openPermissions()
    await $('[aria-label="Approval for Web search"]').$('button=Default').click()
    await closeSettings()
  })

  it('offers no looser choice for a tool that already asks', async () => {
    await openPermissions()
    const group = await $('[aria-label="Approval for Web fetch"]')
    const buttons = await group.$$('button')
    const labels = []
    for (const button of buttons) labels.push(await button.getText())
    if (labels.join('|') !== 'Default|Never allow') {
      throw new Error(`Web fetch offered: ${labels.join(', ')}`)
    }
    await closeSettings()
  })
})
