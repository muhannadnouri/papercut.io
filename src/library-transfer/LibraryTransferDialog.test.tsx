import { renderToStaticMarkup } from 'react-dom/server'
import {
  describe,
  expect,
  it,
  vi,
} from 'vitest'
import {
  LibraryNetworkFlow,
  LibraryTransferCompletion,
  TransferPreparationMessage,
} from './LibraryTransferDialog'

vi.mock('react-i18next', async (importOriginal) => ({
  ...await importOriginal<typeof import('react-i18next')>(),
  useTranslation: () => ({ t: (key: string) => key }),
}))

vi.mock('../i18n', () => ({
  currentAppLocale: () => 'en',
  default: {},
}))

const locales = import.meta.glob<{ libraryTransfer: Record<string, unknown> }>('../i18n/locales/*.json', {
  eager: true,
  import: 'default',
})
it.each(Object.entries(locales))('uses the actual pairing-action labels in %s instructions', (_path, locale) => {
  const copy = locale.libraryTransfer
  for (const [instruction, labels] of [
    ['sendDescription', ['modeReceive', 'showPairingDetails']],
    ['senderListensDescription', ['modeReceive', 'otherConnectionMethod', 'useSenderCode']],
    ['receiverConnectsDescription', ['modeSend', 'continueToConnection', 'otherConnectionMethod', 'showCodeHere']],
    ['receiverPairingStep', ['modeSend', 'continueToConnection']],
    ['senderPairingStep', ['modeReceive', 'otherConnectionMethod', 'useSenderCode']],
  ] as const) {
    for (const label of labels) {
      expect(copy[instruction]).toContain(copy[label])
    }
  }
})

describe('library transfer send flow', () => {
  it('announces indeterminate package preparation', () => {
    const html = renderToStaticMarkup(<TransferPreparationMessage />)

    expect(html).toContain('role="status"')
    expect(html).toContain('aria-live="polite"')
    expect(html).toContain('class="spinner"')
    expect(html).toContain('libraryTransfer.preparingSelection')
  })

  it('prioritizes pairing instructions while waiting for the receiving device', () => {
    const html = renderToStaticMarkup(
      <LibraryNetworkFlow
        role="sender"
        starting={false}
        locale="en"
        onRetry={() => undefined}
        onExit={() => undefined}
        status={{
          id: 1,
          listenerReady: true,
          addresses: ['192.168.1.20:49152'],
          role: 'sender',
          mode: 'pull',
          state: 'waiting',
          expiresAtMs: null,
          cancellable: true,
          canRetry: false,
          receiverProgress: null,
          error: null,
          result: null,
          code: 'ABCD-EFGH-JKLM',
          documents: 2,
          audiobooks: 1,
          packageBytes: 4096,
          bytesTransferred: 0,
        }}
      />,
    )

    expect(html).toContain('<ol>')
    expect(html).toContain('libraryTransfer.senderPairingStep')
    expect(html).toContain('libraryTransfer.connectStepEnter')
    expect(html).toContain('192.168.1.20:49152')
    expect(html).toContain('ABCD-EFGH-JKLM')
    expect(html).toContain('libraryTransfer.cancelTransfer')
  })

  it('summarizes completed sends without claiming receiver-side import counts', () => {
    const html = renderToStaticMarkup(
      <LibraryTransferCompletion
        kind="send"
        status={{
          id: 1,
          listenerReady: true,
          addresses: ['192.168.1.20:49152'],
          role: 'sender',
          mode: 'push',
          state: 'complete',
          expiresAtMs: null,
          cancellable: false,
          canRetry: false,
          receiverProgress: null,
          error: null,
          result: null,
          code: 'ABCD-EFGH-JKLM',
          documents: 2,
          audiobooks: 1,
          packageBytes: 4096,
          bytesTransferred: 4096,
        }}
      />,
    )

    expect(html).toContain('libraryTransfer.completionTitle')
    expect(html).toContain('libraryTransfer.sendComplete')
    expect(html).toContain('libraryTransfer.documentsSent')
    expect(html).toContain('libraryTransfer.audiobooksSent')
    expect(html).not.toContain('libraryTransfer.importComplete')
  })

  it('makes partial receiver results and failure details explicit', () => {
    const html = renderToStaticMarkup(
      <LibraryTransferCompletion
        kind="receive"
        result={{
          selected: 2,
          imported: 1,
          skipped: 0,
          failed: 1,
          foldersCreated: 0,
          audiobooksSelected: 0,
          audiobooksImported: 0,
          audiobooksSkipped: 0,
          audiobooksFailed: 0,
          importedAudiobooks: [],
          failures: [{
            item: 'Damaged book',
            error: 'Checksum failed',
          }],
        }}
      />,
    )

    expect(html).toContain('libraryTransfer.completionPartialTitle')
    expect(html).toContain('libraryTransfer.importComplete')
    expect(html).toContain('libraryTransfer.failureDetails')
    expect(html).toContain('Damaged book')
    expect(html).toContain('Checksum failed')
  })
})

it('describes receiver preparation without suggesting its local content is being packaged', () => {
  const html = renderToStaticMarkup(<LibraryNetworkFlow role="receiver" starting status={null} locale="en"
    onExit={() => undefined} onRetry={() => undefined} />)
  expect(html).toContain('libraryTransfer.receiveTitle')
  expect(html).toContain('libraryTransfer.exporting')
  expect(html).not.toContain('libraryTransfer.preparingSelection')
})
