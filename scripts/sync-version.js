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
  if (!v || typeof v !== 'string') return false

  const [mainAndPre, ...buildParts] = v.split('+')
  if (buildParts.length > 1) return false
  if (buildParts.length === 1) {
    const build = buildParts[0]
    if (!build) return false
    const buildIds = build.split('.')
    for (const id of buildIds) {
      if (!id || !/^[0-9A-Za-z-]+$/.test(id)) return false
    }
  }

  const [core, ...preParts] = mainAndPre.split('-')
  if (preParts.length > 0) {
    const pre = preParts.join('-')
    if (!pre) return false
    const preIds = pre.split('.')
    for (const id of preIds) {
      if (!id || !/^[0-9A-Za-z-]+$/.test(id)) return false
      if (/^\d+$/.test(id) && id.length > 1 && id.startsWith('0')) return false
    }
  }

  const parts = core.split('.')
  if (parts.length !== 3) return false
  for (const p of parts) {
    if (!p || !/^\d+$/.test(p)) return false
    if (p.length > 1 && p.startsWith('0')) return false
  }
  return true
}

if (!isValidSemver(version)) {
  console.error(`Error: Invalid semver version '${version}'`)
  process.exit(1)
}

// Target paths
const tauriConfPath = path.join(rootDir, 'panel', 'src-tauri', 'tauri.conf.json')
const pkgJsonPath = path.join(rootDir, 'panel', 'package.json')
const cargoTomlPath = path.join(rootDir, 'panel', 'src-tauri', 'Cargo.toml')

// Validate all targets exist upfront before writing anything
for (const targetPath of [tauriConfPath, pkgJsonPath, cargoTomlPath]) {
  if (!fs.existsSync(targetPath)) {
    console.error(`Error: Required version target does not exist: ${targetPath}`)
    process.exit(1)
  }
}

// Validate Cargo.toml contains the version field before writing
const cargoTomlContent = fs.readFileSync(cargoTomlPath, 'utf8')
const cargoVersionPattern = /^version\s*=\s*"[^"]+"/m
if (!cargoVersionPattern.test(cargoTomlContent)) {
  console.error(`Error: Could not locate package version field in ${cargoTomlPath}`)
  process.exit(1)
}

// 1. Update panel/src-tauri/tauri.conf.json
const tauriConf = JSON.parse(fs.readFileSync(tauriConfPath, 'utf8'))
tauriConf.version = version
fs.writeFileSync(tauriConfPath, JSON.stringify(tauriConf, null, 2) + '\n')
console.log(`Updated ${tauriConfPath} version to ${version}`)

// 2. Update panel/package.json
const pkg = JSON.parse(fs.readFileSync(pkgJsonPath, 'utf8'))
pkg.version = version
fs.writeFileSync(pkgJsonPath, JSON.stringify(pkg, null, 2) + '\n')
console.log(`Updated ${pkgJsonPath} version to ${version}`)

// 3. Update panel/src-tauri/Cargo.toml
const updatedCargoToml = cargoTomlContent.replace(cargoVersionPattern, `version = "${version}"`)
fs.writeFileSync(cargoTomlPath, updatedCargoToml)
console.log(`Updated ${cargoTomlPath} version to ${version}`)

console.log(`Version successfully synchronized to ${version}`)
