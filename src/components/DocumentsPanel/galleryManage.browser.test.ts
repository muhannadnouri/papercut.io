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

it.skipIf(!browser)('groups Import and Manage in both views and restores focus after Done', async () => {
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
        if ([460,644].includes(innerWidth)) q('#root').style.marginInline = innerWidth === 644 ? '18px' : '15px';
        if (innerWidth === 591) q('#root').style.marginInline = '13px';
        const docs = [{source:'upload', format:'epub', url:'/uploads/aa.html', title:'Book'},
                      {source:'upload', format:'html', url:'/uploads/bb.html', title:'Document'}];
        const folders = [{id:'shelf',name:'Shelf',parentId:null,depth:0,sortOrder:0}];
        if (innerWidth === 644) {
          docs.push(...Array.from({length:32}, (_, i) => ({...docs[1], title:'Document '+i, url:'/uploads/'+(100+i)+'.html'})));
          folders.push(...Array.from({length:32}, (_, i) => ({id:'folder'+i,name:'Folder '+i,parentId:'shelf',depth:1,sortOrder:i})));
        }
        const noop = () => {};
        createRoot(q('#root')).render(<DocumentsPanel allDocuments={docs} groupedDocs={[{author:'Uploads',docs}]}
          libraryOrganization={{folders,
            documentLocations:[{documentId:'aa',folderId:'shelf',sortOrder:0}]}} collapsedAuthors={new Set()} showDocuments
          documentFilter="" docFilterLower="" documentsLoading={false} onFilterChange={noop} onToggleShow={noop}
          importOptions={[{id:'batch', label:'Import files', onSelect:noop}]} onAudioSavedOnlyChange={noop}
          onToggleAuthor={noop} onViewDocument={noop} onCreateLibraryFolder={noop} onDeleteDocuments={noop}
          onDeleteLibraryFolder={noop} onMoveLibraryItems={noop} onRenameLibraryFolder={noop}/>);
        const manage = '.document-manage-btn';
        while (!q(manage)) await frame();
        const results = [];
        for (const index of [0,1,2]) {
          const category = () => document.querySelectorAll('.library-gallery-category')[index];
          if (index === 2) q('.library-view-toggle').click();
          else category().click();
          await frame();
          const group = q('.library-gallery-categories')?.getBoundingClientRect();
          const action = q(manage).getBoundingClientRect();
          const bar = q('.documents-list-header').getBoundingClientRect();
          const importButton = q('.document-upload-actions button:first-child').getBoundingClientRect();
          const fits = el => {
            const r = el.getBoundingClientRect();
            return r.width > 0 && r.left >= 0 && r.right <= innerWidth && el.scrollWidth <= el.clientWidth + 1;
          };
          const adjacent = Math.abs(importButton.top-action.top)<2 &&
            (narrow ? importButton.left >= action.right : importButton.right <= action.left);
          const centered = index === 2 ? !group : Math.abs(group.left+group.right-bar.left-bar.right)<2;
          const controls = document.querySelectorAll('.documents-list-header button, .library-gallery-category');
          const filter = q('.document-filter-input').getBoundingClientRect();
          const compactLayout = innerWidth > 700 || filter.bottom <= action.top;
          const icon = category()?.querySelector('svg[aria-hidden=true]')?.getBoundingClientRect();
          const label = category()?.querySelector('span')?.getBoundingClientRect();
          const leadingIcon = index === 2 || (icon && (narrow ? icon.left >= label.right : icon.right <= label.left));
          const layout = compactLayout && leadingIcon && adjacent && centered && Array.from(controls).every(fits)
            && q(manage).parentElement === q('.document-upload-actions') && !q('.uploaded-library-done');
          q(manage).click();
          await frame();
          const done = q('.uploaded-library-done');
          const entered = fits(done) && document.activeElement === done && fits(q(manage)) && q(manage).disabled
            && fits(q('.library-view-toggle')) && q('.library-view-toggle').disabled;
          done.click();
          await frame();
          results.push({layout: Boolean(layout), entered, returned: document.activeElement === q(manage) && !q(manage).disabled && !q('.library-view-toggle').disabled,
            view: index === 2 ? !q('.library-gallery-view') : category().getAttribute('aria-pressed') === 'true'});
        }
        if (innerWidth === 644) {
          q(manage).click();
          await frame();
          q('.uploaded-library-selection-actions button').click();
          await frame();
          q('.uploaded-library-move-trigger').click();
          await frame();
          q('.uploaded-library-destination').click();
          await frame();
          const body = q('.app-dialog-body');
          body.scrollTop = body.scrollHeight;
          if (!body.scrollTop) throw Error('Expected a scrolled folder list');
          q('.app-dialog-submit').click();
          await frame();
          const path = q('.uploaded-library-move-path').getBoundingClientRect();
          if (path.top < body.getBoundingClientRect().top - 1 || path.bottom > body.getBoundingClientRect().bottom ||
              document.activeElement !== q('.app-dialog-actions .app-dialog-cancel')) throw Error('Confirmation path clipped or Cancel unfocused');
        }
        if (innerWidth === 591) {
          q(manage).click();
          await frame();
          const trigger = q('[aria-label="Folder actions for Shelf"]');
          const icon = trigger.querySelector('svg');
          const glyph = icon.getBoundingClientRect();
          const button = trigger.getBoundingClientRect();
          if (glyph.width !== 22 || glyph.height !== 22 ||
              getComputedStyle(icon).fill !== getComputedStyle(trigger).color ||
              Math.abs(glyph.left + glyph.right - button.left - button.right) > 1 ||
              Math.abs(glyph.top + glyph.bottom - button.top - button.bottom) > 1) {
            throw Error('Folder menu dots should be centered, 22px square and match the button color');
          }
          trigger.click();
          await frame();
          const menu = q('.uploaded-library-folder-menu');
          const firstAction = menu.querySelector('[role="menuitem"]');
          firstAction.focus();
          firstAction.dispatchEvent(new KeyboardEvent('keydown', {key:'r', bubbles:true}));
          await frame();
          if (document.activeElement?.textContent !== 'Rename') throw Error('R should focus Rename');
          const width = menu.getBoundingClientRect().width;
          if (menu.querySelectorAll('[role="menuitem"]').length !== 3 || width <= 0 || width >= 200 ||
              menu.scrollWidth > menu.clientWidth + 1) {
            throw Error('Folder actions should fit their labels without the Import menu minimum width: ' + [width,menu.scrollWidth,menu.clientWidth,menu.querySelectorAll('[role=menuitem]').length]);
          }
        }
        if (narrow) {
          q(manage).click();
          await frame();
          q('.uploaded-library-selection-actions button').click();
          await frame();
          q('.uploaded-library-move-trigger').click();
          await frame();
          const path = q('.uploaded-library-move-path');
          const row = q('.uploaded-library-destination');
          if (getComputedStyle(row.lastElementChild).transform !== 'matrix(-1, 0, 0, 1, 0, 0)') throw Error('RTL folder chevron');
          // Enter opens Shelf. The next Tab and Enter activate its Root breadcrumb.
          const clicked = new Promise(resolve => path.addEventListener('click', resolve, {once:true}));
          row.focus();
          window.keyboardReady = true;
          await clicked;
          await frame();
          if (path.querySelector('button') || document.activeElement !== path ||
              !q('.uploaded-library-destination') || q('.uploaded-library-move-documents')) throw Error('Keyboard return to Root');
        }
        if (innerWidth === 900) {
          q(manage).click();
          await frame();
          const box = q('[aria-label="Select Shelf"]');
          box.focus();
          window.keyboardReady = true;
          await new Promise(resolve => box.addEventListener('change', resolve, {once:true}));
          await frame();
          if (!box.checked || !box.closest('[role=row]').classList.contains('uploaded-library-item-selected') ||
              q('.uploaded-library-selection-count').textContent !== 'Folders (1)') {
            throw Error('Space did not select the collapsed folder’s document');
          }
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
    for (const [width, direction] of [[900, 'ltr'], [644, 'ltr'], [591, 'ltr'], [460, 'ltr'], [320, 'rtl']] as const) {
      const result = await runBrowserFixture(join(directory, String(width)), join(directory, 'dist/index.html'), width,
        undefined, undefined, width === 900 ? ['Space'] : width === 320 ? ['Enter', 'Tab', 'Enter'] : undefined)
      expect(result, width + 'px').toEqual({
        direction,
        results: Array(3).fill({
          layout: true,
          entered: true,
          returned: true,
          view: true,
        }),
      })
    }
  } finally {
    rmSync(directory, {
      recursive: true,
      force: true,
    })
  }
}, 30_000)
