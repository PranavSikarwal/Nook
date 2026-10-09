import { spawn } from 'node:child_process'
import { createServer } from 'node:net'
import { chmod, mkdir, mkdtemp, open, readFile, readdir, rm, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { randomUUID } from 'node:crypto'
import { fileURLToPath } from 'node:url'
import { Client } from 'pg'

const panelDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const repoDir = path.resolve(panelDir, '..')
const daemonDir = path.join(repoDir, 'daemon')
const mode = process.argv[2]
const isLive = mode === 'live'
const isDeterministic = mode === 'deterministic'
const runId = randomUUID().replaceAll('-', '').slice(0, 16)
const databaseName = `nook_e2e_${runId}`
const marker = `nook-e2e-owner-${runId}`
let tempRoot
let databaseCreated = false
let activeChild
let lockHandle

function required(name) {
  const value = process.env[name]?.trim()
  if (!value) throw new Error(`${name} is required for ${mode} E2E tests.`)
  return value
}

function run(command, args, options = {}) {
  return new Promise((resolve, reject) => {
    const proc = spawn(command, args, { stdio: 'inherit', ...options })
    activeChild = proc
    proc.once('error', reject)
    proc.once('exit', (code, signal) => {
      if (activeChild === proc) activeChild = undefined
      if (code === 0) resolve()
      else reject(new Error(`${command} exited with ${code ?? signal}`))
    })
  })
}

async function acquireRunLock() {
  const lockPath = path.join(os.tmpdir(), 'nook-native-e2e.lock')
  try {
    lockHandle = await open(lockPath, 'wx')
    await lockHandle.writeFile(JSON.stringify({ pid: process.pid, runId }))
  } catch (error) {
    if (error.code !== 'EEXIST') throw error
    let lock
    try {
      lock = JSON.parse(await readFile(lockPath, 'utf8'))
    } catch {
      throw new Error('A native E2E run lock exists but cannot be read. Remove it only after confirming no E2E processes remain.')
    }
    try {
      process.kill(lock.pid, 0)
      throw new Error('Another native E2E run holds the lock. Wait for it to exit before starting another.')
    } catch (processError) {
      if (processError.code !== 'ESRCH') throw processError
    }
    await rm(lockPath, { force: true })
    return await acquireRunLock()
  }
}

async function releaseRunLock() {
  if (!lockHandle) return
  const lockPath = path.join(os.tmpdir(), 'nook-native-e2e.lock')
  await lockHandle.close().catch(() => {})
  lockHandle = undefined
  try {
    const lock = JSON.parse(await readFile(lockPath, 'utf8'))
    if (lock.pid === process.pid && lock.runId === runId) {
      await rm(lockPath, { force: true })
    }
  } catch {
    return
  }
}

async function stopActiveChild(signal) {
  if (activeChild?.exitCode !== null) return
  const child = activeChild
  child.kill(signal)
  await new Promise((resolve) => child.once('exit', resolve))
  if (activeChild === child) activeChild = undefined
}

async function createOwnedDatabase(adminUrl) {
  const admin = new Client({ connectionString: adminUrl })
  await admin.connect()
  try {
    const databaseUrl = new URL(adminUrl)
    databaseUrl.pathname = `/${databaseName}`
    await admin.query(`CREATE DATABASE "${databaseName}"`)
    databaseCreated = true
    try {
      const owner = new Client({ connectionString: databaseUrl.toString() })
      await owner.connect()
      try {
        await owner.query('CREATE SCHEMA e2e')
        await owner.query('CREATE TABLE e2e.run_owner (run_id text PRIMARY KEY)')
        await owner.query('INSERT INTO e2e.run_owner (run_id) VALUES ($1)', [marker])
      } finally {
        await owner.end()
      }
    } catch (error) {
      await admin.query(`DROP DATABASE "${databaseName}" WITH (FORCE)`)
      databaseCreated = false
      throw error
    }
    return databaseUrl.toString()
  } finally {
    await admin.end()
  }
}

async function dropOwnedDatabase(adminUrl) {
  if (!databaseCreated) return
  const admin = new Client({ connectionString: adminUrl })
  await admin.connect()
  try {
    const databaseUrl = new URL(adminUrl)
    databaseUrl.pathname = `/${databaseName}`
    const owner = new Client({ connectionString: databaseUrl.toString() })
    await owner.connect()
    let owned = false
    try {
      const result = await owner.query('SELECT run_id FROM e2e.run_owner')
      owned = result.rows.length === 1 && result.rows[0].run_id === marker
    } finally {
      await owner.end()
    }
    if (!owned) throw new Error(`Refusing to drop unowned database ${databaseName}.`)
    await admin.query(`DROP DATABASE "${databaseName}" WITH (FORCE)`)
    databaseCreated = false
  } finally {
    await admin.end()
  }
}

async function hasWorkerEnvironmentWarning(directory) {
  const entries = await readdir(directory, { withFileTypes: true })
  for (const entry of entries) {
    const entryPath = path.join(directory, entry.name)
    if (entry.isDirectory()) {
      if (await hasWorkerEnvironmentWarning(entryPath)) return true
    } else if (entry.isFile() && /\.log$/i.test(entry.name)) {
      const contents = await readFile(entryPath, 'utf8')
      if (/VIRTUAL_ENV[^\n]*(?:does not match|mismatch|differs)|(?:virtual environment|project environment)[^\n]*(?:does not match|mismatch|differs)/i.test(contents)) {
        return true
      }
    }
  }
  return false
}

async function ensurePortAvailable() {
  const port = Number(process.env.NOOK_E2E_WDIO_PORT ?? 4444)
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    throw new Error('NOOK_E2E_WDIO_PORT must be an integer between 1 and 65535.')
  }
  const server = createServer()
  await new Promise((resolve, reject) => {
    server.once('error', reject)
    server.listen(port, '127.0.0.1', resolve)
  })
  await new Promise((resolve, reject) => server.close((error) => error ? reject(error) : resolve()))
  return port
}

