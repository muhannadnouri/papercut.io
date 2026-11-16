// @vitest-environment node
import {
  mkdtempSync,
  rmSync,
  writeFileSync,
} from 'node:fs'
import { tmpdir } from 'node:os'
import {
  join,
  resolve,
} from 'node:path'
import { build } from 'vite'
import {
  afterAll,
  beforeAll,
  describe,
  expect,
  it,
} from 'vitest'
import {
  browser,
  runBrowserFixture,
} from '../playAnywhere/browserCaptureFixture'

let directory: string
let bundle: string
let css: string

beforeAll(async () => {
  directory = mkdtempSync(join(tmpdir(), 'papercut-setup-errors-'))
  const entry = join(directory, 'entry.tsx')
  writeFileSync(entry, `
    import React from ${JSON.stringify(resolve('node_modules/react/index.js'))};
    import {createRoot} from ${JSON.stringify(resolve('node_modules/react-dom/client.js'))};
    import {AudioSetupDialog} from ${JSON.stringify(resolve('src/tts/components/AudioSetupDialog.tsx'))};
    import {FALLBACK_TTS_MODELS} from ${JSON.stringify(resolve('src/tts/models.ts'))};
    import i18n from ${JSON.stringify(resolve('src/i18n/index.ts'))};
    import ${JSON.stringify(resolve('src/index.css'))};
    import ${JSON.stringify(resolve('src/tts/components/AudiobooksPanel.css'))};
    import ${JSON.stringify(resolve('src/tts/components/AudioControls.css'))};
    export async function mount(locale) {
      await i18n.changeLanguage(locale);
      document.documentElement.dir=locale==='ar'?'rtl':'ltr';
      const model=FALLBACK_TTS_MODELS.find(item=>item.name==='Kokoro 7M'),noop=()=>{};
      const url='https://github.com/muhannadnouri/papercut.io/releases/download/tts-distill-7m-fp32-2026-11-15/kokoro-7m-distill-en-fp32.tar.bz2';
      const message='Failed to download offline voice model: '+url+': HTTP status client error (404 Not Found) for url ('+url+')';
      createRoot(document.getElementById('fixture')).render(<AudioSetupDialog title="Audio Setup" onClose={noop} audioSetup={{
        appliedThreadCount:null,defaultThreadCount:4,maxThreadCount:8,modelId:model.id,models:[model],modelStatus:null,
        modelInstallProgress:{modelId:model.id,status:'error',message,downloadedBytes:0,totalBytes:35634041,percent:0},
        onInstallModel:noop,onModelChange:noop,onPreviewStart:noop,onSilmaNfeStepChange:noop,onTextPreprocessorChange:noop,
        onThreadCountChange:noop,onVoiceChange:noop,silmaNfeStep:8,textPreprocessor:model.defaultTextPreprocessor,
        textPreprocessors:model.textPreprocessors,threadCount:4,voice:model.defaultVoice,voices:model.voices
      }}/>);
      requestAnimationFrame(()=>requestAnimationFrame(()=>{
        const dialog=document.querySelector('.app-dialog'),body=document.querySelector('.app-dialog-body');
        const status=document.querySelector('.audiobook-status-error');
        const bounds=status.getBoundingClientRect(),container=document.querySelector('.audio-model-install').getBoundingClientRect();
        parent.postMessage({type:'capture-result',result:{
          dialogOverflow:dialog.scrollWidth-dialog.clientWidth,bodyOverflow:body.scrollWidth-body.clientWidth,
          statusOverflow:status.scrollWidth-status.clientWidth,
          withinColumn:bounds.left>=container.left-1&&bounds.right<=container.right+1,
          fullError:status.textContent.includes(message),percentage:status.textContent.includes('0%')
        }},'*');
      }));
    }
  `)
  const output = await build({
    configFile: false,
    logLevel: 'silent',
    define: { 'process.env.NODE_ENV': JSON.stringify('production') },
    resolve: { alias: { 'react/jsx-runtime': resolve('node_modules/react/jsx-runtime.js') } },
    esbuild: {
      jsx: 'automatic',
      jsxDev: false,
    },
    build: {
      write: false,
      minify: false,
      lib: {
        entry,
        name: 'SetupErrors',
        formats: ['iife'],
      },
    },
  })
  const result = Array.isArray(output) ? output[0] : output
  if (!('output' in result)) {
    throw new Error('Expected setup fixture bundle')
  }
  bundle = result.output.filter(item => item.type === 'chunk').map(item => item.code).join('\n')
  css = result.output.filter(item => item.type === 'asset' && item.fileName.endsWith('.css'))
    .map(item => item.type === 'asset' ? String(item.source) : '').join('\n')
}, 30_000)

afterAll(() => {
  if (directory) {
    rmSync(directory, {
      recursive: true,
      force: true,
    })
  }
})

// JSDOM cannot detect flex/grid intrinsic-size overflow from real download URLs.
describe.skipIf(!browser)('Audio setup download errors in Chromium', () => {
  it.each([[390, 'en'], [900, 'en'], [390, 'ar'], [900, 'ar']] as const)(
    'wraps full diagnostics without horizontal scrolling at %ipx in %s', async (width, locale) => {
      const frame = join(directory, `error-${width}-${locale}.html`)
      writeFileSync(frame, `<!doctype html><html><meta charset="utf-8"><style>${css}</style>
        <div id="fixture"></div><script>${bundle.replace(/<\/script/gi, '<\\/script')}</script>
        <script>SetupErrors.mount(${JSON.stringify(locale)});</script>`)
      const result = await runBrowserFixture(directory, frame, width,
        resolve('.cache/sherpa-stage4c-2026-11-16', `error-${width}-${locale}.png`))
      expect(result).toMatchObject({
        withinColumn: true,
        fullError: true,
        percentage: false,
      })
      expect(result.dialogOverflow).toBeLessThanOrEqual(1)
      expect(result.bodyOverflow).toBeLessThanOrEqual(1)
      expect(result.statusOverflow).toBeLessThanOrEqual(1)
    }, 20_000,
  )
})
