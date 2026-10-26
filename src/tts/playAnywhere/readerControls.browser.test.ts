// @vitest-environment node
import {
  mkdtempSync,
  readFileSync,
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
  browser,
  runBrowserFixture,
} from './browserCaptureFixture'
import {
  afterAll,
  beforeAll,
  describe,
  expect,
  it,
} from 'vitest'
let directory: string; let bundle: string; let css: string
beforeAll(async () => {
  directory = mkdtempSync(join(tmpdir(), 'papercut-reader-controls-'))
  const entry = join(directory, 'entry.tsx')
  writeFileSync(entry, `
    import ${JSON.stringify(resolve('src/index.css'))};
    import ${JSON.stringify(resolve('src/App.css'))};
    import React from ${JSON.stringify(resolve('node_modules/react/index.js'))};
    import {createRoot} from ${JSON.stringify(resolve('node_modules/react-dom/client.js'))};
    import {createPortal} from ${JSON.stringify(resolve('node_modules/react-dom/index.js'))};
    import {ReaderPassageAction} from ${JSON.stringify(resolve('src/tts/playAnywhere/ReaderPassageAction.tsx'))};
    import {AudioControls} from ${JSON.stringify(resolve('src/tts/components/AudioControls.tsx'))};
    import {DocumentViewer} from ${JSON.stringify(resolve('src/components/DocumentViewer/DocumentViewer.tsx'))};
    import ${JSON.stringify(resolve('src/components/AppSettings/AppSettings.css'))};
    import {HtmlViewer} from ${JSON.stringify(resolve('src/viewers/HtmlViewer.tsx'))};
    import i18n from ${JSON.stringify(resolve('src/i18n/index.ts'))};
    import ${JSON.stringify(resolve('src/components/DocumentViewer/DocumentViewer.css'))};
    import {usePlayAnywhereReader} from ${JSON.stringify(resolve('src/tts/playAnywhere/usePlayAnywhereReader.ts'))};
    import {bindSelectionIntent} from ${JSON.stringify(resolve('src/tts/playAnywhere/selectionTarget.ts'))};
    import {readerTextViewport} from ${JSON.stringify(resolve('src/tts/playAnywhere/htmlTextTarget.ts'))};
    import {createTextTargetPreparation} from ${JSON.stringify(resolve('src/tts/playAnywhere/prepareTextTarget.ts'))};
    import {chunkAudiobookSaveHtmlWithSpans} from ${JSON.stringify(resolve('src/tts/utils/text.ts'))};
    export const viewport = readerTextViewport;
    export async function mountHeader(locale, tools=false, playbackStatus='idle', savedAudio=true, missingSetup=false) {
      await i18n.changeLanguage(locale);
      document.documentElement.dir=locale==='ar'?'rtl':'ltr';
      const noop=()=>{};
      const saved={id:'saved',documentUrl:'fixture.html',title:'Fixture',voice:'af_heart',speed:1,modelId:'kokoro',textPreprocessor:'none',dtype:'int8',savedAt:0,chunks:2};
      function Reader() {
        const [status, setStatus] = React.useState(playbackStatus);
        React.useEffect(() => {
          window.setReaderStatus = setStatus;
          return () => { delete window.setReaderStatus; };
        }, []);
        const active = ['playing','paused','loading'].includes(status);
        return <DocumentViewer
        url="fixture.html" content={tools?'<p>Synthetic reader text remains readable while its tools stay compact.</p>'.repeat(60):'<p>Synthetic header layout.</p>'}
        className={active?'app-audio-floating':''}
        floatingControls={tools && savedAudio && !active ? <ReaderPassageAction pending={false} onSnapshotSelection={()=>false} onPlayFromHere={noop} onPlayFromSelection={noop}/> : undefined}
        onClose={()=>{window.backRequested=true}}
        appControls={<button className="app-settings-btn" aria-label="Settings">⚙</button>}
        headerControls={<AudioControls
          audiobookState={{status:'saved',complete:true,cachedChunks:2,totalChunks:2}}
          canPlayAudiobook={savedAudio} canSaveAudiobook={false} canSkipBackward={false} canSkipForward
          modelSetupLabel={missingSetup?i18n.t('tts.controls.modelRequired'):null} saveInProgress={false}
          onManageSave={noop} onPause={noop} onRead={noop} onResume={noop} onSelectSavedAudiobook={noop}
          onJumpToChunk={noop} onPlaybackRateChange={noop} onOpenAudioSetup={()=>{window.setupRequested=true}} onSave={noop}
          onSkipBackward={noop} onSkipForward={noop} onStop={noop} onWordHighlightEnabledChange={noop}
          playbackRate={1} savedAudiobooks={savedAudio?[saved]:[]} selectedAudiobookId="saved" wordHighlightEnabled
          ttsState={{status,message:status==='error'?'Synthetic audio failure that remains visible for recovery.':'',chunksTotal:2,chunksPlayed:0,currentChunkIndex:null,currentChunkProgress:0,currentChunkDuration:0,chunkSummaries:[]}}
          playAnywhere={{browsing:false,pending:false,feedback:null,onPlayFromHere:()=>{window.playRequested=true},onPlayFromBeginning:noop,onReturnToAudio:noop}} />}
      />;
      }
      createRoot(document.getElementById('header-fixture')).render(<Reader/>);
    }
    export async function mount(locale,captureMode=false,playbackStatus='idle',legacy=false,priorFeedback=null) {
      await i18n.changeLanguage(locale);document.documentElement.dir=locale==='ar'?'rtl':'ltr';
      const noop=()=>{};
      const shell=document.querySelector('.app-reader'),surface=shell.querySelector('.document-view'),content=surface.innerHTML;
      if(captureMode)surface.replaceChildren();
      const chunks=captureMode?chunkAudiobookSaveHtmlWithSpans(content).map((chunk,i)=>legacy?{id:'saved-'+i,text:chunk.text}:{...chunk,id:'saved-'+i}):[];
      const preparation=createTextTargetPreparation({documentUrl:'/user-uploads/ab.html',docContent:content,chunks,getChunks:async()=>chunks,getPdfSegments:async()=>[],getPdfPageText:async()=>{throw new Error('HTML must not read PDF')}});
      const playFromHere=async(api)=>{
        const result=await api.captureViewport();
        const target=await preparation.resolve('/user-uploads/ab.html',result);
        await new Promise(requestAnimationFrame);
        parent.postMessage({type:'capture-result',result:{status:result.status,point:result.point,reason:result.reason,current:result.status==='captured'&&result.isCurrent(),target:{status:target.status,reason:target.reason,chunkIndex:target.chunkIndex,chunkId:target.chunkId}}},'*');
        return target.status==='resolved'?{status:'dispatched',started:Promise.resolve('started')}:target;
      };
      function Controls(){
        const [sample,setSample]=React.useState({index:18,status:'paused',pending:false,feedback:'unmapped',message:'',highlight:true,rate:1});
        React.useEffect(()=>{window.updateControls=patch=>setSample(value=>({...value,...patch}));return()=>{delete window.updateControls}},[]);
        const reader=usePlayAnywhereReader({documentUrl:'fixture',active:!captureMode||playbackStatus!=='idle',playFromHere,playFromBeginning:async()=>({status:'dispatched',started:Promise.resolve('started')}),commandSignal:()=>new AbortController().signal});
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
        onJumpToChunk={noop} onPlaybackRateChange={rate=>{window.rateRequested=rate;setSample(value=>({...value,rate}))}} onOpenAudioSetup={noop} onSave={noop}
        onSkipBackward={noop} onSkipForward={noop} onStop={noop} onWordHighlightEnabledChange={highlight=>{window.highlightRequested=highlight;setSample(value=>({...value,highlight}))}}
        playbackRate={sample.rate} savedAudiobooks={[]} selectedAudiobookId={null} wordHighlightEnabled={sample.highlight}
        ttsState={{status:captureMode?playbackStatus:sample.status,message:sample.message,chunksTotal:captureMode?2:184,chunksPlayed:0,currentChunkIndex:captureMode?0:sample.index,currentChunkProgress:0,currentChunkDuration:0,chunkSummaries:captureMode?[]:Array.from({length:184},(_,index)=>({index,chunkId:'fixture-'+index,textPreview:'Synthetic narrated passage '+(index+1)+' with several readable words.'}))}}
        playAnywhere={captureMode?{...reader,feedback:reader.feedback??priorFeedback,onPlayFromHere:reader.playFromHere,onPlayFromBeginning:reader.playFromBeginning,onReturnToAudio:reader.returnToAudio,onSnapshotSelection:reader.snapshotSelection,onPlayFromSelection:reader.playFromSelection,onSelectionMenuClose:reader.clearSelection}:{browsing:true,pending:sample.pending,feedback:sample.feedback,onPlayFromHere:noop,onPlayFromBeginning:noop,onReturnToAudio:noop,onPlayFromSelection:noop,onSnapshotSelection:()=>false}} /></>;
      }
      createRoot(document.getElementById('controls')).render(<Controls/>);
    }
  `)
  const output = await build({
    configFile: false,
    logLevel: 'silent',
    define: { 'process.env.NODE_ENV': JSON.stringify('production') },
    resolve: {
      alias: {
        'react/jsx-dev-runtime': resolve('node_modules/react/jsx-dev-runtime.js'),
        'react/jsx-runtime': resolve('node_modules/react/jsx-runtime.js'),
      },
    },
    esbuild: {
      jsx: 'automatic',
      jsxDev: false,
    },
    build: {
      write: false,
      minify: false,
      lib: {
        entry,
        name: 'ReaderControls',
        formats: ['iife'],
      },
    },
  })
  const result = Array.isArray(output) ? output[0] : output
  if (!('output' in result)) {
    throw new Error('Expected browser fixture bundle')
  }
  bundle = result.output.filter(item => item.type === 'chunk').map(item => item.code).join('\n')
  css = result.output.filter(item => item.type === 'asset' && item.fileName.endsWith('.css')).map(item => item.type === 'asset' ? String(item.source) : '').join('\n')
}, 30_000)
afterAll(() => {
  if (directory) {
    rmSync(directory, {
      recursive: true,
      force: true,
    })
  }
})
describe.skipIf(!browser)('Reader controls in Chromium', () => {
  it.each([
    {
      width: 620,
      locale: 'en',
      setup: true,
    },
    {
      width: 620,
      locale: 'en',
      setup: false,
    },
    {
      width: 320,
      locale: 'ar',
      setup: true,
    },
  ])('uses green/warning floppy Save states with setup precedence without blocking imported playback: %j', async ({ width, locale, setup }) => {
    const frame = join(directory, 'save-states.html')
    writeFileSync(frame, `<!doctype html><html data-theme="${locale === 'ar' ? 'dark' : 'light'}"><meta charset="utf-8"><style>${css}body{margin:0}</style>
      <div id="header-fixture"></div><script>${bundle.replace(/<\/script/gi, '<\\/script')}</script><script>
      (async()=>{
        await ReaderControls.mountHeader(${JSON.stringify(locale)},false,'idle',true,${setup});
        while(!document.querySelector('.audio-menu-btn'))await new Promise(requestAnimationFrame);
        const save=document.querySelector('.audio-compact-row > button:last-child');
        const more=document.querySelector('.audio-menu-btn');
        const probe=document.createElement('span');
        probe.style.backgroundColor=${setup}?'var(--surface-warning)':'var(--surface-success)';
        document.body.append(probe);
        const expectedBackground=getComputedStyle(probe).backgroundColor;
        probe.remove();
        const result={
          setup:save.classList.contains('audio-setup-required'),saved:save.classList.contains('audio-save-complete'),
          tonal:getComputedStyle(save).backgroundColor===expectedBackground,
          floppy:save.querySelector('svg path')?.getAttribute('d')?.startsWith('M5 3h12'),
          warning:getComputedStyle(save,'::after').content===(${setup}?'"!"':'none'),
          label:save.getAttribute('aria-label'),title:save.title,disabled:save.disabled,
          iconOnly:more.textContent.trim()==='' && Boolean(more.querySelector('svg[aria-hidden="true"]')),
          named:Boolean(more.getAttribute('aria-label')) && more.title===more.getAttribute('aria-label'),
        };
        save.click();
        document.querySelector('.audio-primary-btn').click();
        result.setupRequested=Boolean(window.setupRequested);
        result.playRequested=Boolean(window.playRequested);
        parent.postMessage({type:'capture-result',result},'*');
      })();</script>`)
    const result = await runBrowserFixture(directory, frame, width,
      resolve('src-tauri/target/play-anywhere-acceptance/ui-review', `save-${width}-${locale}-${setup}.png`))
    expect(result).toMatchObject({
      setup,
      saved: !setup,
      tonal: true,
      disabled: !setup,
      setupRequested: setup,
      playRequested: true,
      iconOnly: true,
      named: true,
      floppy: true,
      warning: true,
    })
    expect(result.label).toBe(result.title)
    if (locale === 'en' && setup) {
      expect(result.label).toContain('Saved audio can still play.')
    }
  }, 30_000)

  it.each([
    {
      width: 320,
      font: 16,
      locale: 'en',
    },
    {
      width: 453,
      font: 16,
      locale: 'en',
    },
    {
      width: 620,
      font: 16,
      locale: 'ar',
    },
    {
      width: 900,
      font: 32,
      locale: 'en',
    },
    {
      width: 453,
      font: 32,
      locale: 'ar',
    },
    {
      width: 320,
      font: 32,
      locale: 'en',
    },
  ])('keeps Back and saved-audio controls distinct in the complete reader header: %j', async ({ width, font, locale }) => {
    const frame = join(directory, 'full-header.html')
    writeFileSync(frame, `<!doctype html><html><meta charset="utf-8"><style>${css}html{font-size:${font}px}body{margin:0}</style>
      <div id="header-fixture"></div><script>${bundle.replace(/<\/script/gi, '<\\/script')}</script><script>
      (async () => {
        await ReaderControls.mountHeader(${JSON.stringify(locale)});
        while (!document.querySelector('.audio-saved-versions-btn')) await new Promise(requestAnimationFrame);
        await document.fonts.ready;
        await new Promise(requestAnimationFrame);
        const header = document.querySelector('.doc-header');
        const buttons = [...header.querySelectorAll('button')];
        const box = element => {
          const r = element.getBoundingClientRect();
          return {left:r.left, right:r.right, top:r.top, bottom:r.bottom};
        };
        const boxes = buttons.map(box);
        const overlaps = (a, b) => a.left < b.right - .5 && a.right > b.left + .5 && a.top < b.bottom - .5 && a.bottom > b.top + .5;
        const visible = buttons.every(button => {
          const r = button.getBoundingClientRect();
          return document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2)?.closest('button') === button;
        });
        const overlap = boxes.some((a, index) => boxes.slice(index + 1).some(b => overlaps(a, b)));
        const badgeOverlap = overlaps(box(document.querySelector('.audio-saved-count')), box(document.querySelector('.back-button')));
        document.querySelector('.back-button').click();
        parent.postMessage({type:'capture-result', result:{boxes, visible, overlap, badgeOverlap, back:window.backRequested, scrollWidth:document.documentElement.scrollWidth}}, '*');
      })();</script>`)
    const result = await runBrowserFixture(directory, frame, width,
      resolve('src-tauri/target/play-anywhere-acceptance/ui-review', `header-${width}-${font}-${locale}.png`))
    expect(result).toMatchObject({
      visible: true,
      overlap: false,
      badgeOverlap: false,
      back: true,
    })
    expect(result.scrollWidth).toBeLessThanOrEqual(width)
    for (const box of result.boxes) {
      expect(box.left).toBeGreaterThanOrEqual(-0.5)
      expect(box.right).toBeLessThanOrEqual(width + 0.5)
    }
  }, 30_000)

  it.each([
    {
      width: 320,
      font: 16,
      locale: 'en',
    },
    {
      width: 320,
      font: 16,
      locale: 'ar',
    },
    {
      width: 620,
      font: 16,
      locale: 'en',
    },
    {
      width: 900,
      font: 32,
      locale: 'en',
    },
  ])('groups the switch and compact rate with consistent Options styling: %j', async ({ width, font, locale }) => {
    const frame = join(directory, 'compact-options.html')
    writeFileSync(frame, `<!doctype html><html data-theme="${locale === 'ar' ? 'dark' : 'light'}"><meta charset="utf-8"><style>${css}html{font-size:${font}px}body{margin:0}</style>
      <div class="app-reader"><header class="doc-header"><div id="controls"></div></header><main class="document-view"><p>Fixture text.</p></main></div>
      <script>${bundle.replace(/<\/script/gi, '<\\/script')}</script><script>
      (async () => {
        await ReaderControls.mount(${JSON.stringify(locale)});
        while (!document.querySelector('.audio-menu-btn')) await new Promise(requestAnimationFrame);
        document.querySelector('.audio-menu-btn').click();
        while (!document.querySelector('.audio-menu-views')) await new Promise(requestAnimationFrame);
        const views = [...document.querySelectorAll('.audio-menu-views button')];
        const icons = views.every(button => button.querySelector('svg[aria-hidden="true"]') && button.textContent.trim());
        const sameOptionsIcon=document.querySelector('.audio-menu-btn svg').innerHTML===views[1].querySelector('svg').innerHTML;
        views[1].click();
        while (!document.querySelector('.audio-menu-rate')) await new Promise(requestAnimationFrame);
        await document.fonts.ready;
        const toggle = document.querySelector('.audio-word-highlight-toggle');
        const speed = document.querySelector('.audio-menu-rate');
        const menu = document.querySelector('.audio-chunk-menu');
        menu.scrollTop = menu.scrollHeight;
        await new Promise(requestAnimationFrame);
        const a = toggle.getBoundingClientRect(), b = speed.getBoundingClientRect();
        const input = toggle.querySelector('input');
        const name = input.getAttribute('aria-label');
        toggle.click();
        while (input.checked) await new Promise(requestAnimationFrame);
        const changed = window.highlightRequested === false && input.getAttribute('aria-label') === name;
        input.focus();
        const focused = document.activeElement === input;
        toggle.click();
        while (!input.checked) await new Promise(requestAnimationFrame);
        parent.postMessage({type:'capture-result',result:{icons, sameOptionsIcon, changed, focused, name, rate:speed.textContent, rateName:speed.getAttribute('aria-label'), sameRow:Math.abs(a.top-b.top)<1, heights:[a.height,b.height], bounds:[a.left,a.right,b.left,b.right], checked:input.checked, terminalBorder:getComputedStyle(toggle.closest('.audio-chunk-menu-header')).borderBottomWidth, moreName:document.querySelector('.audio-menu-btn').getAttribute('aria-label'), iconOnly:document.querySelector('.audio-menu-btn').textContent.trim() === ''}}, '*');
      })();</script>`)
    const axPath = resolve('src-tauri/target/play-anywhere-acceptance/ui-review', `options-${width}-${font}-${locale}-ax.json`)
    const result = await runBrowserFixture(directory, frame, width,
      resolve('src-tauri/target/play-anywhere-acceptance/ui-review', `options-${width}-${font}-${locale}.png`), axPath)
    expect(result).toMatchObject({
      icons: true,
      sameOptionsIcon: true,
      changed: true,
      focused: true,
      rate: '1x',
      sameRow: true,
      checked: true,
    })
    expect(result.rateName).toBeTruthy()
    expect(result.terminalBorder).toBe('0px')
    expect(result.moreName).toBeTruthy()
    expect(result.iconOnly).toBe(true)
    expect(Math.min(...result.heights)).toBeGreaterThanOrEqual(44)
    expect(Math.min(...result.bounds)).toBeGreaterThanOrEqual(0)
    expect(Math.max(...result.bounds)).toBeLessThanOrEqual(width)
    const tree = JSON.parse(readFileSync(axPath, 'utf8'))
    const control = tree.nodes.find((node: { ignored: boolean;
      role?: { value: string } }) => !node.ignored && node.role?.value === 'switch')
    expect(control?.name?.value).toBe(result.name)
    expect(control?.properties?.find((property: { name: string }) => property.name === 'checked')?.value?.value).toBe('true')
  }, 30_000)

  it.each([
    {
      width: 320,
      font: 16,
      locale: 'en',
      status: 'idle',
      saved: true,
    },
    {
      width: 320,
      font: 32,
      locale: 'en',
      status: 'idle',
      saved: true,
    },
    {
      width: 320,
      font: 16,
      locale: 'en',
      status: 'paused',
      saved: true,
    },
    {
      width: 1600,
      font: 16,
      locale: 'en',
      status: 'paused',
      saved: true,
    },
    {
      width: 1600,
      font: 16,
      locale: 'ar',
      status: 'paused',
      saved: true,
    },
    {
      width: 900,
      font: 32,
      locale: 'en',
      status: 'paused',
      saved: true,
    },
    {
      width: 390,
      font: 32,
      locale: 'ar',
      status: 'loading',
      saved: true,
    },
    {
      width: 320,
      font: 32,
      locale: 'en',
      status: 'error',
      saved: true,
    },
    {
      width: 390,
      font: 16,
      locale: 'ar',
      status: 'idle',
      saved: false,
    },
  ])('keeps real reader tools and bookmark feedback clear of playback: %j', async ({ width, font, locale, status, saved }) => {
    const frame = join(directory, 'reader-capsule.html')
    writeFileSync(frame, `<!doctype html><html data-theme="${locale === 'ar' ? 'dark' : 'light'}"><meta charset="utf-8"><style>${css}html{font-size:${font}px}body{margin:0}</style>
      <div id="header-fixture"></div><script>${bundle.replace(/<\/script/gi, '<\\/script')}</script><script>
      (async () => {
        // Each case uses the same synthetic file origin; an earlier saved bookmark must not change this action into a menu.
        localStorage.clear();
        await ReaderControls.mountHeader(${JSON.stringify(locale)}, true, ${JSON.stringify(status)}, ${saved});
        while (!window.setReaderStatus || !document.querySelector('.document-html-content p')) await new Promise(requestAnimationFrame);
        await document.fonts.ready;
        await new Promise(requestAnimationFrame);
        scrollTo(0, 600);
        while (!document.querySelector('.reader-floating-actions')) await new Promise(requestAnimationFrame);
        document.querySelector('.reader-bookmark-btn').click();
        while (!document.querySelector('.reader-bookmark-notice')) await new Promise(requestAnimationFrame);
        await new Promise(requestAnimationFrame);
        await new Promise(requestAnimationFrame);
        const shell = document.querySelector('.app-reader');
        const rail = document.querySelector('.reader-floating-actions');
        const rect = element => {
          const r = element.getBoundingClientRect();
          return {left:r.left, right:r.right, top:r.top, bottom:r.bottom, width:r.width, height:r.height};
        };
        const tools = [...rail.querySelectorAll('button')];
        const boxes = tools.map(rect);
        const dock = document.querySelector('.audio-playback-dock');
        const result = {
          rail:rect(rail), boxes, notice:rect(document.querySelector('.reader-bookmark-notice')),
          dock:rect(dock), layout:shell.dataset.readerToolsLayout, count:tools.length, named:tools.every(button => button.getAttribute('aria-label')),
          reachable:tools.every(button => {
            const r = button.getBoundingClientRect();
            return document.elementFromPoint(r.x+r.width/2, r.y+r.height/2)?.closest('button') === button;
          }),
          mutedTop:getComputedStyle(rail.querySelector('.scroll-top-btn')).backgroundColor === 'rgba(0, 0, 0, 0)',
          measured:parseFloat(shell.style.getPropertyValue('--reader-actions-height')),
          viewport:ReaderControls.viewport(shell),
        };
        if (${saved} && ${JSON.stringify(status)} !== 'idle') {
          window.setReaderStatus('idle');
          while (!document.querySelector('.reader-passage-action')) await new Promise(requestAnimationFrame);
          await new Promise(requestAnimationFrame);
          await new Promise(requestAnimationFrame);
          result.stoppedIconOnly = document.querySelector('.reader-passage-action').textContent === '';
          window.setReaderStatus(${JSON.stringify(status)});
          while (document.querySelector('.audio-floating-playback') === null && ${JSON.stringify(status)} !== 'error') await new Promise(requestAnimationFrame);
          await new Promise(requestAnimationFrame);
          await new Promise(requestAnimationFrame);
        }
        parent.postMessage({type:'capture-result', result}, '*');
      })();</script>`)
    const result = await runBrowserFixture(directory, frame, width,
      resolve('src-tauri/target/play-anywhere-acceptance/ui-review', `capsule-${width}-${font}-${locale}-${status}-${saved}.png`))
    expect(result).toMatchObject({
      named: true,
      reachable: true,
      mutedTop: true,
    })
    expect(result.count).toBe(saved && ['idle', 'error'].includes(status) ? 3 : 2)
    expect(result.rail.left).toBeGreaterThanOrEqual(0)
    expect(result.rail.right).toBeLessThanOrEqual(width)
    expect(result.rail.top).toBeGreaterThanOrEqual(0)
    expect(result.notice.top).toBeGreaterThanOrEqual(0)
    expect(result.notice.bottom).toBeLessThanOrEqual(result.rail.top)
    expect(result.measured).toBeCloseTo(result.rail.height, 1)
    if (font === 16) {
      expect(result.viewport).not.toBeNull()
    }
    if (result.viewport) {
      for (const control of [result.rail, result.notice, result.dock]) {
        if (control.height > 0) {
          expect(result.viewport.bottom <= control.top || result.viewport.top >= control.bottom).toBe(true)
        }
      }
    }
    if (result.dock.height > 0) {
      if (result.layout === 'beside') {
        expect(result.rail.bottom).toBeCloseTo(result.dock.bottom, 1)
        expect(result.rail.right <= result.dock.left || result.rail.left >= result.dock.right).toBe(true)
      } else {
        expect(result.layout).toBe('stacked')
        expect(result.rail.bottom).toBeLessThanOrEqual(result.dock.top)
        expect(locale === 'ar' ? result.rail.left : result.rail.right).toBeCloseTo(locale === 'ar' ? result.dock.left : result.dock.right, 1)
      }
    }
    if (width === 1600 && font === 16) {
      expect(result.layout).toBe('beside')
    }
    if (saved && status !== 'idle') {
      expect(result.stoppedIconOnly).toBe(true)
    }
    for (const box of result.boxes) {
      expect(box.width).toBeGreaterThanOrEqual(44)
      expect(box.height).toBeGreaterThanOrEqual(44)
    }
    if (font === 16) {
      expect(Math.max(...result.boxes.map((box: { top: number }) => box.top)) - result.boxes[0].top).toBeLessThan(1)
    }
  }, 30_000)

  it.each(['en', 'ar'])('recalculates shared-edge layout after live resize and cleans up on Stop: %s', async locale => {
    const child = join(directory, 'live-layout-child.html')
    writeFileSync(child, `<!doctype html><html><meta charset="utf-8"><style>${css}body{margin:0}</style>
      <div id="header-fixture"></div><script>${bundle.replace(/<\/script/gi, '<\\/script')}</script>
      <script>localStorage.clear();ReaderControls.mountHeader(${JSON.stringify(locale)},true,'paused');</script>`)
    const frame = join(directory, 'live-layout.html')
    writeFileSync(frame, `<!doctype html><html><style>body{margin:0}iframe{border:0;height:600px}</style>
      <iframe src="live-layout-child.html" width="1280"></iframe><script>
      (async()=>{
        const frame=document.querySelector('iframe');
        while(!frame.contentWindow?.setReaderStatus)await new Promise(requestAnimationFrame);
        const view=frame.contentWindow, doc=view.document;
        view.scrollTo(0,600);
        while(!doc.querySelector('.reader-floating-actions'))await new Promise(requestAnimationFrame);
        const layouts=[];
        for(const width of [1280,320,1280]){
          frame.width=width;
          for(let i=0;i<5;i++)await new Promise(requestAnimationFrame);
          const shell=doc.querySelector('.app-reader'), rail=doc.querySelector('.reader-floating-actions').getBoundingClientRect(), dock=doc.querySelector('.audio-playback-dock').getBoundingClientRect();
          layouts.push({width,mode:shell.dataset.readerToolsLayout,bottom:rail.bottom,playerBottom:dock.bottom,top:dock.top,left:rail.left,right:rail.right});
        }
        view.setReaderStatus('idle');
        while(!doc.querySelector('.reader-passage-action'))await new Promise(requestAnimationFrame);
        for(let i=0;i<3;i++)await new Promise(requestAnimationFrame);
        const shell=doc.querySelector('.app-reader');
        parent.postMessage({type:'capture-result',result:{layouts,modeAfterStop:shell.dataset.readerToolsLayout??null,offsetAfterStop:shell.style.getPropertyValue('--reader-playback-offset')}},'*');
      })();</script>`)
    const result = await runBrowserFixture(directory, frame, 1280)
    expect(result.layouts.map((layout: { mode: string }) => layout.mode)).toEqual(['beside', 'stacked', 'beside'])
    for (const layout of result.layouts) {
      expect(layout.left).toBeGreaterThanOrEqual(0)
      expect(layout.right).toBeLessThanOrEqual(layout.width)
      if (layout.mode === 'beside') {
        expect(layout.bottom).toBeCloseTo(layout.playerBottom, 1)
      } else {
        expect(layout.bottom).toBeLessThanOrEqual(layout.top)
      }
    }
    expect(result.modeAfterStop).toBeNull()
    expect(result.offsetAfterStop).toBe('0px')
  }, 30_000)

  it.each([{
    width:320,
    font:16,
    locale:'en',
  }, {
    width:390,
    font:32,
    locale:'ar',
  }, {
    width:1280,
    font:32,
    locale:'fr',
  }])('keeps browse/return controls usable outside the hidden fullscreen header: %j', async ({width,font,locale}) => {
    const frame = join(directory, 'frame.html')
    writeFileSync(frame, `<!doctype html><html><meta charset="utf-8"><style>html{font-size:${font}px}body{margin:0}${css}</style>
      <div class="app app-reader app-reader-pdf app-reader-pdf-fullscreen app-reader-pdf-toolbar-hidden"><header class="doc-header"><div id="controls"></div></header><main class="document-view">Reader</main></div>
      <script>${bundle.replace(/<\/script/gi, '<\\/script')}</script><script>
      (async()=>{try{await ReaderControls.mount(${JSON.stringify(locale)});while(!document.querySelector('.audio-playback-dock'))await new Promise(requestAnimationFrame);await document.fonts.ready;
      const dock=document.querySelector('.audio-playback-dock');const rect=e=>{const r=e.getBoundingClientRect();return {left:r.left,right:r.right,top:r.top,bottom:r.bottom,width:r.width,height:r.height}};
      const buttons=[...dock.querySelectorAll('button')].filter(button=>getComputedStyle(button).display!=='none');const visible=buttons.every(b=>{const r=b.getBoundingClientRect();return r.left>=0&&r.right<=innerWidth&&r.top>=0&&r.bottom<=innerHeight&&getComputedStyle(b).visibility==='visible'&&document.elementFromPoint(r.x+r.width/2,r.y+r.height/2)?.closest('button')===b});
      const browse=[...dock.querySelectorAll('.audio-return-control')].map(rect);const inlineReturn=Boolean(dock.querySelector('.audio-floating-playback > .audio-return-control'));
      parent.postMessage({type:'capture-result',result:{visible,browse,inlineReturn,extraStrip:Boolean(dock.querySelector('.audio-browse-actions')),dock:rect(dock),outsideHeader:!dock.closest('header'),feedback:dock.querySelector('[role=status]').textContent}},'*');
      }catch(e){parent.postMessage({type:'capture-result',result:{error:String(e.stack||e)}},'*')}})();</script>`)
    const result = await runBrowserFixture(directory, frame, width)
    expect(result.visible, JSON.stringify(result)).toBe(true)
    expect(result.outsideHeader).toBe(true); expect(result.feedback.length).toBeGreaterThan(0)
    expect(result.browse).toHaveLength(1)
    expect(result.inlineReturn).toBe(true); expect(result.extraStrip).toBe(false)
    for (const button of result.browse) {
      expect(button.height).toBeGreaterThanOrEqual(44)
    }
  }, 30_000)
  it.each([{
    width:320,
    font:16,
    locale:'en',
    theme:'light',
  }, {
    width:620,
    font:16,
    locale:'en',
    theme:'light',
  },
  {
    width:390,
    font:32,
    locale:'ar',
    theme:'dark',
  }, {
    width:1280,
    font:32,
    locale:'fr',
    theme:'light',
  }])('reflows passage commands and keeps the popover reachable: %j', async ({width,font,locale,theme}) => {
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
      for(const button of [...actions,menu.querySelector('.audio-word-highlight-toggle input')]){
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
    expect(result).toMatchObject({
      disabled: true,
      icons: true,
      escaped: true,
      focusVisible: true,
      inlineReturn: true,
      defaultChunks: true,
    })
    expect(result.listHeight / result.menuHeight).toBeGreaterThan(font === 16 ? 0.75 : 0.5)
    for (const button of result.initial) {
      expect(button.height).toBeGreaterThanOrEqual(44)
    }
    if (width === 620) {
      expect(Math.abs(result.initial[0].top-result.initial[1].top)).toBeLessThan(1)
      expect(Math.abs(result.initial[2].top-result.initial[3].top)).toBeLessThan(1)
      expect(result.initial[2].top).toBeGreaterThan(result.initial[0].top)
    }
    if (font === 32 && width === 390) {
      expect(result.initial[2].top).toBeGreaterThan(result.initial[1].top)
    }
  }, 30_000)
  it.each(['en', 'ar'])('keeps six usable transport targets together at 320px (%s)', async locale => {
    const frame = join(directory, 'compact-player.html')
    const theme = locale === 'ar' ? 'dark' : 'light'
    const axPath = resolve('src-tauri/target/play-anywhere-acceptance/ui-review', `compact-320-${locale}-ax.json`)
    writeFileSync(frame, `<!doctype html><html data-theme="${theme}"><meta charset="utf-8"><style>${css}body{margin:0}</style>
      <div class="app-reader"><header class="doc-header"><div id="controls"></div></header><main class="document-view"><article>${'<p>A synthetic book retains its current passage.</p>'.repeat(40)}</article></main></div>
      <script>${bundle.replace(/<\/script/gi, '<\\/script')}</script><script>
      (async () => {
        await ReaderControls.mount(${JSON.stringify(locale)});
        while (!document.querySelector('.audio-return-control')) await new Promise(requestAnimationFrame);
        await document.fonts.ready;
        const toolbar = document.querySelector('.audio-floating-playback');
        const buttons = [...toolbar.querySelectorAll('button')].filter(button => getComputedStyle(button).display !== 'none');
        const boxes = buttons.map(button => {
          const box = button.getBoundingClientRect();
          return { top: box.top, left: box.left, right: box.right, width: box.width, height: box.height };
        });
        const luminance = color => {
          const rgb = color.match(/[0-9.]+/g).slice(0, 3).map(Number).map(value => {
            const n = value / 255;
            return n <= 0.04045 ? n / 12.92 : Math.pow((n + 0.055) / 1.055, 2.4);
          });
          return rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722;
        };
        const contrasts = buttons.filter(button => !button.disabled).map(button => {
          const style = getComputedStyle(button);
          const a = luminance(style.color), b = luminance(style.backgroundColor);
          return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
        });
        const trigger = document.querySelector('.audio-menu-btn');
        trigger.click();
        while (!document.querySelector('.audio-menu-views')) await new Promise(requestAnimationFrame);
        document.querySelectorAll('.audio-menu-views button')[1].click();
        while (!document.querySelector('.audio-menu-rate')) await new Promise(requestAnimationFrame);
        const speed = document.querySelector('.audio-menu-rate');
        const speedVisible = getComputedStyle(speed).display !== 'none';
        speed.click();
        document.dispatchEvent(new KeyboardEvent('keydown', {key:'Escape', bubbles:true}));
        while (document.querySelector('.audio-chunk-menu')) await new Promise(requestAnimationFrame);
        window.updateControls({feedback:null});
        while (document.querySelector('.audio-target-notice').textContent) await new Promise(requestAnimationFrame);
        parent.postMessage({type:'capture-result', result:{boxes, contrasts, speedVisible, rate:window.rateRequested}}, '*');
      })();
      </script>`)
    const result = await runBrowserFixture(directory, frame, 320,
      resolve('src-tauri/target/play-anywhere-acceptance/ui-review', `compact-320-${locale}.png`), axPath)
    expect(result.boxes).toHaveLength(6)
    for (const box of result.boxes) {
      expect(Math.abs(box.top - result.boxes[0].top)).toBeLessThan(1)
      expect(box.width).toBeGreaterThanOrEqual(44)
      expect(box.height).toBeGreaterThanOrEqual(44)
      expect(box.left).toBeGreaterThanOrEqual(0)
      expect(box.right).toBeLessThanOrEqual(320)
    }
    expect(Math.min(...result.contrasts)).toBeGreaterThanOrEqual(3)
    expect(result).toMatchObject({
      speedVisible: true,
      rate: 1.25,
    })
    const tree = JSON.parse(readFileSync(axPath, 'utf8'))
    const buttons = tree.nodes.filter((node: { ignored: boolean;
      role?: { value: string } }) => !node.ignored && node.role?.value === 'button')
    expect(buttons.every((node: { name?: { value: string } }) => Boolean(node.name?.value))).toBe(true)
    expect(buttons.some((node: { name?: { value: string } }) => node.name?.value === (locale === 'en' ? 'Return to audio' : 'العودة إلى موضع الصوت'))).toBe(true)
  }, 30_000)

  it('keeps a reader-browsed chunk stable while playback advances and restores centering after reopening', async () => {
    const frame = join(directory, 'stable-list.html')
    writeFileSync(frame, `<!doctype html><html><meta charset="utf-8"><style>${css}body{margin:0}</style>
      <div class="app-reader"><header class="doc-header"><div id="controls"></div></header><main class="document-view"><article><p>Synthetic book.</p></article></main></div>
      <script>${bundle.replace(/<\/script/gi, '<\\/script')}</script><script>
      (async()=>{const settle=()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));

      await ReaderControls.mount('en');
      while(!window.updateControls||!document.querySelector('.audio-menu-btn'))await settle();

      const trigger=document.querySelector('.audio-menu-btn');
      trigger.click();
      while(!document.querySelector('.audio-chunk-list'))await settle();

      await settle();
      const initial=document.querySelector('.audio-chunk-list').scrollTop;

      window.updateControls({index:19});
      await settle();

      const list=document.querySelector('.audio-chunk-list'),followed=list.scrollTop!==initial;

      list.dispatchEvent(new WheelEvent('wheel',{bubbles:true,deltaY:400}));
      list.scrollTop=2200;
      await settle();
      await settle();

      const before=list.scrollTop;
      window.updateControls({index:20});
      await settle();

      const stable=list.scrollTop===before;

      const views=document.querySelectorAll('.audio-menu-views button');
      views[1].click();
      await settle();
      views[0].click();
      await settle();

      const restored=document.querySelector('.audio-chunk-list').scrollTop===before;

      document.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true}));
      await settle();
      trigger.click();
      await settle();

      parent.postMessage({type:'capture-result',result:{followed,stable,restored,recentered:document.querySelector('.audio-chunk-list').scrollTop!==before}},'*');
      })();
      </script>`)
    expect(await runBrowserFixture(directory, frame)).toEqual({
      followed: true,
      stable: true,
      restored: true,
      recentered: true,
    })
  }, 30_000)

  it.each([{
    width:320,
    locale:'en',
    theme:'light',
  }, {
    width:620,
    locale:'ar',
    theme:'dark',
  }])('shows compact Return and integrated pending/recovery feedback: %j', async ({width,locale,theme}) => {
    const frame = join(directory, 'feedback.html')
    writeFileSync(frame, `<!doctype html><html data-theme="${theme}"><meta charset="utf-8"><style>${css}body{margin:0}</style>
      <div class="app-reader"><header class="doc-header"><div id="controls"></div></header><main class="document-view"><article>${'<p>A synthetic book continues while checking audio.</p>'.repeat(30)}</article></main></div>
      <script>${bundle.replace(/<\/script/gi, '<\\/script')}</script><script>
      (async()=>{const until=async check=>{while(!check())await new Promise(requestAnimationFrame)};
      await ReaderControls.mount(${JSON.stringify(locale)});
      while(!window.updateControls||!document.querySelector('.audio-return-control'))await new Promise(requestAnimationFrame);

      const control=document.querySelector('.audio-return-control'),label=control.querySelector('.audio-return-label'),labelHidden=getComputedStyle(label).display==='none',name=control.getAttribute('aria-label');

      const ordinary=document.querySelector('.audio-rate-control');
      const tonal=getComputedStyle(ordinary).backgroundColor===getComputedStyle(control).backgroundColor;

      window.updateControls({status:'loading',pending:true,feedback:null,message:'Checking saved audio...'});
      await until(()=>document.querySelector('.audio-target-notice')?.textContent==='Checking saved audio...');

      const notice=document.querySelector('.audio-target-notice');
      const sameSurface=notice.parentElement===document.querySelector('.audio-floating-playback').parentElement;

      const spinner=Boolean(notice.querySelector('.spinner[aria-hidden=true]'));
      const status=notice.getAttribute('role');

      const reduced=matchMedia('(prefers-reduced-motion: reduce)').matches;

      window.updateControls({status:'error',pending:false,message:'Seek failed. Try again.'});
      await until(()=>notice.textContent==='Seek failed. Try again.');

      const recoveryVisible=notice.textContent==='Seek failed. Try again.'&&!notice.querySelector('.spinner')&&getComputedStyle(notice).position!=='absolute';

      window.updateControls({status:'loading',pending:true,message:'Checking saved audio...'});
      await until(()=>notice.textContent==='Checking saved audio...');

      parent.postMessage({type:'capture-result',result:{labelHidden,name,tonal,sameSurface,spinner,status,recoveryVisible,reduced}},'*');
      })();
      </script>`)
    const result = await runBrowserFixture(directory, frame, width,
      resolve('src-tauri/target/play-anywhere-acceptance/ui-review', `playback-feedback-${width}-${locale}-${theme}.png`))
    expect(result).toMatchObject({
      labelHidden: width <= 480,
      tonal: true,
      sameSurface: true,
      spinner: true,
      status: 'status',
      recoveryVisible: true,
    })
    expect(result.name.length).toBeGreaterThan(0)
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
    expect(result.point).toEqual({
      format: 'html',
      segmentIndex: 1,
      offset: 9,
    })
  }, 30_000)

  it.each([{
    width:320,
    font:16,
    locale:'en',
  }, {
    width:390,
    font:32,
    locale:'ar',
  }])('keeps idle shortcut and bookmark feedback separate: %j', async ({width,font,locale}) => {
    const frame = join(directory, 'reader-feedback.html')
    writeFileSync(frame, `<!doctype html><html><meta charset="utf-8"><style>${css}html{font-size:${font}px}article{margin:80px 30px;font-size:1rem}</style>
      <div class="app-reader"><header class="doc-header"><div id="controls"></div></header><main class="document-view"><article>${'<p>A synthetic saved book passage for reading.</p>'.repeat(30)}</article></main><div class="reader-bookmark-notice">Bookmark saved</div></div>
      <script>${bundle.replace(/<\/script/gi, '<\\/script')}</script><script>
      (async()=>{await ReaderControls.mount(${JSON.stringify(locale)},true);while(!window.captureReady||!document.querySelector('.reader-passage-action'))await new Promise(requestAnimationFrame);
      scrollTo(0,600);const text=document.querySelectorAll('article p')[10].firstChild;document.getSelection().setBaseAndExtent(text,25,text,5);document.dispatchEvent(new Event('selectionchange'));
      await document.fonts.ready;await new Promise(requestAnimationFrame);await new Promise(requestAnimationFrame);
      const button=document.querySelector('.reader-passage-action'),r=button.getBoundingClientRect(),rail=document.querySelector('.reader-floating-actions').getBoundingClientRect(),notice=document.querySelector('.reader-bookmark-notice').getBoundingClientRect();
      parent.postMessage({type:'capture-result',result:{left:r.left,right:r.right,width:r.width,height:r.height,name:button.getAttribute('aria-label'),label:button.textContent,railTop:rail.top,noticeBottom:notice.bottom}},'*');})();</script>`)
    const result = await runBrowserFixture(directory, frame, width,
      resolve('src-tauri/target/play-anywhere-acceptance/ui-review', `reader-shortcut-${width}-${locale}.png`))
    expect(result.left).toBeGreaterThanOrEqual(0)
    expect(result.right).toBeLessThanOrEqual(width)
    expect(result.height).toBeGreaterThanOrEqual(44)
    expect(result.width).toBeCloseTo(result.height, 1)
    expect(result).toMatchObject({ label: '' })
    expect(result.name).toBeTruthy()
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
      if(action.getAttribute('aria-label')!=='Play from selection')throw new Error('Selection shortcut did not update');
      if(${JSON.stringify(method)}==='keyboard')document.body.dispatchEvent(new KeyboardEvent('keydown',{key:'Tab',bubbles:true}));
      else action.dispatchEvent(new PointerEvent('pointerdown',{bubbles:true,pointerType:'mouse'}));
      action.focus();document.getSelection().removeAllRanges();document.dispatchEvent(new Event('selectionchange'));await new Promise(requestAnimationFrame);
      if(action.getAttribute('aria-label')!=='Play from selection')throw new Error('Focus changed the advertised action');
      action.click();})();</script>`)
    const result = await runBrowserFixture(directory, frame, 390)
    expect(result.status, JSON.stringify(result)).toBe('captured')
    expect(result.current).toBe(true)
    expect(result.target.status, JSON.stringify(result)).toBe('resolved')
    expect(result.point).toEqual({
      format: 'html',
      segmentIndex: 12,
      offset: 12,
    })
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
    if (legacy) {
      expect(result.target.chunkIndex).toBeGreaterThan(0)
    }
  }, 30_000)

})
