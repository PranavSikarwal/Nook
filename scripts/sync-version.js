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

function isValidCorePart(part) {
  if (!part || !/^\d+$/.test(part)) return false
  return !(part.length > 1 && part.startsWith('0'))
}

function isValidCore(core) {
  const parts = core.split('.')
  return parts.length === 3 && parts.every(isValidCorePart)
}

function isValidPrereleaseId(id) {
  if (!id || !/^[0-9A-Za-z-]+$/.test(id)) return false
  return !(/^\d+$/.test(id) && id.length > 1 && id.startsWith('0'))
}

function isValidPrerelease(pre) {
  if (!pre) return false
  return pre.split('.').every(isValidPrereleaseId)
}

function isValidBuild(build) {
  if (!build) return false
  return build.split('.').every((id) => Boolean(id) && /^[0-9A-Za-z-]+$/.test(id))
}

function isValidSemver(v) {
  if (!v || typeof v !== 'string') return false

  const [mainAndPre, ...buildParts] = v.split('+')
  if (buildParts.length > 1) return false
  if (buildParts.length === 1 && !isValidBuild(buildParts[0])) return false

  const [core, ...preParts] = mainAndPre.split('-')
  if (preParts.length > 0 && !isValidPrerelease(preParts.join('-'))) return false

  return isValidCore(core)
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
