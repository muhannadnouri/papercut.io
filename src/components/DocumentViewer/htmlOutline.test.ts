// @vitest-environment jsdom
import {
  expect,
  it,
} from 'vitest'
import { buildHtmlOutline } from './htmlOutline'

function outline(markup: string) {
  const article = document.createElement('article')
  article.innerHTML = markup
  document.body.append(article)
  try {
    return buildHtmlOutline(article)
  } finally {
    article.remove()
  }
}

it('keeps document order, nests skipped ranks, and distinguishes repeated labels by ordinal', () => {
  expect(outline(`
    <h2 data-papercut-section="0">  دليل   <em>البدء</em></h2>
    <h4 data-papercut-section="2">Examples</h4>
    <h2 data-papercut-section="4">Examples</h2>`)).toEqual([
    {
      label: 'دليل البدء',
      target: '0',
      children: [{
        label: 'Examples',
        target: '2',
        children: [],
      }],
    },
    {
      label: 'Examples',
      target: '4',
      children: [],
    },
  ])
})

it('counts nested headings and excludes only navigation regions', () => {
  expect(outline(`
    <nav><section><h1 data-papercut-section="0">Menu</h1></section></nav>
    <aside role="doc-index navigation"><h2 data-papercut-section="1">Menu</h2></aside>
    <section role="doc-toc doc-index"><h3 data-papercut-section="2">Contents</h3></section>
    <section epub:type="toc index"><h4 data-papercut-section="3">Links</h4></section>
    <h5 data-papercut-section="4">Notes</h5>
    <aside role="doc-bibliography"><h6 data-papercut-section="5">References</h6></aside>`)).toEqual([
    {
      label: 'Notes',
      target: '4',
      children: [{
        label: 'References',
        target: '5',
        children: [],
      }],
    },
  ])
})

it('requires two nonempty headings with usable section ordinals', () => {
  expect(outline(`
    <h1 data-papercut-section="1">Only usable heading</h1>
    <h2 data-papercut-section="2">  </h2>
    <h2 id="author-id">Missing ordinal</h2>
    <h3 data-papercut-section="01">Leading zero</h3>
    <h4 data-papercut-section="-1">Negative</h4>
    <h5 data-papercut-section="9007199254740992">Unsafe integer</h5>`)).toEqual([])
})
