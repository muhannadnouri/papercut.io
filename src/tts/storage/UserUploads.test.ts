import {
  expect,
  it,
  vi,
} from 'vitest'
import { getUserUploads } from './UserUploads'

it('retains native bundle imports with a null SILMA setting and rejects malformed settings', () => {
  const record = {
    url: '/user-uploads/abc.html',
    title: 'Lantern Field Notes',
    importedAt: 1,
    voice: 'af_heart',
    speed: 1,
    dtype: 'native',
    chunks: 1,
  }
  const raw = JSON.stringify([null, 'bad'].map(silmaNfeStep => ({
    ...record,
    silmaNfeStep,
  })))
  vi.stubGlobal('localStorage', { getItem: () => raw })
  try {
    const uploads = getUserUploads()
    expect(uploads).toHaveLength(1)
    expect(uploads[0]).toMatchObject(record)
    expect(uploads[0].silmaNfeStep).toBeUndefined()
  } finally {
    vi.unstubAllGlobals()
  }
})
