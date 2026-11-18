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
import {
  afterAll,
  describe,
  expect,
  it,
} from 'vitest'
import {
  browser,
  runBrowserFixture,
} from '../../tts/playAnywhere/browserCaptureFixture'

const directory = mkdtempSync(join(tmpdir(), 'papercut-search-layout-'))
const css = readFileSync(resolve('src/components/SearchTab/SearchTab.css'), 'utf8')
afterAll(() => rmSync(directory, {
  recursive: true,
  force: true,
}))

describe.skipIf(!browser)('Search selector layout in Chromium', () => {
  it.each([
    [280, 16, 'ltr'], [352, 16, 'ltr'], [280, 24, 'rtl'], [352, 24, 'rtl'],
  ])('keeps tabs clear of the trigger with and without subgrid at %ipx/%ipx/%s', async (width, font, direction) => {
    // Disable the subgrid declaration and activate the real fallback to model unsupported engines.
    const fallback = css.replace('grid-template-columns: subgrid;', 'grid-template-columns: none;')
      .replace('@supports not (grid-template-columns: subgrid)', '@supports (display: grid)')
    const frame = join(directory, 'selector.html')
    writeFileSync(frame, `<!doctype html><html dir="${direction}"><meta charset="utf-8">
      <style>*{box-sizing:border-box}html{font-size:${font}px}body{margin:0}</style>
      <style id="selector-css">${css}</style>
      <div class="search-result-selector" data-keyword-active="true">
        <div class="search-result-tabs" role="tablist">
          <div class="search-result-tab search-keyword-tab" role="tab"><span class="search-keyword-label"><span>Keyword</span><span class="search-keyword-mode">All Words</span></span></div>
          <div class="search-result-tab" role="tab">Conceptual</div>
        </div>
        <div class="search-keyword-matching"><button class="search-matching-trigger" aria-label="Matching">⌄</button></div>
      </div>
      <script>
        const selector=document.querySelector('.search-result-selector');
        const tabs=[...document.querySelectorAll('[role="tab"]')];
        const matching=document.querySelector('.search-keyword-matching');
        const trigger=document.querySelector('button');
        const box=e=>e.getBoundingClientRect();
        const results=[];
        for(const sheet of [${JSON.stringify(css)},${JSON.stringify(fallback)}]){
          document.querySelector('#selector-css').textContent=sheet;
          for(const keyword of [true,false]){
            selector.dataset.keywordActive=String(keyword);
            matching.hidden=!keyword;
            const t=box(trigger),s=box(selector),rects=tabs.map(box);
            results.push({keyword,overlap:keyword && rects.some(b=>b.left<t.right-.5 && b.right>t.left+.5),
              within:rects.every(b=>b.left>=s.left-.5 && b.right<=s.right+.5),
              triggerWidth:t.width,triggerHeight:t.height,overflow:document.documentElement.scrollWidth>innerWidth});
          }
        }
        parent.postMessage({type:'capture-result',result:{cases:results}},'*');
      </script></html>`)
    const result = await runBrowserFixture(directory, frame, Number(width))
    const cases = result.cases as Array<{ keyword: boolean;
      overlap: boolean;
      within: boolean;
      triggerWidth: number;
      triggerHeight: number;
      overflow: boolean }>
    expect(cases).toHaveLength(4)
    for (const measurement of cases) {
      expect(measurement).toMatchObject({
        overlap: false,
        within: true,
        overflow: false,
      })
      if (measurement.keyword) {
        expect(measurement.triggerWidth).toBeGreaterThanOrEqual(44)
        expect(measurement.triggerHeight).toBeGreaterThanOrEqual(44)
      }
    }
  }, 30_000)
})
