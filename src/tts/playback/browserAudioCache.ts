export interface LoadedBrowserAudio {
  index: number
  chunkId: string
  text: string
  wav: ArrayBuffer
}

export interface QueuedBrowserAudio {
  index: number
  chunkId: string
  text: string
  url: string
}

/** Own the bounded desktop WAV window and every Blob URL created for it. */
export class BrowserAudioCache {
  private readonly items = new Map<number, QueuedBrowserAudio>()
  private readonly loadedIndexes = new Set<number>()
  private readonly loading = new Map<number, Promise<LoadedBrowserAudio | null>>()

  get generatedCount(): number {
    return this.loadedIndexes.size
  }

  has(index: number): boolean {
    return this.items.has(index)
  }

  get(index: number): QueuedBrowserAudio | undefined {
    return this.items.get(index)
  }

  getLoading(index: number): Promise<LoadedBrowserAudio | null> | undefined {
    return this.loading.get(index)
  }

  // Share concurrent reads for one chunk and remove only the promise that this
  // call registered, so an older completion cannot clear a newer retry.
  trackLoading(index: number, promise: Promise<LoadedBrowserAudio | null>): void {
    this.loading.set(index, promise)
    const clear = () => {
      if (this.loading.get(index) === promise) this.loading.delete(index)
    }
    void promise.then(clear, clear)
  }

  enqueue(loaded: LoadedBrowserAudio): void {
    const previous = this.items.get(loaded.index)
    if (previous) URL.revokeObjectURL(previous.url)

    this.items.set(loaded.index, {
      index: loaded.index,
      chunkId: loaded.chunkId,
      text: loaded.text,
      url: URL.createObjectURL(new Blob([loaded.wav], { type: 'audio/wav' })),
    })
    this.loadedIndexes.add(loaded.index)
  }

  // Keep the current chunk plus a small look-behind/look-ahead window. Loaded
  // indexes remain counted because the UI reports chunks fetched during this job.
  prune(anchorIndex: number, currentIndex: number | null, totalChunks: number): void {
    const min = Math.max(anchorIndex - 2, 0)
    const max = Math.min(anchorIndex + 4, Math.max(totalChunks - 1, 0))

    for (const item of this.items.values()) {
      if (item.index >= min && item.index <= max) continue
      if (item.index === currentIndex) continue
      URL.revokeObjectURL(item.url)
      this.items.delete(item.index)
    }
  }

  clear(): void {
    for (const item of this.items.values()) URL.revokeObjectURL(item.url)
    this.items.clear()
    this.loadedIndexes.clear()
    this.loading.clear()
  }
}
