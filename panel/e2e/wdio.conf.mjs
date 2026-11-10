import path from 'node:path'
import { fileURLToPath } from 'node:url'

const panelDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const runDir = process.env.NOOK_E2E_RUN_DIR
if (!runDir) throw new Error('NOOK_E2E_RUN_DIR is required.')
const appBinaryPath = process.execPath
const specsByMode = {
  smoke: ['e2e/specs/smoke.spec.mjs'],
  deterministic: [
    'e2e/specs/deterministic.spec.mjs',
    'e2e/specs/approval-grants.spec.mjs',
    'e2e/specs/file-tools.spec.mjs',
    'e2e/specs/terminal-tool.spec.mjs',
    'e2e/specs/mcp-tool.spec.mjs',
    'e2e/specs/permissions.spec.mjs',
    'e2e/specs/history-settings.spec.mjs',
    'e2e/specs/links.spec.mjs',
    'e2e/specs/tools.spec.mjs',
    'e2e/specs/folders.spec.mjs',
  ],
  live: process.env.NOOK_E2E_LIVE_TOOLS === '1'
    ? ['e2e/specs/live-tools.spec.mjs']
    : ['e2e/specs/live-news.spec.mjs', 'e2e/specs/live-fetch.spec.mjs'],
}
const specs = specsByMode[process.env.NOOK_E2E_MODE]
const appArgs = [
  path.join(panelDir, 'e2e/isolated-launcher.mjs'),
  process.env.NOOK_E2E_REAL_APP_BINARY ?? '',
]

export const config = {
  runner: 'local',
  specs: specs.map((spec) => path.join(panelDir, spec)),
  maxInstances: 1,
  logLevel: 'info',
  outputDir: runDir,
  bail: 1,
  baseUrl: 'http://localhost',
  waitforTimeout: 10_000,
  connectionRetryTimeout: 60_000,
  connectionRetryCount: 1,

  services: [
    [
      '@wdio/tauri-service',
      {
        appBinaryPath,
        appArgs,
        driverProvider: 'embedded',
        startTimeout: 60_000,
        tauriDriverPort: Number(process.env.NOOK_E2E_WDIO_PORT ?? 4444),
        captureBackendLogs: true,
        captureFrontendLogs: true,
        backendLogLevel: 'debug',
        frontendLogLevel: 'warn',
        windowLabel: 'main',
        logDir: runDir,
      },
    ],
  ],
  capabilities: [
    {
      browserName: 'tauri',
      'wdio:tauriServiceOptions': {
        windowLabel: 'main',
      },
      'tauri:options': {
        application: appBinaryPath,
        args: appArgs,
      },
    },
  ],
  framework: 'mocha',
  reporters: ['spec'],
  mochaOpts: {
    ui: 'bdd',
    timeout: 480_000,
  },
  async before() {
    if (process.env.NOOK_E2E_MODE === 'smoke') return
    await browser.tauri.switchWindow('main')
    await browser.waitUntil(async () => {
      const input = await $('[aria-label="Ask anything"]')
      return await input.isExisting() && await input.isEnabled()
    }, { timeout: 90_000, interval: 100, timeoutMsg: 'The Panel did not enable its input.' })
  },
  onPrepare() {
    if (!process.env.NOOK_E2E_REAL_APP_BINARY) {
      throw new Error('NOOK_E2E_REAL_APP_BINARY must point to the E2E-built Nook binary.')
    }
    const allowedRoot = path.resolve(process.env.NOOK_E2E_RUN_DIR ?? '')
    const appDir = path.resolve(process.env.NOOK_APP_DIR ?? '')
    if (!process.env.NOOK_E2E_RUN_DIR || !process.env.NOOK_APP_DIR || !appDir.startsWith(allowedRoot + path.sep)) {
      throw new Error('NOOK_APP_DIR must be inside this run\'s temporary directory.')
    }
    let database
    try {
      database = new URL(process.env.NOOK_DATABASE_URL ?? '').pathname.slice(1)
    } catch {
      throw new Error('NOOK_DATABASE_URL must be a valid database URL.')
    }
    if (!/^nook_e2e_[a-f0-9]+$/.test(database)) {
      throw new Error('NOOK_DATABASE_URL must name a runner-owned nook_e2e_ database.')
    }
    if (process.env.NOOK_DISABLE_KEYCHAIN !== '1') {
      throw new Error('NOOK_DISABLE_KEYCHAIN=1 is required for isolated tests.')
    }
  },
}
