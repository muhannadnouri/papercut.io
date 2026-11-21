import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import {
  Button,
  Menu,
  MenuItem,
  MenuTrigger,
  Popover,
} from 'react-aria-components'

export interface DocumentImportOption {
  id: string
  label: string
  detail?: string
  statusLabel?: string
  disabled?: boolean
  future?: boolean
  onSelect?: () => void
}

interface DocumentImportMenuProps {
  importOptions: DocumentImportOption[]
  importBusy: boolean
}

export function DocumentImportMenu({ importOptions, importBusy }: DocumentImportMenuProps) {
  const { t } = useTranslation()
  const [importMenuOpen, setImportMenuOpen] = useState(false)
  const activeImport = importOptions.find((option) => option.statusLabel)
  const importDisabled = importOptions.every((option) => option.disabled || option.future || !option.onSelect)

  return (
    <MenuTrigger isOpen={importMenuOpen} onOpenChange={setImportMenuOpen}>
      <Button
        className={`document-import-btn${importBusy ? ' document-import-btn-busy' : ''}`}
        isDisabled={importDisabled}
        aria-label={importBusy
          ? activeImport?.statusLabel ?? t('library.import.importingBatch')
          : undefined}
      >
        <span className="document-import-btn-label" aria-hidden={importBusy ? true : undefined}>
          {t('library.documents.import')}
          <span className={`toggle-arrow ${importMenuOpen ? 'open' : ''}`} aria-hidden="true">&#9662;</span>
        </span>
        {importBusy && <span className="spinner document-import-btn-spinner" aria-hidden="true" />}
      </Button>
      <Popover
        className="document-import-popover"
        placement="bottom end"
        offset={6}
        containerPadding={8}
        shouldFlip
      >
        <Menu className="document-import-options" aria-label={t('library.documents.import')}>
          {importOptions.map((option) => {
            const disabled = option.disabled || option.future || !option.onSelect
            const label = option.label + (option.future ? ` (${t('library.documents.future')})` : '')
            return (
              <MenuItem
                key={option.id}
                id={option.id}
                className="document-import-option"
                isDisabled={disabled}
                textValue={label}
                aria-label={option.detail ? `${label}. ${option.detail}` : label}
                onAction={option.onSelect}
              >
                <ImportOptionIcon id={option.id} />
                <span className="document-import-option-copy">
                  <span>{label}</span>
                  {option.detail && <small>{option.detail}</small>}
                </span>
              </MenuItem>
            )
          })}
        </Menu>
      </Popover>
    </MenuTrigger>
  )
}

function ImportOptionIcon({ id }: { id: string }) {
  const paths: Record<string, string> = {
    batch: 'M8 3h8l5 5v13H8ZM16 3v5h5M5 7H3v14',
    folder: 'M3 6h6l3 3h9v10H3Z',
    paste: 'M9 5H5v16h14V5h-4M9 3h6v4H9ZM8 11h8M8 15h6',
    scan: 'M7 3H3v4M17 3h4v4M21 17v4h-4M7 21H3v-4M8 7h8v10H8Z',
    photos: 'M3 4h18v16H3ZM3 16l6-6 5 5 3-3 4 4',
  }
  return paths[id] ? (
    <svg className="document-import-option-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d={paths[id]} />
    </svg>
  ) : null
}
