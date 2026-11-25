import {
  execFileSync,
  spawnSync,
} from 'node:child_process'
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  statSync,
} from 'node:fs'
import { setTimeout as delay } from 'node:timers/promises'
import { tmpdir } from 'node:os'
import {
  join,
  resolve,
} from 'node:path'
import { fileURLToPath } from 'node:url'
import { parseArgs } from 'node:util'

import { withCleanup } from './smoke-cleanup.mjs'

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

/** The document portal can briefly outlive D-Bus shutdown. Never descend into its FUSE mount. */
async function removeProfile(profile) {
  if (linux) {
    for (let attempt = 0; ; attempt++) {
      const result = spawnSync('rm', ['--one-file-system', '-rf', '--', profile], { encoding: 'utf8' })
      if (result.error) {
        throw result.error
      }
      if (result.status === 0 && !existsSync(profile)) {
        return
      }
      if (attempt === 20) {
        throw Error('Unable to remove smoke profile ' + profile + ': ' + result.stderr)
      }
      await delay(250)
    }
  }
  rmSync(profile, {
    recursive: true,
    force: true,
    maxRetries: 5,
    retryDelay: 200,
  })
}

// Keep the two launches of a restart case together in their own disposable desktop.
async function runCase(id, caseFile) {
  const report = join(evidence, id)
  mkdirSync(report)
  console.log(id, 'Report', report)
  const profile = mkdtempSync(join(tmpdir(), 'papercut-smoke-profile-'))
  const worker = fileURLToPath(new URL('./smoke-worker.mjs', import.meta.url))
  const env = {
    ...process.env,
    LANG: 'en_US.UTF-8',
  }
  await withCleanup(id + ' remove profile', async () => {
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
  }, () => removeProfile(profile))
}

// Known Folders ignore the WebView2 profile. Move stores from the previous completed
// case aside in the disposable account, on the same volume, before the next launch.
function archiveWindowsCaseStores(id) {
  for (const base of new Set([process.env.APPDATA, process.env.LOCALAPPDATA])) {
    const store = join(base, 'io.papercut.desktop')
    if (existsSync(store)) {
      const archive = mkdtempSync(join(base, 'papercut-smoke-' + id + '-'))
      renameSync(store, join(archive, 'io.papercut.desktop'))
    }
  }
}

let previousCase
for (const [id, file] of cases) {
  if (['DS-001', 'DS-005'].includes(id)) {
    if (!linux && previousCase) {
      archiveWindowsCaseStores(previousCase)
    }
    try {
      await runCase(id, file)
    } catch (error) {
      console.error(error)
      process.exitCode = 1
    }
    previousCase = id
  }
}
console.log('Partial Top rehearsal finished. Total ms', Date.now() - started)
