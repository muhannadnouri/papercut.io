// Report teardown errors separately when the case already failed. A cleanup-only failure still fails the case.
export async function withCleanup(label, run, cleanup) {
  let failed = false
  try {
    return await run()
  } catch (error) {
    failed = true
    throw error
  } finally {
    try {
      await cleanup()
    } catch (error) {
      if (!failed) {
        throw error
      }
      console.error(label, '(secondary cleanup failure)', error)
    }
  }
}
