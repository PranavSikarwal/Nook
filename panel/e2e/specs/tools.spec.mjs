describe('native Settings: tool sources', () => {
  before(async () => {
    await browser.tauri.switchWindow('main')
    await browser.waitUntil(async () =>
      await browser.tauri.execute(({ core }) => core.invoke('ping_daemon')) === true,
    { timeout: 90_000, interval: 250 })
  })

  const openTools = async () => {
    await $('[aria-label="Toggle settings modal"]').click()
    await $('button*=Tools').click()
    await $('[data-testid="tools-tab"]').waitForDisplayed({ timeout: 10_000 })
    await browser.waitUntil(async () => (await $$('[data-testid="tool-source-row"]')).length > 0,
      { timeout: 10_000, interval: 100, timeoutMsg: 'No tool source was listed.' })
  }

  const row = (id) => $(`[data-testid="tool-source-row"][data-source-id="${id}"]`)

  it('lists the file tools, the command tool, and the running MCP server as ready', async () => {
    await openTools()

    for (const id of ['filesystem', 'terminal', 'mcp:e2e']) {
      const state = await row(id).getAttribute('data-source-state')
      if (state !== 'ready') throw new Error(`${id} is ${state}, not ready.`)
    }
    const text = await row('mcp:e2e').getText()
    if (!/\d+ tools/.test(text)) throw new Error(`The MCP row shows no tool count: ${text}`)
  })

  it('shows a server that did not start as off, with the reason', async () => {
    const broken = await row('mcp:broken')
    if (await broken.getAttribute('data-source-state') !== 'off') {
      throw new Error('The broken server is not shown as off.')
    }
    const reason = await broken.$('[data-testid="tool-source-reason"]').getText()
    if (reason.trim().length === 0) throw new Error('The broken server has no reason.')
    if (await row('mcp:e2e').$('[data-testid="tool-source-reason"]').isExisting()) {
      throw new Error('A ready server shows a reason.')
    }
  })

  it('reloads the list with Refresh', async () => {
    await $('button=Refresh').click()
    await browser.waitUntil(async () => (await $$('[data-testid="tool-source-row"]')).length === 4,
      { timeout: 10_000, interval: 100, timeoutMsg: 'The list did not hold four sources.' })
  })
})
