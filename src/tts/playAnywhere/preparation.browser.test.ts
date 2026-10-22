// @vitest-environment node
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { build } from 'vite'
import { afterAll, beforeAll, describe, expect, it } from 'vitest'
import { browser, runBrowserFixture } from './browserCaptureFixture'

let directory: string, bundle: string
beforeAll(async () => {
  directory = mkdtempSync(join(tmpdir(), 'papercut-preparation-'))
  const entry = join(directory, 'entry.ts')
  writeFileSync(entry, `
    import {createTextTargetPreparation} from ${JSON.stringify(resolve('src/tts/playAnywhere/prepareTextTarget.ts'))};
    import {chunkAudiobookSaveHtmlWithSpans} from ${JSON.stringify(resolve('src/tts/utils/text.ts'))};
    import {getTtsDiagnostics} from ${JSON.stringify(resolve('src/tts/diagnostics/TtsDiagnostics.ts'))};
    export async function exercise(cancel) {
      const texts=Array.from({length:1200},(_,i)=>'Paragraph '+i+' '+('Narrated clause without a space,'+'another clause (etc.). Next sentence. ').repeat(4));
      const docContent=texts.map(text=>'<p>'+text.trim()+'</p>').join('');
      const canonical=chunkAudiobookSaveHtmlWithSpans(docContent);
      const saved=canonical.map(({text},i)=>({id:'saved-'+i,text}));
      const before=JSON.stringify(saved),controller=new AbortController();
      const preparation=createTextTargetPreparation({documentUrl:'/user-uploads/ab.html',docContent,chunks:saved,
        getChunks:async()=>saved,getPdfSegments:async()=>[],getPdfPageText:async()=>{throw Error('HTML only')}});
      const capture={status:'captured',point:{format:'html',segmentIndex:1199,offset:2},
        segmentTexts:Object.freeze(texts.map(text=>text.trim())),segmentLengths:texts.map(text=>text.trim().length),isCurrent:()=>true};
      let ticks=0,finished=false,paintBeforeCompletion=false,eventBeforeCompletion=false;
      const heartbeat=setInterval(()=>ticks++,0);
      document.getElementById('status').textContent='Preparing';
      requestAnimationFrame(()=>{paintBeforeCompletion=!finished});
      const started=performance.now(),pending=preparation.resolve('/user-uploads/ab.html',capture,controller.signal);
      setTimeout(()=>{eventBeforeCompletion=!finished;if(cancel)controller.abort()},0);
      const result=await pending;finished=true;
      const coldMs=performance.now()-started;
      clearInterval(heartbeat);
      const warmStart=performance.now(),warm=cancel?null:await preparation.resolve('/user-uploads/ab.html',capture);
      const warmMs=performance.now()-warmStart;
      const metrics=getTtsDiagnostics().find(event=>event.label==='[play-anywhere] preparation completed')?.data;
      preparation.dispose();
      return {status:result.status,reason:result.reason,chunkIndex:result.chunkIndex,expectedIndex:saved.length-1,
        identityUnchanged:JSON.stringify(saved)===before,ticks,eventBeforeCompletion,paintBeforeCompletion,
        warmStatus:warm?.status,coldMs,warmMs,metrics,chunks:saved.length,sourceBytes:docContent.length};
    }
  `)
  const output = await build({ configFile: false, logLevel: 'silent', build: { write: false, minify: false, lib: { entry, name: 'Preparation', formats: ['iife'] } } })
  const result = Array.isArray(output) ? output[0] : output
  if (!('output' in result)) throw new Error('Expected fixture bundle')
  bundle = result.output.filter(item => item.type === 'chunk').map(item => item.code).join('\n')
}, 30_000)
afterAll(() => { if (directory) rmSync(directory, { recursive: true, force: true }) })

describe.skipIf(!browser)('Cold preparation in Chromium', () => {
  it.each([false, true])('handles another event before completion; cancel=%s', async cancel => {
    const frame = join(directory, 'frame.html')
    writeFileSync(frame, `<!doctype html><meta charset="utf-8"><body><div id="status"></div><script>${bundle.replace(/<\/script/gi, '<\\/script')}</script><script>
      Preparation.exercise(${cancel}).then(result=>parent.postMessage({type:'capture-result',result},'*'));
    </script>`)
    const result = await runBrowserFixture(directory, frame)
    expect(result.eventBeforeCompletion).toBe(true)
    expect(result.ticks).toBeGreaterThan(0)
    expect(result.identityUnchanged).toBe(true)
    if (cancel) expect(result).toMatchObject({ status: 'unavailable', reason: 'stale' })
    else {
      expect(result).toMatchObject({ status: 'resolved', chunkIndex: result.expectedIndex, warmStatus: 'resolved', paintBeforeCompletion: true })
      expect(result.metrics.batches).toBeGreaterThan(1)
      // Numeric evidence only: browser/host load makes wall-clock thresholds unsuitable here.
      console.info('Synthetic Chromium preparation (not packaged acceptance)', result)
    }
  })
})
