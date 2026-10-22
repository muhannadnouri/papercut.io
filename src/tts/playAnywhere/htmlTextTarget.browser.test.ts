// @vitest-environment node
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { build } from 'vite'
import { browser, runBrowserFixture } from './browserCaptureFixture'
import { afterAll, beforeAll, describe, expect, it } from 'vitest'

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
    ['src/tts/playAnywhere/prepareTextTarget.ts', 'createTextTargetPreparation'],
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
    (async()=>{try{await document.fonts.ready;${action}}catch(error){parent.postMessage({type:'capture-result',result:{error:String(error.stack||error)}},'*')}})();</script></html>`)
  return runBrowserFixture(directory, frame, options.width)
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
      const incidental=root.querySelector('p').firstChild;document.getSelection().setBaseAndExtent(incidental,0,incidental,6);
      scrollTo(0,720);
      const session=HtmlTargets.createHtmlTextTargetApi(root,bounds);
      const result=await session.api.captureViewport(), oracle=expected();
      let target=null;
      if(result.status==='captured'){
        const chunks=HtmlTargets.chunkAudiobookSaveHtmlWithSpans(root.innerHTML).map((c,i)=>({...c,id:'saved-'+i}));
        const preparation=HtmlTargets.createTextTargetPreparation({documentUrl:'/fixture.html',docContent:root.innerHTML,chunks,getChunks:async()=>chunks,getPdfSegments:async()=>[],getPdfPageText:async()=>{throw new Error('HTML must not load PDF')}});
        const resolved=await preparation.resolve('/fixture.html',result);
        target=resolved.status==='resolved'?{status:resolved.status,chunkIndex:resolved.chunkIndex,chunkId:resolved.chunkId}:resolved;
      }
      parent.postMessage({type:'capture-result',result:{capture:plain(result),oracle,target}},'*');session.dispose();
    `, options)
    expect(result.oracle).not.toBeNull()
    expect(result.capture.status, JSON.stringify(result)).toBe('captured')
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
    expect(result.capture.status, JSON.stringify(result)).toBe('captured')
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
    expect(result.capture.status, JSON.stringify(result)).toBe('captured')
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

  it.each([true, false])('waits for initial visibility and bounds a missing callback (delivers=%s)', async delivers => {
    const result = await run('<p>Visible text.</p>', `
      const Observer=window.IntersectionObserver;
      window.IntersectionObserver=class extends Observer {
        constructor(callback,options){super((entries,observer)=>{if(${delivers})setTimeout(()=>callback(entries,observer),120)},options)}
      };
      const session=HtmlTargets.createHtmlTextTargetApi(root,bounds);
      const result=await session.api.captureViewport();
      parent.postMessage({type:'capture-result',result:{capture:plain(result),oracle:expected()}},'*');session.dispose();
    `)
    if (delivers) {
      expect(result.capture.status, JSON.stringify(result)).toBe('captured')
      expect(result.capture.point).toEqual(result.oracle)
    } else {
      expect(result.capture).toEqual({ status: 'unavailable', reason: 'not-ready' })
    }
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
