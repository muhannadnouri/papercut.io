// @vitest-environment node
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { build } from 'vite'
import { browser, runBrowserFixture } from './browserCaptureFixture'
import { afterAll, beforeAll, describe, expect, it } from 'vitest'
let directory: string, bundle: string, css: string
beforeAll(async () => {
  directory = mkdtempSync(join(tmpdir(), 'papercut-reader-controls-'))
  const entry = join(directory, 'entry.tsx')
  writeFileSync(entry, `
    import React from ${JSON.stringify(resolve('node_modules/react/index.js'))};
    import {createRoot} from ${JSON.stringify(resolve('node_modules/react-dom/client.js'))};
    import {createPortal} from ${JSON.stringify(resolve('node_modules/react-dom/index.js'))};
    import {AudioControls} from ${JSON.stringify(resolve('src/tts/components/AudioControls.tsx'))};
    import {HtmlViewer} from ${JSON.stringify(resolve('src/viewers/HtmlViewer.tsx'))};
    import i18n from ${JSON.stringify(resolve('src/i18n/index.ts'))};
    import ${JSON.stringify(resolve('src/components/DocumentViewer/DocumentViewer.css'))};
    import {usePlayAnywhereReader} from ${JSON.stringify(resolve('src/tts/playAnywhere/usePlayAnywhereReader.ts'))};
    import {bindSelectionIntent} from ${JSON.stringify(resolve('src/tts/playAnywhere/selectionTarget.ts'))};
    import {readerTextViewport} from ${JSON.stringify(resolve('src/tts/playAnywhere/htmlTextTarget.ts'))};
    import {createTextTargetPreparation} from ${JSON.stringify(resolve('src/tts/playAnywhere/prepareTextTarget.ts'))};
    import {chunkAudiobookSaveHtmlWithSpans} from ${JSON.stringify(resolve('src/tts/utils/text.ts'))};
    export async function mount(locale,captureMode=false,playbackStatus='idle',legacy=false) {
      await i18n.changeLanguage(locale);document.documentElement.dir=locale==='ar'?'rtl':'ltr';
      const noop=()=>{};
      const shell=document.querySelector('.app-reader'),surface=shell.querySelector('.document-view'),content=surface.innerHTML;
      if(captureMode)surface.replaceChildren();
      const chunks=captureMode?chunkAudiobookSaveHtmlWithSpans(content).map((chunk,i)=>legacy?{id:'saved-'+i,text:chunk.text}:{...chunk,id:'saved-'+i}):[];
      const preparation=createTextTargetPreparation({documentUrl:'/user-uploads/ab.html',docContent:content,chunks,getChunks:async()=>chunks,getPdfSegments:async()=>[],getPdfPageText:async()=>{throw new Error('HTML must not read PDF')}});
      const playFromHere=async(api,onStarted)=>{
        const result=await api.captureViewport();
        const target=await preparation.resolve('/user-uploads/ab.html',result);
        if(target.status==='resolved')onStarted?.();
        await new Promise(requestAnimationFrame);
        parent.postMessage({type:'capture-result',result:{status:result.status,point:result.point,reason:result.reason,current:result.status==='captured'&&result.isCurrent(),target:{status:target.status,reason:target.reason,chunkIndex:target.chunkIndex,chunkId:target.chunkId}}},'*');
        return target.status==='resolved'?{status:'dispatched'}:target;
      };
      function Controls(){
        const reader=usePlayAnywhereReader({documentUrl:'fixture',active:!captureMode||playbackStatus!=='idle',playFromHere,playFromBeginning:async()=>({status:'dispatched'}),cancelTargetPlayback:noop});
        const contentRef=React.useRef(null);
        const viewport=React.useCallback(()=>readerTextViewport(shell),[]);
        const register=React.useCallback(api=>{reader.register(api);if(api)window.captureReady=true},[reader.register]);
        React.useEffect(()=>{
          if(!captureMode)return;
          const unbind=bindSelectionIntent(shell,reader.selectionActions);if(playbackStatus!=='idle')reader.browse();
          return unbind;
        },[]);
        return <>{captureMode&&createPortal(<HtmlViewer url="fixture" content={content} contentRef={contentRef} onTextTargetApiChange={register} getTextTargetViewport={viewport} audioFollowPolicy={reader.followPolicy}/>,surface)}<AudioControls
        audiobookState={{status:'saved',complete:true,cachedChunks:2,totalChunks:2}}
        canPlayAudiobook canSaveAudiobook={false} canSkipBackward canSkipForward modelSetupLabel={null} saveInProgress={false}
        onManageSave={noop} onPause={noop} onRead={noop} onResume={noop} onSelectSavedAudiobook={noop}
        onJumpToChunk={noop} onPlaybackRateChange={noop} onOpenAudioSetup={noop} onSave={noop}
        onSkipBackward={noop} onSkipForward={noop} onStop={noop} onWordHighlightEnabledChange={noop}
        playbackRate={1} savedAudiobooks={[]} selectedAudiobookId={null} wordHighlightEnabled
        ttsState={{status:captureMode?playbackStatus:'paused',message:'',chunksTotal:2,chunksPlayed:0,currentChunkIndex:0,currentChunkProgress:0,currentChunkDuration:0,chunkSummaries:[]}}
        playAnywhere={captureMode?{...reader,onPlayFromHere:reader.playFromHere,onPlayFromBeginning:reader.playFromBeginning,onReturnToAudio:reader.returnToAudio,onSnapshotSelection:reader.snapshotSelection,onPlayFromSelection:reader.playFromSelection,onSelectionMenuClose:reader.clearSelection}:{browsing:true,pending:false,feedback:'unmapped',onPlayFromHere:noop,onPlayFromBeginning:noop,onReturnToAudio:noop}} /></>;
      }
      createRoot(document.getElementById('controls')).render(<Controls/>);
    }
  `)
  const output = await build({ configFile: false, logLevel: 'silent', define: { 'process.env.NODE_ENV': JSON.stringify('production') }, resolve: { alias: { 'react/jsx-dev-runtime': resolve('node_modules/react/jsx-dev-runtime.js'), 'react/jsx-runtime': resolve('node_modules/react/jsx-runtime.js') } }, esbuild: { jsx: 'automatic', jsxDev: false }, build: {
    write: false, minify: false, lib: { entry, name: 'ReaderControls', formats: ['iife'] },
  } })
  const result = Array.isArray(output) ? output[0] : output
  if (!('output' in result)) throw new Error('Expected browser fixture bundle')
  bundle = result.output.filter(item => item.type === 'chunk').map(item => item.code).join('\n')
  css = result.output.filter(item => item.type === 'asset' && item.fileName.endsWith('.css')).map(item => item.type === 'asset' ? String(item.source) : '').join('\n')
}, 30_000)
afterAll(() => { if (directory) rmSync(directory, { recursive: true, force: true }) })
describe.skipIf(!browser)('Reader controls in Chromium', () => {
  it.each([{width:320,font:16,locale:'en'}, {width:390,font:32,locale:'ar'}, {width:1280,font:32,locale:'fr'}])('keeps browse/return controls usable outside the hidden fullscreen header: %j', async ({width,font,locale}) => {
    const frame = join(directory, 'frame.html')
    writeFileSync(frame, `<!doctype html><html><meta charset="utf-8"><style>html{font-size:${font}px}body{margin:0}${css}</style>
      <div class="app app-reader app-reader-pdf app-reader-pdf-fullscreen app-reader-pdf-toolbar-hidden"><header class="doc-header"><div id="controls"></div></header><main class="document-view">Reader</main></div>
      <script>${bundle.replace(/<\/script/gi, '<\\/script')}</script><script>
      (async()=>{try{await ReaderControls.mount(${JSON.stringify(locale)});while(!document.querySelector('.audio-playback-dock'))await new Promise(requestAnimationFrame);await document.fonts.ready;
      const dock=document.querySelector('.audio-playback-dock');const rect=e=>{const r=e.getBoundingClientRect();return {left:r.left,right:r.right,top:r.top,bottom:r.bottom,width:r.width,height:r.height}};
      const buttons=[...dock.querySelectorAll('button')];const visible=buttons.every(b=>{const r=b.getBoundingClientRect();return r.left>=0&&r.right<=innerWidth&&r.top>=0&&r.bottom<=innerHeight&&getComputedStyle(b).visibility==='visible'&&document.elementFromPoint(r.x+r.width/2,r.y+r.height/2)?.closest('button')===b});
      const browse=[...dock.querySelectorAll('.audio-browse-actions button')].map(rect);
      parent.postMessage({type:'capture-result',result:{visible,browse,dock:rect(dock),outsideHeader:!dock.closest('header'),feedback:dock.querySelector('[role=status]').textContent}},'*');
      }catch(e){parent.postMessage({type:'capture-result',result:{error:String(e.stack||e)}},'*')}})();</script>`)
    const result = await runBrowserFixture(directory, frame, width)
    expect(result.visible, JSON.stringify(result)).toBe(true)
    expect(result.outsideHeader).toBe(true); expect(result.feedback.length).toBeGreaterThan(0)
    expect(result.browse).toHaveLength(2)
    for (const button of result.browse) expect(button.height).toBeGreaterThanOrEqual(44)
  }, 30_000)
  it.each(['idle', 'playing', 'paused'])('captures the %s viewport without loading feedback invalidating its layout snapshot', async status => {
    const frame = join(directory, 'capture.html')
    writeFileSync(frame, `<!doctype html><html><meta charset="utf-8"><style>body{margin:0}${css}
      .doc-header{position:fixed;inset:0 0 auto;height:65px;background:white;z-index:2}
      .document-view{margin-top:80px}article{font:20px/1.7 Arial;margin:0 30px}p{margin-bottom:24px}
      </style><div class="app-reader"><header class="doc-header"><div id="controls"></div></header>
      <main class="document-view"><article>${Array.from({length:40},(_,i)=>`<p>Passage ${i}. A saved book has a readable sentence with several words.</p>`).join('')}</article></main></div>
      <script>${bundle.replace(/<\/script/gi, '<\\/script')}</script><script>
      (async()=>{await ReaderControls.mount('en',true,${JSON.stringify(status)});while(!window.captureReady)await new Promise(requestAnimationFrame);scrollTo(0,700);await new Promise(requestAnimationFrame);document.querySelector(${JSON.stringify(status === 'idle' ? '.audio-primary-btn' : '.audio-browse-actions button')}).click()})();</script>`)
    const result = await runBrowserFixture(directory, frame, 620)
    expect(result.status, JSON.stringify(result)).toBe('captured')
    expect(result.current).toBe(true)
    expect(result.target.status, JSON.stringify(result)).toBe('resolved')
    expect(result.point.segmentIndex).toBeGreaterThan(0)
  }, 30_000)

  it.each(['pointer', 'keyboard'])('keeps the chosen passage through %s menu focus and native selection collapse', async method => {
    const frame = join(directory, 'selection.html')
    writeFileSync(frame, `<!doctype html><html><meta charset="utf-8"><style>${css}article{margin:80px 30px;font:20px/1.6 Arial}</style>
      <div class="app-reader"><header class="doc-header"><div id="controls"></div></header><main class="document-view"><article><p>First paragraph.</p><p>Selected readable passage here.</p></article></main></div>
      <script>${bundle.replace(/<\/script/gi, '<\\/script')}</script><script>
      (async()=>{await ReaderControls.mount('en',true);while(!window.captureReady)await new Promise(requestAnimationFrame);
      const text=document.querySelectorAll('article p')[1].firstChild;document.getSelection().setBaseAndExtent(text,15,text,9);
      const trigger=document.querySelector('[aria-expanded=false]');
      if(${JSON.stringify(method)}==='keyboard')document.body.dispatchEvent(new KeyboardEvent('keydown',{key:'Tab',bubbles:true}));
      else trigger.dispatchEvent(new PointerEvent('pointerdown',{bubbles:true,pointerType:'touch'}));
      if(${JSON.stringify(method)}==='keyboard'){const intermediate=document.createElement('button');document.body.append(intermediate);intermediate.focus();document.getSelection().removeAllRanges();intermediate.dispatchEvent(new KeyboardEvent('keydown',{key:'Tab',bubbles:true}));}
      trigger.focus();document.getSelection().removeAllRanges();trigger.click();
      while(!document.querySelector('.audio-chunk-menu'))await new Promise(requestAnimationFrame);
      const action=[...document.querySelectorAll('.audio-chunk-menu button')].find(button=>button.textContent==='Play from selection');
      if(!action)throw new Error('Missing explicit selection action');action.click();})();</script>`)
    const result = await runBrowserFixture(directory, frame, 390)
    expect(result.status, JSON.stringify(result)).toBe('captured')
    expect(result.current).toBe(true)
    expect(result.target.status, JSON.stringify(result)).toBe('resolved')
    expect(result.point).toEqual({ format: 'html', segmentIndex: 1, offset: 9 })
  }, 30_000)

  it.each([false, true])('resolves imported clause-spaced audio from the top and browse controls (legacy=%s)', async legacy => {
    const frame = join(directory, 'imported.html')
    const text = 'A narrated clause with repeated words '.repeat(4).trim() + ',“Another clause follows without a source space”, ' +
      'A final long clause keeps the sentence above the splitting limit '.repeat(8).trim() + '.'
    writeFileSync(frame, `<!doctype html><html><meta charset="utf-8"><style>body{margin:0}${css}
      .doc-header{position:fixed;inset:0 0 auto;height:65px;background:white;z-index:2}
      .document-view{margin-top:80px}article{font:20px/1.7 Arial;margin:0 30px}p{margin-bottom:24px}
      </style><div class="app-reader"><header class="doc-header"><div id="controls"></div></header>
      <main class="document-view"><article><p>Opening passage.</p><p>${text}</p>${Array.from({length:30},(_,i)=>`<p>Later passage ${i}. A saved book continues here.</p>`).join('')}</article></main></div>
      <script>${bundle.replace(/<\/script/gi, '<\\/script')}</script><script>
      (async()=>{await ReaderControls.mount('en',true,${JSON.stringify(legacy ? 'playing' : 'idle')},${legacy});while(!window.captureReady)await new Promise(requestAnimationFrame);
      ${legacy ? "scrollTo(0,1000);await new Promise(requestAnimationFrame);document.querySelector('.audio-browse-actions button').click()" : "document.querySelector('.audio-primary-btn').click()"}})();</script>`)
    const result = await runBrowserFixture(directory, frame, 620)
    expect(result.status, JSON.stringify(result)).toBe('captured')
    expect(result.current).toBe(true)
    expect(result.target.status, JSON.stringify(result)).toBe('resolved')
    expect(result.target.chunkId).toBe('saved-' + result.target.chunkIndex)
    if (legacy) expect(result.target.chunkIndex).toBeGreaterThan(0)
  }, 30_000)

})
