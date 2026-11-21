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
  expect,
  it,
} from 'vitest'
import {
  browser,
  runBrowserFixture,
} from '../../tts/playAnywhere/browserCaptureFixture'

it.skipIf(!browser)('opens browse titles and View through one control while scope titles select', async () => {
  const directory = mkdtempSync(join(tmpdir(), 'papercut-document-list-'))
  try {
    const frame = join(directory, 'index.html')
    writeFileSync(frame, `<!doctype html><html dir="rtl"><meta charset="utf-8">
      <div id="root"></div><script type="module" src="./entry.tsx"></script></html>`)
    writeFileSync(join(directory, 'entry.tsx'), `
      import {createRoot} from ${JSON.stringify(resolve('node_modules/react-dom/client.js'))};
      import {DocumentList} from ${JSON.stringify(resolve('src/components/DocumentList/DocumentList.tsx'))};
      import {changeAppLocale} from ${JSON.stringify(resolve('src/i18n/index.ts'))};
      import ${JSON.stringify(resolve('src/components/DocumentBrowser/DocumentBrowser.css'))};
      const doc={title:'الفصل الأول — A long document title '.repeat(5),url:'book.html',source:'upload'};
      const props={groupedDocs:[{author:'Library',docs:[doc]}],collapsedAuthors:new Set(),docFilterLower:'',onToggleAuthor:()=>{}};
      const opened=[], removed=[], selected=[];
      changeAppLocale('ar');
      createRoot(document.querySelector('#root')).render(<>
        <DocumentList {...props} onViewDocument={url=>opened.push(url)} onDeleteDocument={d=>removed.push(d.url)} />
        <DocumentList {...props} openingDocumentUrl={doc.url} onViewDocument={url=>opened.push(url)} />
        <DocumentList {...props} selectable onToggleFilter={url=>selected.push(url)} />
      </>);
      window.keyboardReady=(async()=>{
        while(!document.querySelector('.document-item')) await new Promise(requestAnimationFrame);
        const [browse,busy,scope]=document.querySelectorAll('.document-item');
        const title=browse.querySelector('.document-item-title');
        const view=browse.querySelector('.document-row-action-view');
        title.click();
        const afterTitle=opened.length;
        view.click();
        const afterView=opened.length;
        busy.querySelector('.document-item-title').click();
        scope.querySelector('.document-item-title').click();
        document.querySelector('.author-group-toggle').focus();
        const remove=browse.querySelector('.document-row-action-danger');
        remove.addEventListener('focus',()=>{
          remove.click();
          parent.postMessage({type:'capture-result',result:{opened,removed,selected,afterTitle,afterView,
            rtl:document.documentElement.dir==='rtl',
            sameButton:title.closest('button')===view.closest('button'),
            fullTitle:title.textContent===doc.title && title.scrollWidth>title.clientWidth,
            busy:busy.querySelector('button').disabled}},'*');
        });
      })();
    `)
    await build({
      root: directory,
      configFile: false,
      logLevel: 'silent',
      base: './',
      resolve: { alias: { 'react/jsx-runtime': resolve('node_modules/react/jsx-runtime.js') } },
      esbuild: {
        jsx: 'automatic',
        jsxDev: false,
      },
    })
    const result = await runBrowserFixture(directory, join(directory, 'dist/index.html'), 320,
      undefined, undefined, ['Tab', 'Enter', 'Space', 'Tab'])
    expect(result).toEqual({
      afterTitle: 1,
      afterView: 2,
      opened: Array(4).fill('book.html'),
      removed: ['book.html'],
      selected: ['book.html'],
      rtl: true,
      sameButton: true,
      fullTitle: true,
      busy: true,
    })
  } finally {
    rmSync(directory, {
      recursive: true,
      force: true,
    })
  }
}, 30_000)
