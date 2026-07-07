import type { ComponentProps } from 'react'
import { AudiobooksPanel } from '../../tts/components/AudiobooksPanel'
import { TtsDiagnosticsPanel } from '../../tts/components/TtsDiagnosticsPanel'

type AudiobooksPanelProps = ComponentProps<typeof AudiobooksPanel>
type AudioSetupProps = Omit<AudiobooksPanelProps['audioSetup'], 'debugEnabled' | 'onDiagnosticsChange'>

interface AudiobooksTabProps {
  audiobooksPanelProps: Omit<
    AudiobooksPanelProps,
    'audioSetup' | 'documentOpening' | 'importState' | 'onImportAudiobook' | 'onOpenSaved'
  >
  audioSetupProps: AudioSetupProps
  audiobookImport: AudiobooksPanelProps['importState']
  documentOpening: boolean
  ttsDiagnosticsEnabled: boolean
  onDiagnosticsChange: (enabled: boolean) => void
  onImportAudiobook: AudiobooksPanelProps['onImportAudiobook']
  onOpenSaved: AudiobooksPanelProps['onOpenSaved']
}

export function AudiobooksTab({
  audiobooksPanelProps,
  audioSetupProps,
  audiobookImport,
  documentOpening,
  ttsDiagnosticsEnabled,
  onDiagnosticsChange,
  onImportAudiobook,
  onOpenSaved,
}: AudiobooksTabProps) {
  return (
    <section className="tab-panel" role="tabpanel" aria-label="Audiobooks" data-tab="audiobooks">
      <AudiobooksPanel
        {...audiobooksPanelProps}
        audioSetup={{
          ...audioSetupProps,
          debugEnabled: ttsDiagnosticsEnabled,
          onDiagnosticsChange,
        }}
        importState={audiobookImport}
        documentOpening={documentOpening}
        onImportAudiobook={onImportAudiobook}
        onOpenSaved={onOpenSaved}
      />

      <TtsDiagnosticsPanel enabled={ttsDiagnosticsEnabled} />
    </section>
  )
}
