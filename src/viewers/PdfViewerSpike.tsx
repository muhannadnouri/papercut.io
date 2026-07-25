import { useEffect, useState } from 'react'
import { PdfViewer } from './PdfViewer'

interface SelectedPdf {
  name: string
  url: string
}

export function PdfViewerSpike() {
  const [pdf, setPdf] = useState<SelectedPdf | null>(null)

  useEffect(() => () => {
    if (pdf) URL.revokeObjectURL(pdf.url)
  }, [pdf])

  return (
    <main className="pdf-spike">
      <h1>PDF.js WebView Spike</h1>
      <label>
        Choose a local PDF
        <input
          type="file"
          accept="application/pdf,.pdf"
          onChange={(event) => {
            const file = event.currentTarget.files?.[0]
            setPdf(file ? { name: file.name, url: URL.createObjectURL(file) } : null)
          }}
        />
      </label>
      {pdf && (
        <>
          <h2 dir="auto">{pdf.name}</h2>
          <PdfViewer url={pdf.url} />
        </>
      )}
    </main>
  )
}
