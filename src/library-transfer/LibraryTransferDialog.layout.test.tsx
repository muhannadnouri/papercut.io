// @vitest-environment node
import { execFileSync } from 'node:child_process'
import {
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'
import { renderToStaticMarkup } from 'react-dom/server'
import {
  describe,
  expect,
  it,
  vi,
} from 'vitest'
import { LibraryTransferDialog } from './LibraryTransferDialog'
import en from '../i18n/locales/en.json'
import { NearbySessions } from './NearbySessions'

function translate(key: string) {
  let message: unknown = en
  for (const part of key.split('.')) {
    if (!message || typeof message !== 'object') {
      return key
    }
    message = (message as Record<string, unknown>)[part]
  }
  return typeof message === 'string' ? message : key
}
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: translate,
    i18n: {
      language: 'en',
      getFixedT: () => translate,
    },
  }),
}))
vi.mock('../i18n', () => ({
  currentAppLocale: () => 'en',
  default: {},
}))

const browser = [process.env.CHROME_BIN, 'chromium', 'chromium-browser', 'google-chrome', 'google-chrome-stable']
  .find(candidate => {
    if (!candidate) {
      return false
    }
    try {
      execFileSync(candidate, ['--version'], {
        stdio: 'ignore',
        timeout: 5000,
      }); return true
    } catch {
      return false
    }
  })
if (process.env.CI && !browser) {
  throw new Error('Layout tests require Chrome/Chromium; set CHROME_BIN if it is not on PATH.')
}

const css = ['src/components/AppDialog/AppDialog.css', 'src/library-transfer/LibraryTransferDialog.css']
  .map(path => readFileSync(path, 'utf8')).join('\n')

// Real components/CSS in a browser: jsdom cannot detect grid tracks collapsing under overflow.
function fixture(connector: boolean) {
  const dialog = renderToStaticMarkup(<LibraryTransferDialog onBack={() => {}} onClose={() => {}} onImported={() => {}} />)
  const content = renderToStaticMarkup(<>
    <div className="library-transfer-mode" id="task-mode"><button>Send</button><button>Receive</button></div>
    <section className="library-transfer-primary-section">
      <header><h3>Connect Devices</h3><p>Choose the session on the other device and enter its pairing code.</p></header>
      <button id="refresh">Refresh Networks</button>
      {connector ? <>
        <NearbySessions interfaceId="test-network" mode="push" disabled={false} onSelect={() => {}} />
        <div className="library-transfer-pairing-fields">
          <label>Device Address<input placeholder="192.168.1.20:53318" /></label>
          <label>Pairing Code<input placeholder="ABCD-EFGH-JKLM" /></label>
        </div>
      </> : <details className="library-transfer-alternate"><summary>TCP Port (Advanced)</summary><input /></details>}
    </section>
    <section className="library-transfer-primary-section">
      <p>Keep both devices on the same local network.</p><button>Continue</button>
    </section>
    <div id="expanding-content" />
    <div className="library-transfer-primary-section"><button id="last-action">Last Action</button></div>
  </>)
  return `${dialog}<template id="flow-content">${content}</template>`
}

const measure = `
const body = document.querySelector('.app-dialog-body');
const rect = element => { const r = element.getBoundingClientRect(); return {top:r.top,bottom:r.bottom,height:r.height,width:r.width}; };
const dialog = document.querySelector('.app-dialog'), navigation = document.querySelector('.app-dialog-navigation');
const bounds = () => ({dialog:rect(dialog),navigation:rect(navigation)});
const entry = bounds();
body.replaceChildren(document.querySelector('#flow-content').content.cloneNode(true));
const content = [...body.children].filter(element => element.id !== 'task-mode');
document.querySelector('.app-dialog-header h2').textContent = 'Send';
document.querySelector('.app-dialog-header p').style.display = 'none';
const modes = () => ['task-mode'].map(id => {
  const group = document.getElementById(id);
  return {group:rect(group).height,button:rect(group.querySelector('button')).height};
});
const compact = modes();
const selection = bounds();
document.querySelector('#expanding-content').style.height = '900px';
const expanded = modes();
const connection = bounds();
const header = rect(document.querySelector('.app-dialog-header'));
const overflowing = body.scrollHeight > body.clientHeight;
document.querySelector('#last-action').focus();
const last = rect(document.querySelector('#last-action')), viewport = rect(body);
const reachable = last.top >= viewport.top - 1 && last.bottom <= viewport.bottom + 1;
const scrolled = bounds();
const horizontalOverflow = body.scrollWidth - body.clientWidth;
const style = button => { const s = getComputedStyle(button); return [s.minHeight,s.borderRadius,s.backgroundColor,s.color,s.fontSize,s.fontFamily,s.cursor]; };
const reference = document.querySelector('#refresh'), nearby = document.querySelector('.library-transfer-nearby button');
let buttons = null;
if (nearby) {
  const normal = {reference:style(reference),nearby:style(nearby)};
  nearby.focus(); const focused = document.activeElement === nearby;
  nearby.blur();
  reference.disabled = true; nearby.disabled = true;
  buttons = {normal,focused,disabled:{reference:style(reference),nearby:style(nearby)}};
}
document.querySelector('#expanding-content').style.height = '0';
body.scrollTop = 0;
const completion = bounds();
content.forEach(element => element.style.display = 'none');
dialog.classList.remove('library-transfer-dialog');
const ordinaryDialog = rect(dialog);
parent.postMessage({type:'papercut-layout',result:{width:innerWidth,height:innerHeight,compact,expanded,collapsed:modes(),header,overflowing,reachable,horizontalOverflow,buttons,entry,selection,connection,scrolled,completion,ordinaryDialog}},'*');
`

