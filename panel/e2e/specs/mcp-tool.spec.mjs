describe('native MCP tools run in the Daemon sandbox', () => {
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

  it('asks before an MCP tool runs and returns its answer', async () => {
    await send('E2E_MCP:echo|from mcp')
    const card = await waitForApproval()
    const text = await card.getText()
    if (!text.includes('Call echo on MCP server e2e')) {
      throw new Error(`The approval did not name the server and tool: ${text}`)
    }
    await card.$('button=Allow once').click()
    const reply = await waitForLatestComplete('Tool ok: echo: from mcp')
    const line = await reply.$('[data-testid="tool-call"]').getText()
    if (!line.includes('e2e echo')) throw new Error(`The tool call line was: ${line}`)
  })

  it('shows an error answer from the server without ending the chat', async () => {
    await send('E2E_MCP:fail')
    await (await waitForApproval()).$('button=Allow once').click()
    await waitForLatestComplete('Tool error: Error: something broke')
  })

  it('does not ask again for the same tool after Allow this tool for this chat', async () => {
    await $('[aria-label="Start new chat"]').click()
    await browser.waitUntil(async () => (await $$('[data-message-role="user"]')).length === 0, { timeout: 10_000 })

    await send('E2E_MCP:echo|one')
    await (await waitForApproval()).$('button=Allow this tool for this chat').click()
    await waitForLatestComplete('Tool ok: echo: one')

    await send('E2E_MCP:echo|two')
    await waitForLatestComplete('Tool ok: echo: two')
    if (await $('[data-testid="approval-card"]').isExisting()) {
      throw new Error('An approval card appeared for a tool that already had a chat grant.')
    }

    await send('E2E_MCP:fail')
    await (await waitForApproval()).$('button=Deny').click()
    await waitForLatestComplete('Tool denied: Denied by the user')
  })

  it('refuses arguments that break the tool schema before any prompt', async () => {
    await send('E2E_MCP:echo|{}')
    await waitForLatestComplete('Tool denied: Invalid arguments')
    if (await $('[data-testid="approval-card"]').isExisting()) {
      throw new Error('An approval card appeared for invalid arguments.')
    }
  })
})
