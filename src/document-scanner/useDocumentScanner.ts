import { useEffect, useState } from 'react'
import { getDocumentScannerAvailability } from './documentScanner'

/** Probe once per app mount; unsupported platforms simply omit scanner UI. */
export function useDocumentScanner() {
  const [supported, setSupported] = useState(false)
  const [photoImportSupported, setPhotoImportSupported] = useState(false)

  useEffect(() => {
    let cancelled = false
    getDocumentScannerAvailability()
      .then((availability) => {
        if (!cancelled) {
          setSupported(availability.supported)
          setPhotoImportSupported(availability.photoImportSupported)
        }
      })
      .catch((error) => {
        console.warn('Unable to check document scanner availability:', error)
      })
    return () => {
      cancelled = true
    }
  }, [])

  return { supported, photoImportSupported }
}
