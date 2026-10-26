// @vitest-environment node
import {
  copyFileSync,
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
import { pathToFileURL } from 'node:url'
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
} from './browserCaptureFixture'

let directory: string
beforeAll(async () => {
  directory = mkdtempSync(join(tmpdir(), 'papercut-pdf-target-'))
  copyFileSync('node_modules/pdfjs-dist/legacy/build/pdf.worker.mjs', join(directory, 'pdf.worker.mjs'))
  const entry = join(directory, 'entry.ts')
  writeFileSync(entry, [
    ['src/tts/playAnywhere/pdfTextTarget.ts', 'createPdfTextTargetApi'],
    ['src/tts/playAnywhere/htmlTextTarget.ts', 'readerTextViewport'],
    ['src/tts/playAnywhere/prepareTextTarget.ts', 'createTextTargetPreparation'],
    ['src/pdf/ocr/pdfOcrTextLayer.ts', 'renderPdfOcrTextLayer, pdfOcrSourcePoint'],
    ['src/pdf/pdfTts.ts', 'pdfNarrationToReadableSegments, pdfSourceSpansForChunk'],
    ['src/tts/utils/text.ts', 'chunkAudiobookSaveSegmentsWithSpans'],
    ['node_modules/pdfjs-dist/legacy/build/pdf.mjs', 'getDocument, GlobalWorkerOptions'],
    ['node_modules/pdfjs-dist/legacy/web/pdf_viewer.mjs', 'PDFViewer, EventBus, PDFLinkService, PDFFindController'],
  ].map(([path, exports]) => `export { ${exports} } from ${JSON.stringify(resolve(path))}`).join('\n'))
  const output = await build({
    configFile: false,
    logLevel: 'silent',
    build: {
      write: false,
      minify: false,
      lib: {
        entry,
        name: 'PdfTargets',
        formats: ['iife'],
      },
    },
  })
  const result = Array.isArray(output) ? output[0] : output
  if (!('output' in result)) {
    throw new Error('Expected browser bundle')
  }
  writeFileSync(join(directory, 'bundle.js'), result.output.filter(item => item.type === 'chunk').map(item => item.code).join('\n'))
}, 30_000)
afterAll(() => {
  if (directory) {
    rmSync(directory, {
      recursive: true,
      force: true,
    })
  }
})

