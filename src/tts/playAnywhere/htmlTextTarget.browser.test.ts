// @vitest-environment node
import { execFileSync, spawn } from 'node:child_process'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import type { Readable, Writable } from 'node:stream'
import { StringDecoder } from 'node:string_decoder'
import { build } from 'vite'
import { afterAll, beforeAll, describe, expect, it } from 'vitest'

const browser = [process.env.CHROME_BIN, 'chromium', 'chromium-browser', 'google-chrome', 'google-chrome-stable']
  .find(candidate => {
    if (!candidate) return false
    try { execFileSync(candidate, ['--version'], { stdio: 'ignore', timeout: 5000 }); return true } catch { return false }
  })
if (process.env.CI && !browser) throw new Error('HTML capture tests require Chrome/Chromium; set CHROME_BIN.')

let directory: string
let bundle: string
beforeAll(async () => {
  directory = mkdtempSync(join(tmpdir(), 'papercut-html-target-'))
  const entry = join(directory, 'entry.ts')
  writeFileSync(entry, [
    ['src/tts/playAnywhere/htmlTextTarget.ts', 'createHtmlTextTargetApi, readerTextViewport'],
    ['src/tts/alignment/domTextSegments.ts', 'buildReadableDomSegmentIndex, createRangeForSourceSpan'],
    ['src/tts/alignment/readableSegments.ts', 'normalizeSegmentText'],
    ['src/tts/utils/text.ts', 'chunkAudiobookSaveHtmlWithSpans'],
    ['src/tts/playAnywhere/sourceChunkResolver.ts', 'createChunkSourceResolver'],
  ].map(([path, exports]) => `export { ${exports} } from ${JSON.stringify(resolve(path))}`).join('\n'))
  const output = await build({ configFile: false, logLevel: 'silent', build: {
    write: false, minify: false, lib: { entry, name: 'HtmlTargets', formats: ['iife'] },
  } })
  const result = Array.isArray(output) ? output[0] : output
  if (!('output' in result)) throw new Error('Expected a bundled browser fixture')
  bundle = result.output.filter(item => item.type === 'chunk').map(item => item.code).join('\n')
}, 30_000)
afterAll(() => { if (directory) rmSync(directory, { recursive: true, force: true }) })

const paragraphs = Array.from({ length: 36 }, (_, i) =>
  `<p>Paragraph ${i}. First repeated sentence. <b>First repeated sentence.</b> Last passage here.</p>`).join('')

// The oracle measures actual glyph ranges in source order, independently of caret hit-testing.
const script = `
const root = document.querySelector('article');
const shell = document.querySelector('.app-reader');
const bounds = () => HtmlTargets.readerTextViewport(shell);
function visible(node) {
  if(getComputedStyle(node.parentElement).visibility!=='visible')return false;
  for (let el=node.parentElement;el;el=el.parentElement) {
    const s=getComputedStyle(el);
    if(s.display==='none'||s.opacity==='0')return false;
    if(el===root)return true;
  }
  return false;
}
function expected() {
  const box=bounds(), index=HtmlTargets.buildReadableDomSegmentIndex(root);
  if(!box)return null;
  for(let segmentIndex=0;segmentIndex<index.segments.length;segmentIndex++) {
    const segment=index.segments[segmentIndex];
    const text=HtmlTargets.normalizeSegmentText(segment.textNodes.map(n=>n.data).join(''));
    for(let offset=0;offset<text.length;offset++) {
      if(/\\s/.test(text[offset]))continue;
      const range=HtmlTargets.createRangeForSourceSpan(document,index,{startSegmentIndex:segmentIndex,endSegmentIndex:segmentIndex,startOffset:offset,endOffset:offset+1});
      if(!range||!visible(range.startContainer))continue;
      if([...range.getClientRects()].some(r=>r.bottom>box.top&&r.top<box.bottom&&r.right>box.left&&r.left<box.right))
        return {format:'html',segmentIndex,offset};
    }
  }
  return null;
}
function plain(result) { return result.status==='captured'
  ? {status:result.status,point:result.point,current:result.isCurrent()}
  : result; }
`

