import { AppDialog } from '../AppDialog/AppDialog'

interface ExternalLinkPromptProps {
  url: string
  error?: string
  onCancel: () => void
  onOpen: () => void | Promise<void>
}

export function ExternalLinkPrompt({ url, error, onCancel, onOpen }: ExternalLinkPromptProps) {
  return (
    <AppDialog
      title="Open External Link?"
      description="This link will open outside Papercut."
      onCancel={onCancel}
      actions={(
        <>
          <button type="button" className="app-dialog-cancel" onClick={onCancel}>Cancel</button>
          <button type="button" className="app-dialog-submit" onClick={onOpen}>Open</button>
        </>
      )}
    >
      <code className="app-dialog-code">{url}</code>
      {error && <p className="app-dialog-error" role="alert">{error}</p>}
    </AppDialog>
  )
}
