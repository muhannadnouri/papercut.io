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
    import "/home/catfish/Documents/GitHub/papercut.io/src/index.css";
    import "/home/catfish/Documents/GitHub/papercut.io/src/App.css";
    import React from ${JSON.stringify(resolve('node_modules/react/index.js'))};
    import {createRoot} from ${JSON.stringify(resolve('node_modules/react-dom/client.js'))};
    import {createPortal} from ${JSON.stringify(resolve('node_modules/react-dom/index.js'))};
    import {ReaderPassageAction} from ${JSON.stringify(resolve('src/tts/playAnywhere/ReaderPassageAction.tsx'))};
    import {AudioControls} from ${JSON.stringify(resolve('src/tts/components/AudioControls.tsx'))};
    import {HtmlViewer} from ${JSON.stringify(resolve('src/viewers/HtmlViewer.tsx'))};
    import i18n from ${JSON.stringify(resolve('src/i18n/index.ts'))};
    import ${JSON.stringify(resolve('src/components/DocumentViewer/DocumentViewer.css'))};
    import {usePlayAnywhereReader} from ${JSON.stringify(resolve('src/tts/playAnywhere/usePlayAnywhereReader.ts'))};
    import {bindSelectionIntent} from ${JSON.stringify(resolve('src/tts/playAnywhere/selectionTarget.ts'))};
    import {readerTextViewport} from ${JSON.stringify(resolve('src/tts/playAnywhere/htmlTextTarget.ts'))};
    import {createTextTargetPreparation} from ${JSON.stringify(resolve('src/tts/playAnywhere/prepareTextTarget.ts'))};
    import {chunkAudiobookSaveHtmlWithSpans} from ${JSON.stringify(resolve('src/tts/utils/text.ts'))};
    export async function mount(locale,captureMode=false,playbackStatus='idle',legacy=false,priorFeedback=null) {
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
        return <>{captureMode&&playbackStatus==='idle'&&createPortal(<div className="reader-floating-actions"><ReaderPassageAction pending={reader.pending} onSnapshotSelection={reader.snapshotSelection} onPlayFromHere={reader.playFromHere} onPlayFromSelection={reader.playFromSelection}/><button className="reader-bookmark-btn">Bookmark</button><button className="scroll-top-btn">Top</button></div>,shell)}{captureMode&&createPortal(<HtmlViewer url="fixture" content={content} contentRef={contentRef} onTextTargetApiChange={register} getTextTargetViewport={viewport} audioFollowPolicy={reader.followPolicy}/>,surface)}<AudioControls
        audiobookState={{status:'saved',complete:true,cachedChunks:2,totalChunks:2}}
        canPlayAudiobook canSaveAudiobook={false} canSkipBackward canSkipForward modelSetupLabel={null} saveInProgress={false}
        onManageSave={noop} onPause={noop} onRead={noop} onResume={noop} onSelectSavedAudiobook={noop}
        onJumpToChunk={noop} onPlaybackRateChange={noop} onOpenAudioSetup={noop} onSave={noop}
        onSkipBackward={noop} onSkipForward={noop} onStop={noop} onWordHighlightEnabledChange={noop}
        playbackRate={1} savedAudiobooks={[]} selectedAudiobookId={null} wordHighlightEnabled
        ttsState={{status:captureMode?playbackStatus:'paused',message:'',chunksTotal:captureMode?2:184,chunksPlayed:0,currentChunkIndex:captureMode?0:18,currentChunkProgress:0,currentChunkDuration:0,chunkSummaries:captureMode?[]:Array.from({length:184},(_,index)=>({index,chunkId:'fixture-'+index,textPreview:'Synthetic narrated passage '+(index+1)+' with several readable words.'}))}}
        playAnywhere={captureMode?{...reader,feedback:reader.feedback??priorFeedback,onPlayFromHere:reader.playFromHere,onPlayFromBeginning:reader.playFromBeginning,onReturnToAudio:reader.returnToAudio,onSnapshotSelection:reader.snapshotSelection,onPlayFromSelection:reader.playFromSelection,onSelectionMenuClose:reader.clearSelection}:{browsing:true,pending:false,feedback:'unmapped',onPlayFromHere:noop,onPlayFromBeginning:noop,onReturnToAudio:noop,onPlayFromSelection:noop,onSnapshotSelection:()=>false}} /></>;
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
      const browse=[...dock.querySelectorAll('.audio-return-control')].map(rect);const inlineReturn=Boolean(dock.querySelector('.audio-floating-playback > .audio-return-control'));
      parent.postMessage({type:'capture-result',result:{visible,browse,inlineReturn,extraStrip:Boolean(dock.querySelector('.audio-browse-actions')),dock:rect(dock),outsideHeader:!dock.closest('header'),feedback:dock.querySelector('[role=status]').textContent}},'*');
      }catch(e){parent.postMessage({type:'capture-result',result:{error:String(e.stack||e)}},'*')}})();</script>`)
    const result = await runBrowserFixture(directory, frame, width)
    expect(result.visible, JSON.stringify(result)).toBe(true)
    expect(result.outsideHeader).toBe(true); expect(result.feedback.length).toBeGreaterThan(0)
    expect(result.browse).toHaveLength(1)
    expect(result.inlineReturn).toBe(true); expect(result.extraStrip).toBe(false)
    for (const button of result.browse) expect(button.height).toBeGreaterThanOrEqual(44)
  }, 30_000)
  it.each([{width:320,font:16,locale:'en',theme:'light'}, {width:620,font:16,locale:'en',theme:'light'},
    {width:390,font:32,locale:'ar',theme:'dark'}, {width:1280,font:32,locale:'fr',theme:'light'}])('reflows passage commands and keeps the popover reachable: %j', async ({width,font,locale,theme}) => {
    const frame = join(directory, 'popover.html')
    writeFileSync(frame, `<!doctype html><html data-theme="${theme}"><meta charset="utf-8"><style>${css}
      html{font-size:${font}px}body{margin:0}.document-view{padding:24px}article{font-size:1.1rem;line-height:1.6}
      </style><div class="app-reader"><header class="doc-header"><div id="controls"></div></header>
      <main class="document-view"><article>${'<p>A reader should remain easy to read while audio continues. Browsing pauses following, and Return reveals the spoken passage.</p>'.repeat(12)}</article></main></div>
      <script>${bundle.replace(/<\/script/gi, '<\\/script')}</script><script>
      (async()=>{await ReaderControls.mount(${JSON.stringify(locale)});while(!document.querySelector('.audio-menu-btn'))await new Promise(requestAnimationFrame);
      const trigger=document.querySelector('.audio-menu-btn');trigger.click();while(!document.querySelector('.audio-chunk-list'))await new Promise(requestAnimationFrame);
      const listBounds=document.querySelector('.audio-chunk-list').getBoundingClientRect();
      const chunkMenuBounds=document.querySelector('.audio-chunk-menu').getBoundingClientRect();
      const defaultChunks=!document.querySelector('.audio-passage-actions')&&!document.querySelector('.audio-word-highlight-toggle');
      document.querySelectorAll('.audio-menu-views button')[1].click();while(!document.querySelector('.audio-passage-actions'))await new Promise(requestAnimationFrame);
      await document.fonts.ready;await new Promise(requestAnimationFrame);
      const menu=document.querySelector('.audio-chunk-menu'),actions=[...menu.querySelectorAll('.audio-passage-command')];
      const rect=e=>{const r=e.getBoundingClientRect();return {left:r.left,right:r.right,top:r.top,bottom:r.bottom,height:r.height,width:r.width}};
      const initial=actions.map(rect),bounds=rect(menu);
      let reachable=true;
      for(const button of [...actions,menu.querySelector('.audio-word-highlight-toggle')]){
        button.scrollIntoView({block:'nearest'});const r=rect(button),m=rect(menu);
        reachable&&=r.top>=m.top&&r.bottom<=m.bottom&&r.left>=m.left&&r.right<=m.right;
      }
      menu.scrollTop=0;
      const focusVisible=Boolean(getComputedStyle(document.activeElement).outlineStyle!=='none');
      document.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true}));await new Promise(requestAnimationFrame);
      const escaped=!document.querySelector('.audio-chunk-menu')&&document.activeElement===trigger;
      trigger.click();await new Promise(requestAnimationFrame);
      parent.postMessage({type:'capture-result',result:{bounds,initial,reachable,escaped,focusVisible,defaultChunks,listHeight:listBounds.height,menuHeight:chunkMenuBounds.height,
        disabled:actions[1].disabled,icons:actions.every(b=>b.querySelector('svg')),inlineReturn:Boolean(document.querySelector('.audio-floating-playback>.audio-return-control'))}},'*');})();</script>`)
    const result = await runBrowserFixture(directory, frame, width,
      resolve('src-tauri/target/play-anywhere-acceptance/ui-review', `popover-${width}-${locale}-${theme}.png`))
    expect(result.bounds.left).toBeGreaterThanOrEqual(0)
    expect(result.bounds.top).toBeGreaterThanOrEqual(0)
    expect(result.bounds.right).toBeLessThanOrEqual(width)
    expect(result.bounds.bottom).toBeLessThanOrEqual(600)
    expect(result.reachable, JSON.stringify(result)).toBe(true)
    expect(result).toMatchObject({ disabled: true, icons: true, escaped: true, focusVisible: true, inlineReturn: true, defaultChunks: true })
    expect(result.listHeight / result.menuHeight).toBeGreaterThan(font === 16 ? 0.75 : 0.5)
    for (const button of result.initial) expect(button.height).toBeGreaterThanOrEqual(44)
    if (width === 620) expect(Math.abs(result.initial[1].top-result.initial[2].top)).toBeLessThan(1)
    if (font === 32 && width === 390) expect(result.initial[2].top).toBeGreaterThan(result.initial[1].top)
  }, 30_000)
  it.each(['idle', 'playing', 'paused'])('captures the %s viewport without loading feedback invalidating its layout snapshot', async status => {
    const frame = join(directory, 'capture.html')
    writeFileSync(frame, `<!doctype html><html><meta charset="utf-8"><style>body{margin:0}${css}
      .doc-header{position:fixed;inset:0 0 auto;height:65px;background:white;z-index:2}
      .document-view{margin-top:80px}article{font:20px/1.7 Arial;margin:0 30px}p{margin-bottom:24px}
      </style><div class="app-reader"><header class="doc-header"><div id="controls"></div></header>
      <main class="document-view"><article>${Array.from({length:40},(_,i)=>`<p>Passage ${i}. A saved book has a readable sentence with several words.</p>`).join('')}</article></main></div>
      <script>${bundle.replace(/<\/script/gi, '<\\/script')}</script><script>
      (async()=>{await ReaderControls.mount('en',true,${JSON.stringify(status)},false,${JSON.stringify('unmapped')});while(!window.captureReady)await new Promise(requestAnimationFrame);scrollTo(0,700);await new Promise(requestAnimationFrame);${status === 'idle' ? "document.querySelector('.reader-passage-action').click()" : "document.querySelector('.audio-menu-btn').click();while(!document.querySelector('.audio-passage-here'))await new Promise(requestAnimationFrame);document.querySelector('.audio-passage-here').click()"}})();</script>`)
    const result = await runBrowserFixture(directory, frame, 620)
    expect(result.status, JSON.stringify(result)).toBe('captured')
    expect(result.current).toBe(true)
    expect(result.target.status, JSON.stringify(result)).toBe('resolved')
    expect(result.point.segmentIndex).toBeGreaterThan(0)
  }, 30_000)

  it.each(['pointer', 'keyboard'])('keeps the chosen passage through %s menu focus and native selection collapse', async method => {
    const frame = join(directory, 'selection.html')
    writeFileSync(frame, `<!doctype html><html><meta charset="utf-8"><style>${css}article{margin:80px 30px;font:20px/1.6 Arial}</style>
      <div class="app-reader"><header class="doc-header"><div id="controls"></div></header><main class="document-view"><article><p>First paragraph.</p><p>Selected readable passage here.</p><p>A parenthetical (with an abbreviation.).</p></article></main></div>
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

  it.each([{width:320,font:16,locale:'en'}, {width:390,font:32,locale:'ar'}])('keeps idle shortcut and bookmark feedback separate: %j', async ({width,font,locale}) => {
    const frame = join(directory, 'reader-feedback.html')
    writeFileSync(frame, `<!doctype html><html><meta charset="utf-8"><style>${css}html{font-size:${font}px}article{margin:80px 30px;font-size:1rem}</style>
      <div class="app-reader"><header class="doc-header"><div id="controls"></div></header><main class="document-view"><article>${'<p>A synthetic saved book passage for reading.</p>'.repeat(30)}</article></main><div class="reader-bookmark-notice">Bookmark saved</div></div>
      <script>${bundle.replace(/<\/script/gi, '<\\/script')}</script><script>
      (async()=>{await ReaderControls.mount(${JSON.stringify(locale)},true);while(!window.captureReady||!document.querySelector('.reader-passage-action'))await new Promise(requestAnimationFrame);
      scrollTo(0,600);const text=document.querySelectorAll('article p')[10].firstChild;document.getSelection().setBaseAndExtent(text,25,text,5);document.dispatchEvent(new Event('selectionchange'));
      await document.fonts.ready;await new Promise(requestAnimationFrame);await new Promise(requestAnimationFrame);
      const button=document.querySelector('.reader-passage-action'),r=button.getBoundingClientRect(),rail=document.querySelector('.reader-floating-actions').getBoundingClientRect(),notice=document.querySelector('.reader-bookmark-notice').getBoundingClientRect();
      parent.postMessage({type:'capture-result',result:{left:r.left,right:r.right,height:r.height,railTop:rail.top,noticeBottom:notice.bottom}},'*');})();</script>`)
    const result = await runBrowserFixture(directory, frame, width,
      resolve('src-tauri/target/play-anywhere-acceptance/ui-review', `reader-shortcut-${width}-${locale}.png`))
    expect(result.left).toBeGreaterThanOrEqual(0)
    expect(result.right).toBeLessThanOrEqual(width)
    expect(result.height).toBeGreaterThanOrEqual(44)
    expect(result.noticeBottom).toBeLessThanOrEqual(result.railTop)
  }, 30_000)

  it.each(['pointer', 'keyboard'])('plays a scrolled selection from the idle reader shortcut (%s)', async method => {
    const frame = join(directory, 'reader-shortcut.html')
    writeFileSync(frame, `<!doctype html><html><meta charset="utf-8"><style>${css}article{margin:80px 30px;font:20px/1.6 Arial}</style>
      <div class="app-reader"><header class="doc-header"><div id="controls"></div></header><main class="document-view"><article>${Array.from({length:30},(_,i)=>`<p>Passage ${i}. Selected readable words from the middle of this book.</p>`).join('')}</article></main></div>
      <script>${bundle.replace(/<\/script/gi, '<\\/script')}</script><script>
      (async()=>{await ReaderControls.mount('en',true);while(!window.captureReady||!document.querySelector('.reader-passage-action'))await new Promise(requestAnimationFrame);
      const p=document.querySelectorAll('article p')[12];p.scrollIntoView({block:'center'});await new Promise(requestAnimationFrame);
      const text=p.firstChild;document.getSelection().setBaseAndExtent(text,30,text,12);
      document.dispatchEvent(new Event('selectionchange'));await new Promise(requestAnimationFrame);
      const action=document.querySelector('.reader-passage-action');
      if(action.textContent!=='Play from selection')throw new Error('Selection shortcut did not update');
      if(${JSON.stringify(method)}==='keyboard')document.body.dispatchEvent(new KeyboardEvent('keydown',{key:'Tab',bubbles:true}));
      else action.dispatchEvent(new PointerEvent('pointerdown',{bubbles:true,pointerType:'mouse'}));
      action.focus();document.getSelection().removeAllRanges();document.dispatchEvent(new Event('selectionchange'));await new Promise(requestAnimationFrame);
      if(action.textContent!=='Play from selection')throw new Error('Focus changed the advertised action');
      action.click();})();</script>`)
    const result = await runBrowserFixture(directory, frame, 390)
    expect(result.status, JSON.stringify(result)).toBe('captured')
    expect(result.current).toBe(true)
    expect(result.target.status, JSON.stringify(result)).toBe('resolved')
    expect(result.point).toEqual({ format: 'html', segmentIndex: 12, offset: 12 })
    expect(result.target.chunkIndex).toBeGreaterThan(0)
  }, 30_000)

  it.each([false, true])('resolves imported chunker punctuation from the top control and passage popover (legacy=%s)', async legacy => {
    const frame = join(directory, 'imported.html')
    const text = 'A narrated clause with repeated words '.repeat(4).trim() + ',“Another clause follows without a source space”, ' +
      'A final long clause keeps the sentence above the splitting limit '.repeat(8).trim() + '. A parenthetical (with an abbreviation.). Next phrase.'
    writeFileSync(frame, `<!doctype html><html><meta charset="utf-8"><style>body{margin:0}${css}
      .doc-header{position:fixed;inset:0 0 auto;height:65px;background:white;z-index:2}
      .document-view{margin-top:80px}article{font:20px/1.7 Arial;margin:0 30px}p{margin-bottom:24px}
      </style><div class="app-reader"><header class="doc-header"><div id="controls"></div></header>
      <main class="document-view"><article><p>Opening passage.</p><p>${text}</p>${Array.from({length:30},(_,i)=>`<p>Later passage ${i}. A saved book continues here.</p>`).join('')}</article></main></div>
      <script>${bundle.replace(/<\/script/gi, '<\\/script')}</script><script>
      (async()=>{await ReaderControls.mount('en',true,${JSON.stringify(legacy ? 'playing' : 'idle')},${legacy});while(!window.captureReady)await new Promise(requestAnimationFrame);
      ${legacy ? "scrollTo(0,1000);await new Promise(requestAnimationFrame);document.querySelector('.audio-menu-btn').click();while(!document.querySelector('.audio-passage-here'))await new Promise(requestAnimationFrame);document.querySelector('.audio-passage-here').click()" : "document.querySelector('.audio-primary-btn').click()"}})();</script>`)
    const result = await runBrowserFixture(directory, frame, 620)
    expect(result.status, JSON.stringify(result)).toBe('captured')
    expect(result.current).toBe(true)
    expect(result.target.status, JSON.stringify(result)).toBe('resolved')
    expect(result.target.chunkId).toBe('saved-' + result.target.chunkIndex)
    if (legacy) expect(result.target.chunkIndex).toBeGreaterThan(0)
  }, 30_000)

})
