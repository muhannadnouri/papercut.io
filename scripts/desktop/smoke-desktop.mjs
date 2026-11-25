import { execFileSync } from 'node:child_process'
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
} from 'node:fs'
import { tmpdir } from 'node:os'
import {
  join,
  resolve,
} from 'node:path'
import { fileURLToPath } from 'node:url'
import { parseArgs } from 'node:util'

const root = fileURLToPath(new URL('../../', import.meta.url))
process.chdir(root)
const linux = process.platform === 'linux'
const { values } = parseArgs({
  options: {
    artifact: { type: 'string' },
    case: { type: 'string' },
  },
})
const casesDir = fileURLToPath(new URL('./cases/', import.meta.url))
const cases = new Map()
for (const file of readdirSync(casesDir).sort()) {
  if (!file.endsWith('.mjs')) {
    continue
  }
  const id = /^(DS-\d{3})-[a-z0-9-]+\.mjs$/.exec(file)?.[1]
  if (!id || cases.has(id)) {
    throw Error('Invalid or duplicate desktop smoke case: ' + file)
  }
  cases.set(id, join(casesDir, file))
}
if (!cases.size) {
  throw Error('No desktop smoke cases found')
}
if (values.case !== undefined && !cases.has(values.case)) {
  throw Error('Unknown desktop smoke case ' + values.case + '. Available: ' + [...cases.keys()].join(', '))
}
if (!linux && process.platform !== 'win32') {
  throw Error('Desktop smoke supports Linux and Windows.')
}
if (!linux) {
  // Windows Known Folders and the single-instance mutex belong to the signed-in user.
  for (const base of [process.env.APPDATA, process.env.LOCALAPPDATA]) {
    if (!base || existsSync(join(base, 'io.papercut.desktop'))) {
      throw Error('Run Windows smoke in a fresh disposable Windows user account or hosted runner.')
    }
  }
  if (process.env.RUNNER_ENVIRONMENT !== 'github-hosted' && process.env.PAPERCUT_DISPOSABLE_USER !== '1') {
    throw Error('Use a disposable Windows account and set PAPERCUT_DISPOSABLE_USER=1.')
  }
}

const started = Date.now()
const git = (...args) => execFileSync('git', args, { encoding: 'utf8' }).trim()
console.log('Desktop smoke', git('rev-parse', 'HEAD'), git('status', '--porcelain') ? '(dirty)' : '(clean)', process.platform)
let artifact = values.artifact && resolve(values.artifact)
if (!artifact) {
  const release = resolve(process.env.CARGO_TARGET_DIR || 'src-tauri/target', 'release')
  const args = ['scripts/desktop/build-desktop.js']
  if (linux) {
    args.push('--bundles', 'appimage')
  }
  execFileSync(process.execPath, args, { stdio: 'inherit' })
  if (linux) {
    const dir = join(release, 'bundle/appimage')
    const { version } = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8'))
    const images = readdirSync(dir).filter(name => name.startsWith('Papercut_' + version + '_') && name.endsWith('.AppImage')
      && statSync(join(dir, name)).mtimeMs >= started - 1000)
    if (images.length !== 1) {
      throw Error('Expected one current-version AppImage in ' + dir)
    }
    artifact = join(dir, images[0])
  } else {
    artifact = join(release, 'app.exe')
  }
}
if (!existsSync(artifact)) {
  throw Error('Missing desktop artifact: ' + artifact)
}
const evidence = mkdtempSync(join(tmpdir(), 'papercut-smoke-report-'))
console.log('Artifact', artifact, 'Build/preflight ms', Date.now() - started, 'Report', evidence)

// Keep the two launches of a restart case together in their own disposable desktop.
function runCase(id, caseFile) {
  const report = join(evidence, id)
  mkdirSync(report)
  console.log(id, 'Report', report)
  const profile = mkdtempSync(join(tmpdir(), 'papercut-smoke-profile-'))
  const worker = fileURLToPath(new URL('./smoke-worker.mjs', import.meta.url))
  const env = {
    ...process.env,
    LANG: 'en_US.UTF-8',
  }
  try {
    if (linux) {
      for (const key of ['HOME', 'XDG_DATA_HOME', 'XDG_CACHE_HOME', 'XDG_CONFIG_HOME', 'XDG_STATE_HOME', 'XDG_RUNTIME_DIR']) {
        env[key] = join(profile, key)
        mkdirSync(env[key], { mode: 0o700 })
      }
      delete env.WAYLAND_DISPLAY
      delete env.DBUS_SESSION_BUS_ADDRESS
      env.GDK_BACKEND = 'x11'
      execFileSync(artifact, ['--appimage-extract'], {
        cwd: profile,
        env,
        stdio: ['ignore', 'ignore', 'inherit'],
      })
      execFileSync('dbus-run-session', ['--', 'xvfb-run', '-a', process.execPath, worker,
        join(profile, 'squashfs-root/AppRun'), report, caseFile], {
        env,
        stdio: 'inherit',
      })
    } else {
      env.WEBVIEW2_USER_DATA_FOLDER = join(profile, 'WebView2')
      execFileSync(process.execPath, [worker, artifact, report, caseFile], {
        env,
        stdio: 'inherit',
      })
    }
  } finally {
    rmSync(profile, {
      recursive: true,
      force: true,
    })
  }
}

for (const [id, file] of cases) {
  if (values.case === undefined || values.case === id) {
    runCase(id, file)
  }
}
console.log('Desktop smoke passed. Total ms', Date.now() - started)
