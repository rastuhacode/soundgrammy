import { readFileSync, writeFileSync } from 'node:fs'

function readJson(path: string): Record<string, unknown> {
  return JSON.parse(readFileSync(path, 'utf8')) as Record<string, unknown>
}

function requireString(value: unknown, source: string): string {
  if (typeof value !== 'string' || value.length === 0) {
    throw new Error(`Missing version in ${source}`)
  }
  return value
}

const packageVersion = requireString(readJson('package.json').version, 'package.json')
const tauriVersion = requireString(
  readJson('src-tauri/tauri.conf.json').version,
  'src-tauri/tauri.conf.json',
)

const cargoToml = readFileSync('src-tauri/Cargo.toml', 'utf8')
const cargoPackage = cargoToml.match(/^\[package\]\s*$([\s\S]*?)(?=^\[)/m)?.[1]
const cargoVersion = cargoPackage?.match(/^version\s*=\s*"([^"]+)"\s*$/m)?.[1]

const cargoLock = readFileSync('src-tauri/Cargo.lock', 'utf8')
const cargoLockVersion = cargoLock.match(
  /^\[\[package\]\]\s*\nname\s*=\s*"soundgrammy"\s*\nversion\s*=\s*"([^"]+)"/m,
)?.[1]

const versions = {
  'package.json': packageVersion,
  'src-tauri/tauri.conf.json': tauriVersion,
  'src-tauri/Cargo.toml': cargoVersion,
  'src-tauri/Cargo.lock': cargoLockVersion,
}

const expectedVersion = process.env.EXPECTED_VERSION?.replace(/^v/, '')
const expected = expectedVersion ?? packageVersion
const mismatches = Object.entries(versions).filter(([, version]) => version !== expected)

if (mismatches.length > 0) {
  console.error(`Expected every application version to be ${expected}:`)
  for (const [source, version] of Object.entries(versions)) {
    console.error(`  ${source}: ${version ?? 'missing'}`)
  }
  process.exit(1)
}

console.log(`Application versions are synchronized at ${expected}.`)

const tauriDevConfigPath = 'src-tauri/tauri.dev.conf.json'
const tauriDevConfig = readJson(tauriDevConfigPath)
const expectedDevVersion = `${expected}-develop`

if (tauriDevConfig.version !== expectedDevVersion) {
  tauriDevConfig.version = expectedDevVersion
  writeFileSync(tauriDevConfigPath, `${JSON.stringify(tauriDevConfig, null, 2)}\n`)
  console.log(`Updated ${tauriDevConfigPath} to ${expectedDevVersion}.`)
}
