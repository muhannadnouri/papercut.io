import { AppDialog } from '../../components/AppDialog/AppDialog'
import {
  AudioSetupPanel,
  type AudioSetupPanelProps,
} from './AudioSetupPanel'

interface AudioSetupDialogProps {
  audioSetup: AudioSetupPanelProps
  onClose: () => void
  title: string
}

/** Reuses the same model-install and voice settings dialog from the reader and Audiobooks tab. */
export function AudioSetupDialog({ audioSetup, onClose, title }: AudioSetupDialogProps) {
  return (
    <AppDialog
      className="audiobooks-setup-dialog"
      title={title}
      showCloseButton
      onCancel={onClose}
    >
      <AudioSetupPanel {...audioSetup} />
    </AppDialog>
  )
}
