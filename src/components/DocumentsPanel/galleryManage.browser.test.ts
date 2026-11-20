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

it.skipIf(!browser)('keeps both gallery categories and Manage usable on desktop and narrow RTL screens', async () => {
  const directory = mkdtempSync(join(tmpdir(), 'papercut-gallery-manage-'))
  try {
    writeFileSync(join(directory, 'index.html'), `<!doctype html><meta charset="utf-8">
      <div id="root"></div><script type="module" src="./entry.tsx"></script>`)
    writeFileSync(join(directory, 'entry.tsx'), `
      import {createRoot} from ${JSON.stringify(resolve('node_modules/react-dom/client.js'))};
      import {DocumentsPanel} from ${JSON.stringify(resolve('src/components/DocumentsPanel/DocumentsPanel.tsx'))};
      import {changeAppLocale} from ${JSON.stringify(resolve('src/i18n/index.ts'))};
      import ${JSON.stringify(resolve('src/index.css'))};
      const q = selector => document.querySelector(selector);
      const frame = () => new Promise(requestAnimationFrame);
      async function check() {
        const narrow = innerWidth === 320;
        await changeAppLocale(narrow ? 'ar' : 'en');
        document.documentElement.style.fontSize = narrow ? '24px' : '16px';
        const docs = [{source:'upload', format:'epub', url:'/uploads/aa.html', title:'Book'},
                      {source:'upload', format:'html', url:'/uploads/bb.html', title:'Document'}];
        const noop = () => {};
        createRoot(q('#root')).render(<DocumentsPanel allDocuments={docs} groupedDocs={[{author:'Uploads',docs}]}
          libraryOrganization={{folders:[], documentLocations:[]}} collapsedAuthors={new Set()} showDocuments
          documentFilter="" docFilterLower="" documentsLoading={false} onFilterChange={noop} onToggleShow={noop}
          onToggleAuthor={noop} onViewDocument={noop} onCreateLibraryFolder={noop} onDeleteDocuments={noop}
          onDeleteLibraryFolder={noop} onMoveLibraryDocuments={noop} onRenameLibraryFolder={noop}/>);
        const manage = '.library-gallery-manage';
        while (!q(manage)) await frame();
        const results = [];
        for (const index of [0,1]) {
          const category = () => document.querySelectorAll('.library-gallery-category')[index];
          category().click();
          await frame();
          const group = q('.library-gallery-categories').getBoundingClientRect();
          const action = q(manage).getBoundingClientRect();
          const bar = q('.library-gallery-toolbar').getBoundingClientRect();
          const fits = el => {
            const r = el.getBoundingClientRect();
            return r.width > 0 && r.left >= 0 && r.right <= innerWidth && el.scrollWidth <= el.clientWidth + 1;
          };
          const aligned = narrow ? action.top >= group.bottom && Math.abs(action.left-bar.left)<2
            : action.left > group.right && Math.abs(action.right-bar.right)<2;
          const layout = aligned && Math.abs(group.left+group.right-bar.left-bar.right)<2
            && Array.from(q('.library-gallery-toolbar').querySelectorAll('button')).every(fits);
          q(manage).click();
          await frame();
          const done = q('.uploaded-library-edit-btn[aria-pressed=true]');
          const entered = fits(done) && document.activeElement === done && !q('.library-view-toggle');
          done.click();
          await frame();
          results.push(layout && entered && document.activeElement === q(manage)
            && category().getAttribute('aria-pressed') === 'true');
        }
        return {direction:document.documentElement.dir, results};
      }
      check().then(result => parent.postMessage({type:'capture-result',result}, '*'));
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
    for (const [width, direction] of [[900, 'ltr'], [320, 'rtl']] as const) {
      const result = await runBrowserFixture(directory, join(directory, 'dist/index.html'), width)
      expect(result).toEqual({
        direction,
        results: [true, true],
      })
    }
  } finally {
    rmSync(directory, {
      recursive: true,
      force: true,
    })
  }
}, 30_000)
