#!/usr/bin/env node
import { spawn } from 'node:child_process'

const [binary, ...args] = process.argv.slice(2)
const required = ['NOOK_APP_DIR', 'NOOK_DATABASE_URL', 'NOOK_DISABLE_KEYCHAIN']
for (const name of required) {
  if (!process.env[name]) throw new Error(`${name} must be set before launching Nook.`)
}

const child = spawn(binary, args, { env: process.env, stdio: 'inherit' })
let stopping = false

function stop(signal) {
  if (stopping || child.exitCode !== null) return
  stopping = true
  child.kill(signal)
}

for (const signal of ['SIGINT', 'SIGTERM']) {
  process.once(signal, () => stop(signal))
}

child.once('error', (error) => {
  console.error(error)
  process.exitCode = 1
})
child.once('exit', (code, signal) => {
  process.exitCode = code ?? (signal ? 1 : 0)
})
