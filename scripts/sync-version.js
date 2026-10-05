#!/usr/bin/env node

const fs = require('node:fs')
const path = require('node:path')

const rootDir = path.resolve(__dirname, '..')
const rawTag = process.argv[2]

if (!rawTag) {
  console.error('Error: Version or tag argument required (e.g. v0.1.0 or 0.1.0)')
  process.exit(1)
}

const version = rawTag.startsWith('v') ? rawTag.slice(1) : rawTag

function isValidSemver(v) {
  const [main, ...preParts] = v.split('-')
  const pre = preParts.join('-')
  const [core] = (preParts.length > 0 ? pre : main).split('+')
  const base = preParts.length > 0 ? main : core
  const parts = base.split('.')
  if (parts.length !== 3) return false
  for (const p of parts) {
    if (!/^\d+$/.test(p)) return false
    if (p.length > 1 && p.startsWith('0')) return false
  }
  return true
}

if (!isValidSemver(version)) {
  console.error(`Error: Invalid semver version '${version}'`)
  process.exit(1)
}

// 1. Update panel/src-tauri/tauri.conf.json
const tauriConfPath = path.join(rootDir, 'panel', 'src-tauri', 'tauri.conf.json')
if (fs.existsSync(tauriConfPath)) {
  const tauriConf = JSON.parse(fs.readFileSync(tauriConfPath, 'utf8'))
  tauriConf.version = version
  fs.writeFileSync(tauriConfPath, JSON.stringify(tauriConf, null, 2) + '\n')
  console.log(`Updated ${tauriConfPath} version to ${version}`)
}

// 2. Update panel/package.json
const pkgJsonPath = path.join(rootDir, 'panel', 'package.json')
if (fs.existsSync(pkgJsonPath)) {
  const pkg = JSON.parse(fs.readFileSync(pkgJsonPath, 'utf8'))
  pkg.version = version
  fs.writeFileSync(pkgJsonPath, JSON.stringify(pkg, null, 2) + '\n')
  console.log(`Updated ${pkgJsonPath} version to ${version}`)
}

// 3. Update panel/src-tauri/Cargo.toml
const cargoTomlPath = path.join(rootDir, 'panel', 'src-tauri', 'Cargo.toml')
if (fs.existsSync(cargoTomlPath)) {
  let cargoToml = fs.readFileSync(cargoTomlPath, 'utf8')
  cargoToml = cargoToml.replace(/^version\s*=\s*"[^"]+"/m, `version = "${version}"`)
  fs.writeFileSync(cargoTomlPath, cargoToml)
  console.log(`Updated ${cargoTomlPath} version to ${version}`)
}

console.log(`Version successfully synchronized to ${version}`)
