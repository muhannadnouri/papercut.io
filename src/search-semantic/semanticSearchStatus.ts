import type { SemanticSearchController } from './useSemanticSearch'
import { isIndexingCancelled } from './indexingOutcome'

/** Saved opt-in is not readiness; both Settings views must describe the same observed state. */
export function semanticSearchStatus({ enabled, format, provider, index, operation, error, statusError, cancelling }: SemanticSearchController) {
  if (operation === 'disable') {
    return 'disabling'
  }
  if (!enabled) {
    return 'off'
  }
  if (operation === 'activate' || provider?.loadingFormat) {
    return 'loading'
  }
  if (statusError) {
    return 'statusUnavailable'
  }
  if (!provider?.compiled || provider.activeFormat !== format) {
    return 'fallback'
  }
  if (index?.running || operation === 'build') {
    return cancelling || index?.cancelRequested ? 'cancelling' : 'indexing'
  }
  if ((error && !isIndexingCancelled(error)) || (index?.error && !isIndexingCancelled(index.error)) || operation === 'remove') {
    return 'fallback'
  }
  if (!index) {
    return 'checking'
  }
  if (index.eligibleDocuments === 0) {
    return 'empty'
  }
  return index.indexedDocuments === index.eligibleDocuments ? 'ready' : 'needsIndex'
}
