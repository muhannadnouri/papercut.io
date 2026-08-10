import { AppDialog } from '../../components/AppDialog/AppDialog'
import { AudioSetupPanel, type AudioSetupPanelProps } from './AudioSetupPanel'

interface AudioSetupDialogProps {
  audioSetup: AudioSetupPanelProps
  onClose: () => void
  title: string
  doneLabel: string
}

/** Reuses the same model-install and voice settings dialog from the reader and Audiobooks tab. */
export function AudioSetupDialog({ audioSetup, onClose, title, doneLabel }: AudioSetupDialogProps) {
  return (
    <AppDialog
      className="audiobooks-setup-dialog"
      title={title}
      onCancel={onClose}
      actions={(
        <button type="button" className="app-dialog-submit" onClick={onClose}>
          {doneLabel}
        </button>
      )}
    >
      <AudioSetupPanel {...audioSetup} />
    </AppDialog>
  )
}
