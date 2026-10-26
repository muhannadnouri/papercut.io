import { isPdfWordWrap } from '../pdf/pdfWordWrap'

/** Display only: reader targets always retain the original source text and offsets. */
export function contextPreview(text: string, format: string): string {
  const display = format === 'pdf'
    ? text.replace(/\r\n?/g, '\n').split(/\n[\t ]*\n+/).map(paragraph =>
      paragraph.split('\n').reduce((joined, line) => {
        const next = line.replace(/[\t ]+/g, ' ').trim()
        if (!next) {
          return joined
        }
        if (isPdfWordWrap(joined, next)) {
          return joined.slice(0, -1) + next
        }
        return joined ? `${joined} ${next}` : next
      }, ''),
    ).join('\n\n').trim()
    : text.trim()
  const characters = [...display]
  if (characters.length <= 400) {
    return display
  }
  const prefix = characters.slice(0, 400).join('')
  const boundary = prefix.search(/\s+\S*$/u)
  return `${(boundary > 0 ? prefix.slice(0, boundary) : prefix).trimEnd()}…`
}