async function run(content: string, action: string, options: { width?: number; font?: number; dir?: string; css?: string } = {}) {
  const frame = join(directory, 'frame.html')
  writeFileSync(frame, `<!doctype html><html dir="${options.dir ?? 'ltr'}"><meta charset="utf-8"><style>
    body{margin:0;font-family:Arial}main{margin-top:80px}article{width:calc(100% - 60px);margin:auto;font-size:${options.font ?? 20}px;line-height:1.6}
    p{margin:0 0 24px}.doc-header{position:fixed;top:0;height:60px;width:100%;background:white;z-index:10}
    .audio-floating-playback{position:fixed;bottom:0;height:50px;width:100%;background:white;z-index:10}${options.css ?? ''}
    </style><div class="app-reader"><header class="doc-header">Toolbar</header><main class="document-view"><article>${content}</article></main><div class="audio-floating-playback">Audio</div></div>
    <script>${bundle.replace(/<\/script/gi, '<\\/script')}</script><script>${script}
    (async()=>{try{${action}}catch(error){parent.postMessage({type:'capture-result',result:{error:String(error.stack||error)}},'*')}})();</script></html>`)
  // A DevTools pipe waits for real asynchronous layout; dump-dom virtual time can stop between RAFs.
  const child = spawn(browser!, ['--headless', '--no-sandbox', '--disable-gpu', '--disable-background-networking',
    '--disable-dev-shm-usage', '--no-first-run', '--remote-debugging-pipe',
    `--user-data-dir=${join(directory, 'profile')}`, 'about:blank'],
  { stdio: ['ignore', 'ignore', 'pipe', 'pipe', 'pipe'] })
  const input = child.stdio[3] as Writable, output = child.stdio[4] as Readable
  const closed = new Promise<void>((resolve) => child.once('close', () => resolve()))
  const commands = new Map<number, { resolve: (value: Record<string, unknown>) => void; reject: (error: Error) => void }>()
  let nextId = 0, buffer = '', errors = ''
  const decoder = new StringDecoder('utf8')
  child.stderr?.on('data', (data) => { errors = (errors + data).slice(-2000) })
  const fail = (error: Error) => {
    for (const command of commands.values()) command.reject(error)
    commands.clear()
  }
  child.on('error', fail)
  input.on('error', fail)
  output.on('error', fail)
  child.on('exit', () => fail(new Error(`Browser exited: ${errors}`)))
  output.on('data', (data) => {
    buffer += decoder.write(data)
    let end: number
    while ((end = buffer.indexOf('\0')) >= 0) {
      const message = JSON.parse(buffer.slice(0, end))
      buffer = buffer.slice(end + 1)
      const command = commands.get(message.id)
      if (!command) continue
      commands.delete(message.id)
      if (message.error) command.reject(new Error(JSON.stringify(message.error)))
      else command.resolve(message.result)
    }
  })
  const send = (method: string, params: Record<string, unknown> = {}, sessionId?: string) =>
    new Promise<Record<string, unknown>>((resolve, reject) => {
      const id = ++nextId
      commands.set(id, { resolve, reject })
      input.write(JSON.stringify({ id, method, params, sessionId }) + '\0')
    })
  const timer = setTimeout(() => { fail(new Error(`Browser fixture timed out: ${errors}`)); child.kill() }, 15_000)
  try {
    const target = await send('Target.createTarget', { url: 'about:blank' })
    const session = await send('Target.attachToTarget', { targetId: target.targetId, flatten: true })
    const sessionId = String(session.sessionId)
    await send('Page.enable', {}, sessionId)
    await send('Emulation.setDeviceMetricsOverride', { width: options.width ?? 620, height: 600, deviceScaleFactor: 1, mobile: false }, sessionId)
    await send('Page.addScriptToEvaluateOnNewDocument', { source: `window.captureResult=new Promise(resolve=>addEventListener('message',({data})=>{if(data?.type==='capture-result')resolve(JSON.stringify(data.result))}));` }, sessionId)
    await send('Page.navigate', { url: pathToFileURL(frame).href }, sessionId)
    const evaluated = await send('Runtime.evaluate', { expression: 'window.captureResult', awaitPromise: true, returnByValue: true }, sessionId)
    const value = evaluated.result as { value?: string }
    if (!value.value) throw new Error(`No capture result: ${JSON.stringify(evaluated)}`)
    const result = JSON.parse(value.value)
    expect(result.error).toBeUndefined()
    return result
  } finally { clearTimeout(timer); child.kill(); await closed }
}

