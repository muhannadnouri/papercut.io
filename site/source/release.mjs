// Pin the published application release; voice models have independent GitHub releases.
const version = '1.11.2'
const tag = `v${version}`
const repository = 'https://github.com/muhannadnouri/papercut.io'
const download = `${repository}/releases/download/${tag}`
const archives = `${repository}/archive/refs/tags`

const assets = {
  apk: `Papercut_${tag}_android-arm64-v8a.apk`,
  appimage: `Papercut_${version}_amd64.AppImage`,
  deb: `Papercut_${version}_amd64.deb`,
  rpm: `Papercut-${version}-1.x86_64.rpm`,
  'mac-arm': `Papercut_${version}_aarch64.dmg`,
  'mac-intel': `Papercut_${version}_x64.dmg`,
  msi: `Papercut_${version}_x64_en-US.msi`,
  exe: `Papercut_${version}_x64-setup.exe`,
  checksums: 'SHA256SUMS.txt',
  zip: `${tag}.zip`,
  tar: `${tag}.tar.gz`,
}

export const release = { 'release.tag': tag }
for (const [key, filename] of Object.entries(assets)) {
  const base = key === 'zip' || key === 'tar' ? archives : download
  release[`release.${key}.name`] = filename
  release[`release.${key}.url`] = `${base}/${filename}`
}