describe.skipIf(!browser)('transfer dialog browser layout', () => {
  const viewports = [
    {
      width: 390,
      height: 780,
      font: 16,
      dir: 'ltr',
    },
    {
      width: 320,
      height: 568,
      font: 16,
      dir: 'rtl',
    },
    {
      width: 390,
      height: 500,
      font: 24,
      dir: 'ltr',
    },
    {
      width: 740,
      height: 360,
      font: 16,
      dir: 'ltr',
    },
    {
      width: 800,
      height: 800,
      font: 24,
      dir: 'rtl',
    },
    {
      width: 1280,
      height: 800,
      font: 16,
      dir: 'ltr',
    },
  ]
  for (const viewport of viewports) {
    it.each([false, true])(`keeps controls visible/reachable at ${viewport.width}×${viewport.height}, ${viewport.font}px ${viewport.dir}, connector=%s`, connector => {
      const directory = mkdtempSync(join(tmpdir(), 'papercut-dialog-layout-'))
      try {
        const path = join(directory, 'fixture.html'); const frame = join(directory, 'frame.html')
        writeFileSync(frame, `<!doctype html><html dir="${viewport.dir}"><meta charset="utf-8"><style>
          :root {font-size:${viewport.font}px;--mobile-safe-top-space:1.35rem;--control-surface:#303640;--surface-muted:#20242a;--surface-accent:#363367;
          --text-body:#ddd;--accent-text:#bbf;--accent-strong:#ccf;--text-soft:#888;--radius-control:8px;--shadow-focus:#8899ff}
          *{box-sizing:border-box}body{font-family:Arial}${css}
          .app-dialog-backdrop{--dialog-safe-bottom:22px}</style>${fixture(connector)}<script>${measure}</script></html>`)
        // An iframe gives exact CSS viewports below Chrome's 500px desktop-window minimum.
        writeFileSync(path, `<!doctype html><script>addEventListener('message', ({data}) => {
          if(data?.type !== 'papercut-layout') return;
          const output=document.createElement('output'); output.id='layout-result';
          output.textContent=JSON.stringify(data.result); document.body.append(output);
        });</script><iframe style="border:0;width:${viewport.width}px;height:${viewport.height}px" src="${pathToFileURL(frame).href}"></iframe>`)
        const html = execFileSync(browser!, ['--headless', '--no-sandbox', '--disable-gpu', '--disable-background-networking',
          '--disable-dev-shm-usage', '--no-first-run', `--user-data-dir=${join(directory, 'profile')}`,
          '--window-size=1440,1100', '--virtual-time-budget=1000', '--dump-dom', pathToFileURL(path).href],
        {
          encoding: 'utf8',
          timeout: 20_000,
          maxBuffer: 1024 * 1024,
          stdio: ['ignore', 'pipe', 'pipe'],
        })
        const result = JSON.parse(html.match(/<output id="layout-result">([^<]+)<\/output>/)![1])
        expect(result.width).toBe(viewport.width)
        expect(result.height).toBe(viewport.height)
        for (const phase of ['compact', 'expanded', 'collapsed']) {
          for (const mode of result[phase]) {
            expect(mode.group).toBeGreaterThanOrEqual(mode.button)
          }
        }
        expect(result.header.top).toBeGreaterThanOrEqual(0)
        expect(result.header.bottom).toBeLessThanOrEqual(viewport.height)
        expect(result.overflowing).toBe(true)
        expect(result.reachable).toBe(true)
        expect(result.horizontalOverflow).toBeLessThanOrEqual(1)
        expect(result.entry.dialog.width).toBeCloseTo(Math.min(viewport.width - 2 * viewport.font, (viewport.width <= 767 ? 24 : 32) * viewport.font), 0)
        expect(result.ordinaryDialog.width).toBeCloseTo(Math.min(viewport.width - 2 * viewport.font, 24 * viewport.font), 0)
        if (viewport.width <= 767) {
          expect(result.entry.dialog.height).toBeCloseTo(viewport.height - 3.35 * viewport.font - 22, 0)
          for (const stage of ['selection', 'connection', 'scrolled', 'completion']) {
            expect(result[stage].dialog).toEqual(result.entry.dialog)
            expect(result[stage].navigation).toEqual(result.entry.navigation)
          }
          expect(result.entry.dialog.top).toBeGreaterThanOrEqual(2.35 * viewport.font - 0.5)
          expect(result.entry.dialog.bottom).toBeLessThanOrEqual(viewport.height - viewport.font - 22 + 0.5)
        } else {
          expect(result.entry.dialog.height).toBeLessThan(result.selection.dialog.height)
          expect(result.selection.dialog.height).toBeLessThanOrEqual(result.connection.dialog.height)
          expect(result.connection.dialog.height).toBeCloseTo(viewport.height - 2 * viewport.font - 22, 0)
        }
        expect(result.ordinaryDialog.height).toBeLessThan(result.connection.dialog.height)
        if (connector) {
          expect(result.buttons.normal.nearby).toEqual(result.buttons.normal.reference)
          expect(result.buttons.disabled.nearby).toEqual(result.buttons.disabled.reference)
          expect(result.buttons.focused).toBe(true)
        }
      } finally {
        rmSync(directory, {
          recursive: true,
          force: true,
        })
      }
    }, 30_000)
  }
})
