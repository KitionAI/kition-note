/**
 * The note's images in the host's workspace. The engine draws an image-only line's picture;
 * this module hands it the bytes: every picture the engine asks for (`image.wanted`), written
 * with a path relative to the note's folder (or from the workspace root with a leading `/`), is
 * read through the host (`host/read`) and given to the engine (`image.set`). A pasted or dropped
 * image is kept as a new workspace file in the note's attachments folder
 * (`attachments/<note path>/`, through `host/write`'s `create` mode, which never replaces a file),
 * or beside the note as a new file the host creates (KitFiles `files/create`) when the host serves
 * files; the note links it relative to its folder. The kit loads nothing from the network (UI-K1):
 * a picture pasted by its web address is downloaded by the host (`host/fetchImage`, capability
 * `fetch-image`) and kept the same way.
 */
import type { HostConnection } from '@kition/sdk/editor'

const EXTENSIONS: Record<string, string> = { 'image/png': '.png', 'image/jpeg': '.jpg', 'image/gif': '.gif', 'image/webp': '.webp', 'image/svg+xml': '.svg', 'image/avif': '.avif' }
const SCHEME = /^[a-z][a-z\d+.-]*:/i

/** The folder of the workspace file at `path` (`''` at the root). */
const folderOf = (path: string) => path.split('/').filter(Boolean).slice(0, -1).join('/')

/**
 * The workspace path of an image written as `source` in the note at `documentPath`: relative to
 * the note's folder, or from the workspace root with a leading `/`. `null` for an address (a
 * scheme, `//host`), an empty source, or a path above the workspace.
 */
export function workspaceImagePath(source: string, documentPath: string): string | null {
  let raw = source.trim()
  if (raw.startsWith('<') && raw.endsWith('>')) raw = raw.slice(1, -1).trim()
  raw = raw.split(/[?#]/)[0] ?? ''
  if (!raw || raw.startsWith('//') || SCHEME.test(raw)) return null
  try {
    raw = decodeURIComponent(raw)
  } catch {
    // A stray `%` stays as written.
  }
  const parts: string[] = []
  for (const part of `${raw.startsWith('/') ? '' : folderOf(documentPath)}/${raw}`.split('/')) {
    if (!part || part === '.') continue
    if (part === '..') {
      if (!parts.length) return null
      parts.pop()
    } else parts.push(part)
  }
  return parts.length ? parts.join('/') : null
}

/** `to` (a workspace path) relative to the folder of the note at `from`. */
export function relativeTo(from: string, to: string) {
  const base = folderOf(from).split('/').filter(Boolean)
  const target = to.split('/').filter(Boolean)
  let shared = 0
  while (shared < base.length && shared < target.length - 1 && base[shared] === target[shared]) shared += 1
  return [...base.slice(shared).map(() => '..'), ...target.slice(shared)].join('/')
}

/** Base64 of `bytes`, in chunks (a large picture is beyond one `String.fromCharCode` call). */
function base64(bytes: Uint8Array) {
  let binary = ''
  for (let at = 0; at < bytes.length; at += 0x8000) binary += String.fromCharCode(...bytes.subarray(at, at + 0x8000))
  return btoa(binary)
}

/** A copy of `bytes` in an ArrayBuffer of its own, to transfer. */
const own = (bytes: Uint8Array) => bytes.slice().buffer as ArrayBuffer

type Run = (command: string, params: Record<string, unknown>) => Promise<unknown>

/** Hands the engine the pictures it asks for, each once (a failed read is recorded, not retried until reopened). */
export async function loadWantedImages(host: HostConnection, wanted: string[], run: Run, tried: Set<string>) {
  const documentPath = host.init.document.path
  for (const name of wanted) {
    if (tried.has(name)) continue
    tried.add(name)
    const path = workspaceImagePath(name, documentPath)
    let data = ''
    if (path) {
      try {
        const { bytes } = (await host.peer.request('host/read', { path })) as { bytes: ArrayBuffer }
        data = base64(new Uint8Array(bytes))
      } catch {
        data = ''
      }
    }
    await run('image.set', { name, data }).catch(() => undefined)
  }
}

/** Keeps `file` (an image) as a new workspace file; answers its workspace path. */
export async function keepImage(host: HostConnection, file: Blob, index: number) {
  const documentPath = host.init.document.path
  const mime = file.type || 'image/png'
  const extension = EXTENSIONS[mime]
  if (!extension) throw new Error(`${mime} is not an image the note keeps`)
  const name = `pasted-${Date.now().toString(36)}-${index.toString(36)}${extension}`
  const bytes = new Uint8Array(await file.arrayBuffer())
  if (host.kitFiles.served) {
    const created = await host.kitFiles.create(name, { mime })
    await host.kitFiles.write(created.handle, bytes)
    return [folderOf(documentPath), created.name].filter(Boolean).join('/')
  }
  const folder = `attachments/${documentPath}`
  return ((await host.peer.request('host/write', { path: `${folder}/${name}`, bytes: own(bytes), mode: 'create' }, { transfer: true })) as { path: string }).path
}

/** The link target of a kept image (a workspace path) from the note: relative to its folder. */
const linkTarget = (host: HostConnection, kept: string) => encodeURI(relativeTo(host.init.document.path, kept))

/** The Markdown that links a kept image from the note: `![name](relative path)`. */
export function imageMarkdown(host: HostConnection, kept: string, file: File | Blob) {
  const alt = file instanceof File ? file.name.replace(/\.[^.]+$/, '') : ''
  return `![${alt}](${linkTarget(host, kept)})`
}

/** Whether the host downloads pictures pasted by their web address (Kit UI protocol section 7l). */
export const canFetchImages = (host: HostConnection) => host.init.capabilities.includes('fetch-image')

/** Downloads the picture at `url` through the host and keeps it as a new workspace file; answers its link target. */
export async function keepFetchedImage(host: HostConnection, url: string, index: number) {
  const { bytes, mime } = await host.peer.request('host/fetchImage', { url })
  return linkTarget(host, await keepImage(host, new Blob([bytes], { type: mime }), index))
}