async function main() {
  if (!['smoke', 'deterministic', 'live'].includes(mode)) {
    throw new Error('Usage: npm run test:e2e:smoke, test:e2e:deterministic, or test:e2e:live')
  }
  const adminUrl = required('NOOK_E2E_DATABASE_URL')
  const parsedAdminUrl = new URL(adminUrl)
  if (!['postgres:', 'postgresql:'].includes(parsedAdminUrl.protocol)) {
    throw new Error('NOOK_E2E_DATABASE_URL must use PostgreSQL.')
  }
  if (!/^nook_e2e_[a-f0-9]{16}$/.test(databaseName)) {
    throw new Error('Refusing to create a database outside the expected nook_e2e_ name pattern.')
  }
  if (isLive) {
    required('NOOK_BASE_URL')
    required('NOOK_MODEL')
    required('NOOK_API_KEY')
  } else if (process.env.NOOK_API_KEY) {
    throw new Error('Unset NOOK_API_KEY for smoke tests. Live credentials are never passed to smoke runs.')
  }
  const defaultAppDir = path.resolve(os.homedir(), 'Library/Application Support/Nook')
  if (path.resolve(panelDir) === defaultAppDir || path.resolve(panelDir).startsWith(defaultAppDir + path.sep)) {
    throw new Error('The E2E runner cannot run from inside Nook\'s default app-data directory.')
  }

  await acquireRunLock()
  try {
    const port = await ensurePortAvailable()
    tempRoot = await mkdtemp(path.join('/tmp', `ne${runId}-`))
    const appDir = path.join(tempRoot, 'app-data')
    await mkdir(appDir, { recursive: false })
    const databaseUrl = await createOwnedDatabase(adminUrl)
    const manifestPath = path.join(tempRoot, 'run.json')
    await writeFile(manifestPath, JSON.stringify({ runId, databaseName, appDir, mode, port }, null, 2))
    const baseUrl = isLive ? required('NOOK_BASE_URL') : 'http://127.0.0.1:9/v1'
    const model = isLive ? required('NOOK_MODEL') : 'nook-e2e-no-network'
    const testConfig = [
      `base_url = ${JSON.stringify(baseUrl)}`,
      `model = ${JSON.stringify(model)}`,
      `database_url = ${JSON.stringify(databaseUrl)}`,
      'max_input_tokens = 1000000',
      'summarize_at_tokens = 750000',
      '',
    ].join('\n')
    await writeFile(path.join(appDir, 'config.toml'), testConfig)

    const daemonPath = path.join(daemonDir, 'target', 'debug', 'nookd')
    let testDaemonPath = daemonPath
    if (mode === 'smoke') {
      testDaemonPath = path.join(appDir, 'nookd-delayed')
      const quotedDaemonPath = daemonPath.replaceAll("'", "'\\''")
      const launcher = [
        '#!/bin/sh',
        'sleep 8',
        `exec '${quotedDaemonPath}'`,
        '',
      ].join('\n')
      await writeFile(testDaemonPath, launcher)
      await chmod(testDaemonPath, 0o755)
    }

    const inheritedEnv = { ...process.env }
    delete inheritedEnv.VIRTUAL_ENV
    const env = {
      ...inheritedEnv,
      NOOK_APP_DIR: appDir,
      NOOK_DATABASE_URL: databaseUrl,
      NOOK_DISABLE_KEYCHAIN: '1',
      NOOKD_PATH: testDaemonPath,
      ...(isDeterministic
        ? { NOOK_E2E_WORKER_COMMAND: `python3 ${path.join(panelDir, 'e2e/fake_worker.py')}` }
        : {}),
      NOOK_E2E_REAL_APP_BINARY: path.join(panelDir, 'src-tauri', 'target', 'debug', 'nook-panel'),
      NOOK_E2E_WDIO_PORT: String(port),
      NOOK_E2E_RUN_ID: runId,
      NOOK_E2E_RUN_DIR: tempRoot,
      NOOK_E2E_MODE: mode,
      NOOK_WORKER_DIR: path.resolve(repoDir, 'worker'),
    }
    if (!isLive) {
      env.NOOK_BASE_URL = 'http://127.0.0.1:9/v1'
      env.NOOK_MODEL = 'nook-e2e-no-network'
      env.NOOK_API_KEY = ''
    }

    try {
      await run('cargo', [
        'build', '-p', 'nookd',
        ...(isDeterministic ? ['--features', 'e2e'] : []),
      ], { cwd: daemonDir, env })
      await run('npm', ['exec', '--', 'tauri', 'build', '--debug', '--no-bundle', '--features', 'e2e', '--config', 'src-tauri/tauri.e2e.conf.json'], { cwd: panelDir, env })
      await run('npm', ['exec', '--', 'wdio', 'run', 'e2e/wdio.conf.mjs'], { cwd: panelDir, env })
      const workerEnvironmentWarning = await hasWorkerEnvironmentWarning(tempRoot)
      console.log(`Worker environment-path warning: ${workerEnvironmentWarning ? 'found' : 'not found in captured logs'}.`)
      await dropOwnedDatabase(adminUrl)
      await rm(tempRoot, { recursive: true, force: true })
    } catch (error) {
      console.error(`E2E run ${runId} failed. Run data was preserved at ${tempRoot}.`)
      throw error
    }
  } finally {
    await stopActiveChild('SIGTERM')
    await releaseRunLock()
  }
}

for (const signal of ['SIGINT', 'SIGTERM']) {
  process.once(signal, () => {
    void stopActiveChild(signal).finally(() => {
      process.exitCode = signal === 'SIGINT' ? 130 : 143
    })
  })
}

try {
  await main()
} catch (error) {
  console.error(error)
  process.exitCode = 1
}