const setup = `
const container=document.querySelector('.pdf-viewer-container'), root=container.firstElementChild;
const bus=new PdfTargets.EventBus();
const bounds=()=>PdfTargets.readerTextViewport(document.querySelector('.app-reader'));
function plain(r){return r.status==='captured'?{status:r.status,point:r.point,sourceText:r.sourceText,current:r.isCurrent()}:r}
const tick=()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));
function oracle(page, mapping) {
  const b=bounds(), c=container.getBoundingClientRect();
  const box={top:Math.max(b.top,c.top),bottom:Math.min(b.bottom,c.bottom),left:Math.max(b.left,c.left),right:Math.min(b.right,c.right)};
  for(let blockOrder=0;blockOrder<mapping.textDivs.length;blockOrder++){
    const element=mapping.textDivs[blockOrder], walker=document.createTreeWalker(element,NodeFilter.SHOW_TEXT);let prior=0;
    while(walker.nextNode()){
      const n=walker.currentNode;
      for(let offset=0;offset<n.length;offset++){
        if(/\\s/u.test(n.data[offset]))continue;
        const range=document.createRange();range.setStart(n,offset);range.setEnd(n,offset+1);
        if([...range.getClientRects()].some(r=>r.bottom>box.top&&r.top<box.bottom&&r.right>box.left&&r.left<box.right))
          return {format:'pdf',pageIndex:page,blockOrder,offset:prior+offset};
      }
      prior+=n.length;
    }
  }
  return null;
}
`
async function run(action: string, native = false) {
  const frame = join(directory, 'frame.html')
  const css = readFileSync('node_modules/pdfjs-dist/web/pdf_viewer.css', 'utf8') + readFileSync('src/viewers/PdfViewer.css', 'utf8')
  const pdf = readFileSync('scripts/fixtures/pdf/find-unicode-wraps.pdf').toString('base64')
  const nativeSetup = `
  PdfTargets.GlobalWorkerOptions.workerSrc=${JSON.stringify(pathToFileURL(join(directory, 'pdf.worker.mjs')).href)};
  const links=new PdfTargets.PDFLinkService({eventBus:bus});
  const find=new PdfTargets.PDFFindController({eventBus:bus,linkService:links});
  const viewer=new PdfTargets.PDFViewer({container,viewer:root,eventBus:bus,linkService:links,findController:find});links.setViewer(viewer);
  const task=PdfTargets.getDocument({data:Uint8Array.from(atob(${JSON.stringify(pdf)}),c=>c.charCodeAt(0)),useSystemFonts:true});
  const pdf=await task.promise;
  const rendered=new Promise(resolve=>bus.on('textlayerrendered',resolve));
  bus.on('pagesinit',()=>{viewer.currentScaleValue='1';});
  viewer.setDocument(pdf);links.setDocument(pdf);await rendered;await tick();
  const sourcePages=await Promise.all(Array.from({length:pdf.numPages},async(_,pageIndex)=>{
    const content=await(await pdf.getPage(pageIndex+1)).getTextContent({disableNormalization:true});
    return {schemaVersion:1,pageIndex,width:612,height:792,blocks:content.items.filter(item=>'str'in item).map((item,order)=>({order,text:item.str+(item.hasEOL?'\\n':''),bounds:[0,0,1,1],confidence:null}))};
  }));
  const sourceSegments=PdfTargets.pdfNarrationToReadableSegments(sourcePages.flatMap(page=>page.blocks.filter(block=>block.text.trim()).map(block=>({text:block.text.trim(),sourceRuns:[{pageIndex:page.pageIndex,blockOrder:block.order,startOffset:0,endOffset:block.text.trim().length,sourceStartOffset:0,sourceEndOffset:block.text.trim().length}]}))));
  const sourceChunks=PdfTargets.chunkAudiobookSaveSegmentsWithSpans(sourceSegments).map((chunk,i)=>({...chunk,id:'saved-'+i,pdfSourceSpans:PdfTargets.pdfSourceSpansForChunk(sourceSegments,chunk.sourceSpan)}));
  const sourcePreparation=PdfTargets.createTextTargetPreparation({documentUrl:'/uploads/ab.pdf',docContent:'',chunks:sourceChunks,getChunks:async()=>sourceChunks,getPdfSegments:async()=>sourceSegments,getPdfPageText:async(_,index)=>sourcePages[index]});
  `
  // Keep generated JavaScript out of HTML parsing; fixture strings need no script-tag sanitizer.
  writeFileSync(join(directory, 'case.js'), `(async()=>{try{${setup}${native ? nativeSetup : ''}${action}}
  catch(error){parent.postMessage({type:'capture-result',result:{error:String(error.stack||error)}},'*')}})()`)
  writeFileSync(frame, `<!doctype html><meta charset="utf-8"><style>${css}
  body{margin:0}.doc-header{position:fixed;top:0;height:90px;width:100%;background:white;z-index:10}
  .document-view{height:600px}.pdf-viewer-container{top:40px;bottom:60px;left:0;right:0}
  .audio-floating-playback{position:fixed;bottom:0;height:60px;width:100%;background:white;z-index:10}
  </style><div class="app-reader"><header class="doc-header">Toolbar</header><main class="document-view"><div class="pdf-viewer-container"><div class="pdfViewer"></div></div></main><div class="audio-floating-playback">Audio</div></div>
  <script src="bundle.js"></script><script src="case.js"></script>`)
  return runBrowserFixture(directory, frame)
}