describe.skipIf(!browser)('HTML viewport capture in a rendered browser', () => {
  it.each([
    { width: 620, font: 20, dir: 'ltr' },
    { width: 390, font: 24, dir: 'rtl' },
    { width: 1280, font: 20, dir: 'ltr', css: 'article{column-count:2;column-gap:70px}' },
  ])('targets the first visible source character under chrome: %j', async options => {
    const content = options.dir === 'rtl'
      ? paragraphs.replaceAll('First repeated sentence.', 'بِدايَةٌ واضحةٌ. نِهايَةٌ واضحةٌ.') : paragraphs
    const result = await run(content, `
      scrollTo(0,720);
      const session=HtmlTargets.createHtmlTextTargetApi(root,bounds);
      const result=await session.api.captureViewport(), oracle=expected();
      let target=null;
      if(result.status==='captured'){
        const chunks=HtmlTargets.chunkAudiobookSaveHtmlWithSpans(root.innerHTML).map((c,i)=>({...c,id:'saved-'+i}));
        const resolver=HtmlTargets.createChunkSourceResolver(chunks,result.segmentLengths);
        target=resolver.status==='ready'?resolver.resolve(result.point):resolver;
      }
      parent.postMessage({type:'capture-result',result:{capture:plain(result),oracle,target}},'*');session.dispose();
    `, options)
    expect(result.oracle).not.toBeNull()
    expect(result.capture.status).toBe('captured')
    expect(result.capture.point).toEqual(result.oracle)
    expect(result.capture.current).toBe(true)
    expect(result.target.status).toBe('resolved')
    expect(result.target.chunkIndex).toBeGreaterThan(0)
  }, 30_000)

  it('captures inside a long paragraph and supports the WebKit caret fallback', async () => {
    const result = await run(`<p>${'First sentence. Second sentence. '.repeat(180)}</p>`, `
      Object.defineProperty(document,'caretPositionFromPoint',{value:undefined});scrollTo(0,1100);
      const session=HtmlTargets.createHtmlTextTargetApi(root,bounds);
      const result=await session.api.captureViewport();
      parent.postMessage({type:'capture-result',result:{capture:plain(result),oracle:expected()}},'*');session.dispose();
    `)
    expect(result.capture.status).toBe('captured')
    expect(result.capture.point).toEqual(result.oracle)
    expect(result.capture.point.offset).toBeGreaterThan(100)
  }, 30_000)

  it('clips to a nested scrolling reader and recomputes after font reflow', async () => {
    const result = await run(paragraphs, `
      root.scrollTop=700;const session=HtmlTargets.createHtmlTextTargetApi(root,bounds);
      const first=await session.api.captureViewport(), firstOracle=expected();
      const pending=session.api.captureViewport();root.style.fontSize='28px';
      const reflow=await pending, second=await session.api.captureViewport();
      parent.postMessage({type:'capture-result',result:{first:plain(first),firstOracle,reflow,second:plain(second),secondOracle:expected()}},'*');session.dispose();
    `, { css: 'article{height:420px;overflow:auto}' })
    expect(result.first.status).toBe('captured')
    expect(result.first.point).toEqual(result.firstOracle)
    expect(result.reflow).toEqual({ status: 'unavailable', reason: 'stale' })
    expect(result.second.status).toBe('captured')
    expect(result.second.point).toEqual(result.secondOracle)
  }, 30_000)

  it('skips hidden text and reference markers without changing narration offsets', async () => {
    const result = await run('<p style="opacity:0">Hidden source</p><p><span style="visibility:hidden">Hidden inline </span><a href="#note">[1]</a>Visible <mark>passage</mark>.</p>', `
      const session=HtmlTargets.createHtmlTextTargetApi(root,bounds);
      const result=await session.api.captureViewport();
      parent.postMessage({type:'capture-result',result:{capture:plain(result),oracle:expected()}},'*');session.dispose();
    `)
    expect(result.capture.status).toBe('captured')
    expect(result.capture.point).toEqual(result.oracle)
  }, 30_000)

  it('invalidates immediate text changes, rejects pending scrolls and cancels replaced captures', async () => {
    const result = await run(paragraphs, `
      const session=HtmlTargets.createHtmlTextTargetApi(root,bounds);
      const first=await session.api.captureViewport();
      root.querySelector('b').firstChild.data='Replacement passage.';
      const stale=first.status==='captured'&&!first.isCurrent();
      const scrolling=session.api.captureViewport();scrollTo(0,900);
      const scrollResult=await scrolling;
      const old=session.api.captureViewport(), latest=session.api.captureViewport();
      const cancelled=await old, current=await latest;
      const pending=session.api.captureViewport();session.dispose();
      parent.postMessage({type:'capture-result',result:{stale,scrollResult,cancelled,current:plain(current),disposed:await pending}},'*');
    `)
    expect(result.stale).toBe(true)
    expect(result.scrollResult).toEqual({ status: 'unavailable', reason: 'stale' })
    expect(result.cancelled).toEqual({ status: 'unavailable', reason: 'stale' })
    expect(result.current.status).toBe('captured')
    expect(result.disposed).toEqual({ status: 'unavailable', reason: 'stale' })
  }, 30_000)

  it('does not pick offscreen audio when only an image area is visible', async () => {
    const result = await run('<div style="height:1200px" role="img" aria-label="Illustration"></div><p>Later text.</p>', `
      scrollTo(0,200);const session=HtmlTargets.createHtmlTextTargetApi(root,bounds);
      const result=await session.api.captureViewport();parent.postMessage({type:'capture-result',result:plain(result)},'*');session.dispose();
    `)
    expect(result).toEqual({ status: 'unavailable', reason: 'no-visible-text' })
  }, 30_000)

  it('rejects pending work on text mutation, hidden documents and detached readers', async () => {
    const result = await run(paragraphs, `
      const session=HtmlTargets.createHtmlTextTargetApi(root,bounds);
      const mutation=session.api.captureViewport();root.querySelector('b').firstChild.data='Changed.';
      const mutated=await mutation, first=await session.api.captureViewport();
      const background=session.api.captureViewport();
      Object.defineProperty(document,'visibilityState',{value:'hidden',configurable:true});document.dispatchEvent(new Event('visibilitychange'));
      const hidden=await background, unavailable=await session.api.captureViewport();
      Object.defineProperty(document,'visibilityState',{value:'visible',configurable:true});
      const stale=first.status==='captured'&&!first.isCurrent();
      const detached=session.api.captureViewport();root.remove();
      parent.postMessage({type:'capture-result',result:{mutated,hidden,unavailable,stale,detached:await detached}},'*');session.dispose();
    `)
    expect(result.mutated).toEqual({ status: 'unavailable', reason: 'stale' })
    expect(result.hidden).toEqual({ status: 'unavailable', reason: 'stale' })
    expect(result.unavailable).toEqual({ status: 'unavailable', reason: 'not-ready' })
    expect(result.stale).toBe(true)
    expect(result.detached).toEqual({ status: 'unavailable', reason: 'stale' })
  }, 30_000)

  it('reports unavailable when the browser provides neither caret API', async () => {
    const result = await run('<p>Visible text.</p>', `
      Object.defineProperty(document,'caretPositionFromPoint',{value:undefined});
      Object.defineProperty(document,'caretRangeFromPoint',{value:undefined});
      const session=HtmlTargets.createHtmlTextTargetApi(root,bounds);
      parent.postMessage({type:'capture-result',result:plain(await session.api.captureViewport())},'*');session.dispose();
    `)
    expect(result).toEqual({ status: 'unavailable', reason: 'unmapped' })
  }, 30_000)
})