describe.skipIf(!browser)('PDF/OCR viewport capture in Chromium', () => {
  it('preserves script-like test data without turning it into HTML', async () => {
    const value = '</script><script>throw new Error("injected")</script><!--'
    const result = await run(`parent.postMessage({type:'capture-result',result:{value:${JSON.stringify(value)}}},'*')`)
    expect(result.value).toBe(value)
  }, 30_000)
  it('maps a backward native PDF selection through real Find nodes and the verified resolver', async () => {
    const result = await run(`
      const found=new Promise(resolve=>bus.on('updatetextlayermatches',resolve));
      bus.dispatch('find',{query:'coastal ferries',type:'',caseSensitive:false,entireWord:false,highlightAll:true,findPrevious:false});await found;await tick();
      const page=viewer.getPageView(0),mapping=page._textHighlighter;
      const blockOrder=mapping.textContentItemsStr.findIndex(text=>text.length>15),item=mapping.textDivs[blockOrder];
      function point(offset){const walker=document.createTreeWalker(item,NodeFilter.SHOW_TEXT);let node=item.nodeType===3?item:walker.nextNode();while(node){if(offset<node.length)return {node,offset};offset-=node.length;node=walker.nextNode()}throw new Error('Missing selected point')}
      let offset=3;while(/\\s/u.test(mapping.textContentItemsStr[blockOrder][offset]))offset++;
      const start=point(offset),end=point(offset+3);
      document.getSelection().setBaseAndExtent(end.node,end.offset,start.node,start.offset);
      const session=PdfTargets.createPdfTextTargetApi(viewer,container,bus,bounds);
      const capture=session.api.captureSelection(),before=plain(capture),resolved=await sourcePreparation.resolve('/uploads/ab.pdf',capture);
      const button=document.createElement('button');button.className='audio-controls';document.body.append(button);button.focus();document.getSelection().removeAllRanges();
      const focusSafe=capture.status==='captured'&&capture.isCurrent();bus.dispatch('rotationchanging',{});
      const stale=capture.status==='captured'&&!capture.isCurrent();session.dispose();
      parent.postMessage({type:'capture-result',result:{before,expected:{format:'pdf',pageIndex:0,blockOrder,offset},resolved:resolved.status,focusSafe,stale}},'*');await task.destroy();
    `, true)
    expect(result.before.status).toBe('captured')
    expect(result.before.point).toEqual(result.expected)
    expect(result.resolved).toBe('resolved')
    expect(result.focusSafe).toBe(true)
    expect(result.stale).toBe(true)
  }, 30_000)

  it.each([{
    mode: 'standard',
    rotation: 90,
  }, {
    mode: 'webkit-fallback',
    rotation: 180,
  }, {
    mode: 'ranges-only',
    rotation: 270,
  }])('uses real PDF.js offsets through Find, zoom, rotation, page navigation and fullscreen (%j)', async ({ mode, rotation }) => {
    const result = await run(`
      if(${JSON.stringify(mode)}!=='standard')Object.defineProperty(document,'caretPositionFromPoint',{value:undefined});
      if(${JSON.stringify(mode)}==='ranges-only')Object.defineProperty(document,'caretRangeFromPoint',{value:undefined});
      const session=PdfTargets.createPdfTextTargetApi(viewer,container,bus,bounds);
      async function capture(){
        await tick();const r=await session.api.captureViewport(), resolved=await sourcePreparation.resolve('/uploads/ab.pdf',r);
        return {capture:plain(r),oracle:oracle(viewer.currentPageNumber-1,viewer.getPageView(viewer.currentPageNumber-1)._textHighlighter),
          target:resolved.status==='resolved'?{status:resolved.status,chunkIndex:resolved.chunkIndex,chunkId:resolved.chunkId}:resolved}
      }
      const first=await capture();
      // The actual Find controller rewrites item children; source offsets must remain unchanged.
      const found=new Promise(resolve=>bus.on('updatetextlayermatches',resolve));
      bus.dispatch('find',{query:'coastal ferries',type:'',caseSensitive:false,entireWord:false,highlightAll:true,findPrevious:false});await found;
      const afterFind=await capture();
      async function layout(change){const ready=new Promise(resolve=>bus.on('textlayerrendered',resolve));change();await ready;return capture()}
      const zoom=await layout(()=>viewer.currentScaleValue='1.35');
      await layout(()=>viewer.pagesRotation=${rotation});
      viewer.getPageView(0)._textHighlighter.textDivs[0].scrollIntoView({block:'center',inline:'center'});
      const rotated=await capture();
      const pageTwo=await layout(()=>{viewer.pagesRotation=0;viewer.currentPageNumber=2});
      document.querySelector('.doc-header').style.display='none';container.style.top='0px';
      const fullscreen=await capture();
      const snapshot=await session.api.captureViewport();bus.dispatch('scalechanging',{});
      const stale=snapshot.status==='captured'&&!snapshot.isCurrent();session.dispose();
      parent.postMessage({type:'capture-result',result:{first,afterFind,zoom,rotated,pageTwo,fullscreen,stale,disposed:await session.api.captureViewport()}},'*');await task.destroy();
    `, true)
    for (const key of ['first', 'afterFind', 'zoom', 'rotated', 'pageTwo', 'fullscreen']) {
      expect(result[key].oracle, key).not.toBeNull()
      expect(result[key].capture.status, key).toBe('captured')
      expect(result[key].capture.point, key).toEqual(result[key].oracle)
      expect(result[key].capture.current, key).toBe(true)
      expect(result[key].target.status, key).toBe('resolved')
    }
    expect(result.pageTwo.capture.point.pageIndex).toBe(1)
    expect(result.pageTwo.target.chunkIndex).toBeGreaterThan(0)
    expect(result.stale).toBe(true)
    expect(result.disposed).toEqual({
      status: 'unavailable',
      reason: 'stale',
    })
  }, 30_000)

  it('inverts merged OCR lines to original word offsets and resolves a production chunk at its beginning', async () => {
    const result = await run(`
      const page=document.createElement('div');page.className='page';page.dataset.pageNumber='4';
      page.style.cssText='position:relative;width:900px;height:900px;margin:0';root.append(page);
      const textLayer=document.createElement('div');textLayer.className='textLayer';textLayer.style.cssText='width:600px;height:900px';
      textLayer.innerHTML='<div class="endOfContent"></div>';page.append(textLayer);
      const layer={schemaVersion:1,pageIndex:3,width:600,height:900,blocks:[
        {text:'Earlier sentence. '.repeat(50)+'\\n',bounds:[30,10,500,20],order:2,confidence:1},
        {text:'😀A',bounds:[30,150,50,20],order:7,confidence:1},
        {text:'recognized\\r\\npassage.  ',bounds:[90,150,350,20],order:11,confidence:1}
      ]};
      const overlay=PdfTargets.renderPdfOcrTextLayer(page,textLayer,layer);
      const viewer={pagesCount:4,_getVisiblePages:()=>({views:[{id:4}]}),getPageView:()=>({div:page})};
      const session=PdfTargets.createPdfTextTargetApi(viewer,container,bus,bounds);
      await tick();container.scrollTop=100;container.scrollLeft=210;
      const r=await session.api.captureViewport();
      const segments=PdfTargets.pdfNarrationToReadableSegments(layer.blocks.map(b=>({text:b.text.replace(/[\\r\\n]+/gu,'').trim(),sourceRuns:[{pageIndex:3,blockOrder:b.order,startOffset:0,endOffset:b.text.replace(/[\\r\\n]+/gu,'').trim().length,sourceStartOffset:0,sourceEndOffset:b.text.trim().length}]})));
      const chunks=PdfTargets.chunkAudiobookSaveSegmentsWithSpans(segments).map((c,i)=>({...c,id:'saved-'+i,pdfSourceSpans:PdfTargets.pdfSourceSpansForChunk(segments,c.sourceSpan)}));
      const preparation=PdfTargets.createTextTargetPreparation({documentUrl:'/uploads/ab.pdf',docContent:'',chunks,getChunks:async()=>chunks,getPdfSegments:async()=>segments,getPdfPageText:async()=>layer});
      const resolved=await preparation.resolve('/uploads/ab.pdf',r);
      const target=resolved.status==='resolved'?{status:resolved.status,chunkIndex:resolved.chunkIndex,chunkId:resolved.chunkId}:resolved;
      const line=overlay.querySelectorAll('span')[1];
      const before=plain(r);line.innerHTML='<mark>'+line.textContent+'</mark>';
      const afterFind=plain(await session.api.captureViewport());
      const selectedNode=line.querySelector('mark').firstChild;
      document.getSelection().setBaseAndExtent(selectedNode,8,selectedNode,6);
      const selection=session.api.captureSelection(),selected=plain(selection),selectionTarget=await preparation.resolve('/uploads/ab.pdf',selection);
      const space=PdfTargets.pdfOcrSourcePoint(line,3),newline=PdfTargets.pdfOcrSourcePoint(line,14),trim=PdfTargets.pdfOcrSourcePoint(line,24);
      line.firstChild.firstChild.data=line.textContent.replace('recognized','misaligned');const stale=r.status==='captured'&&!r.isCurrent();
      parent.postMessage({type:'capture-result',result:{before,afterFind,target,selected,selectionTarget:selectionTarget.status,stale,changed:plain(await session.api.captureViewport()),space,newline,trim}},'*');session.dispose();
    `)
    expect(result.before.status).toBe('captured')
    expect(result.before.point).toMatchObject({
      format: 'pdf',
      pageIndex: 3,
      blockOrder: 11,
    })
    expect(result.before.point.offset).toBeGreaterThan(0)
    expect(result.before.sourceText).toBe('recognized\r\npassage.  ')
    expect(result.afterFind.point).toEqual(result.before.point)
    expect(result.selected.point).toEqual({
      format: 'pdf',
      pageIndex: 3,
      blockOrder: 11,
      offset: 2,
    })
    expect(result.selectionTarget).toBe('resolved')
    expect(result.target.status).toBe('resolved')
    expect(result.target.chunkIndex).toBeGreaterThan(0)
    expect(result.stale).toBe(true)
    expect(result.changed).toEqual({
      status: 'unavailable',
      reason: 'unmapped',
    })
    expect(result.space).toBeNull()
    expect(result.newline).toMatchObject({
      blockOrder: 11,
      offset: 12,
    })
    expect(result.trim).toBeNull()
  }, 30_000)

  it('keeps Arabic OCR in source order when columns run in the opposite screen order', async () => {
    const result = await run(`
      document.documentElement.dir='rtl';
      const page=document.createElement('div');page.className='page';page.dataset.pageNumber='1';page.style.cssText='position:relative;width:600px;height:800px;margin:0';
      page.innerHTML='<div class="textLayer" style="width:600px;height:800px"><div class="endOfContent"></div></div>';root.append(page);
      const layer={schemaVersion:1,pageIndex:0,width:600,height:800,blocks:[
        {text:'بِدايَةٌ واضحةٌ.\\n',bounds:[350,150,200,24],order:9,confidence:1},
        {text:'Later source.\\n',bounds:[10,150,180,24],order:20,confidence:1}
      ]};
      PdfTargets.renderPdfOcrTextLayer(page,page.querySelector('.textLayer'),layer);
      const viewer={pagesCount:1,_getVisiblePages:()=>({views:[{id:1}]}),getPageView:()=>({div:page})};
      const session=PdfTargets.createPdfTextTargetApi(viewer,container,bus,bounds);await tick();
      parent.postMessage({type:'capture-result',result:plain(await session.api.captureViewport())},'*');session.dispose();
    `)
    expect(result.point).toEqual({
      format: 'pdf',
      pageIndex: 0,
      blockOrder: 9,
      offset: 0,
    })
  }, 30_000)

  it('reports unavailable for blank pages and unmapped OCR, and invalidates detached snapshots', async () => {
    const result = await run(`
      const page=document.createElement('div');page.className='page';page.dataset.pageNumber='1';
      page.style.cssText='position:relative;width:600px;height:800px;margin:0';
      page.innerHTML='<div class="textLayer"><div class="endOfContent"></div></div>';root.append(page);
      const view={div:page,_textHighlighter:{textDivs:[],textContentItemsStr:[]}};
      const viewer={pagesCount:1,_getVisiblePages:()=>({views:[{id:1}]}),getPageView:()=>view};
      const session=PdfTargets.createPdfTextTargetApi(viewer,container,bus,bounds);await tick();
      const blank=await session.api.captureViewport();
      const layer={schemaVersion:1,pageIndex:0,width:600,height:800,blocks:[{text:'Visible recognized text.',bounds:[20,150,300,20],order:0,confidence:1}]};
      const overlay=PdfTargets.renderPdfOcrTextLayer(page,page.querySelector('.textLayer'),layer);
      const captured=await session.api.captureViewport();
      overlay.innerHTML=overlay.innerHTML;const unmapped=await session.api.captureViewport();
      const stale=captured.status==='captured'&&!captured.isCurrent();
      container.remove();const detached=await session.api.captureViewport();
      parent.postMessage({type:'capture-result',result:{blank,unmapped:plain(unmapped),captured:plain(captured),stale,detached}},'*');session.dispose();
    `)
    expect(result.blank).toEqual({
      status: 'unavailable',
      reason: 'no-visible-text',
    })
    expect(result.captured.status).toBe('captured')
    expect(result.unmapped).toEqual({
      status: 'unavailable',
      reason: 'unmapped',
    })
    expect(result.stale).toBe(true)
    expect(result.detached).toEqual({
      status: 'unavailable',
      reason: 'stale',
    })
  }, 30_000)

  it('restores source order for spreads and rejects absent/stale/hidden layers without rendering them', async () => {
    const result = await run(`
      const pages=[1,2].map(id=>{const p=document.createElement('div');p.className='page';p.dataset.pageNumber=id;p.style.cssText='position:absolute;top:120px;left:'+(id===1?300:0)+'px;width:300px;height:300px';p.innerHTML='<div class="textLayer"><span style="position:absolute;left:0;top:0;font-size:20px;color:transparent">Visible text '+id+'</span><div class="endOfContent"></div></div>';root.append(p);return p});
      const mappings=pages.map(p=>({textDivs:[p.querySelector('span')],textContentItemsStr:[p.querySelector('span').textContent]}));
      const pageViews=pages.map((p,i)=>({div:p,_textHighlighter:mappings[i]}));
      let requested=[];const viewer={pagesCount:10000,_getVisiblePages:()=>({views:[{id:2},{id:1}]}),getPageView:i=>{requested.push(i);return pageViews[i]}};
      const session=PdfTargets.createPdfTextTargetApi(viewer,container,bus,bounds);
      await tick();const first=await session.api.captureViewport();
      pages[0].querySelector('.endOfContent').remove();const missing=await session.api.captureViewport();
      pages[0].querySelector('.textLayer').innerHTML+='<div class="endOfContent"></div>';
      const unmapped=await session.api.captureViewport();
      const stale=first.status==='captured'&&!first.isCurrent();
      pageViews[0]=undefined;const absent=await session.api.captureViewport();
      Object.defineProperty(document,'visibilityState',{value:'hidden',configurable:true});document.dispatchEvent(new Event('visibilitychange'));
      const hidden=await session.api.captureViewport();session.dispose();
      parent.postMessage({type:'capture-result',result:{first:plain(first),missing:plain(missing),unmapped:plain(unmapped),stale,absent,hidden,requested}},'*');
    `)
    expect(result.first.point.pageIndex).toBe(0)
    expect(result.missing).toEqual({
      status: 'unavailable',
      reason: 'not-ready',
    })
    expect(result.unmapped.status).toBe('unavailable')
    expect(result.stale).toBe(true)
    expect(result.absent).toEqual({
      status: 'unavailable',
      reason: 'not-ready',
    })
    expect(result.hidden).toEqual({
      status: 'unavailable',
      reason: 'not-ready',
    })
    expect(result.requested.every((i: number) => i < 2)).toBe(true)
  }, 30_000)
})
