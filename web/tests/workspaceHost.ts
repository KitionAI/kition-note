/**
 * A test host with a workspace (tests/workspace.mjs): an in-memory folder of files, the replicas
 * it keeps beside them (capability `replica`, Kit UI protocol section 7), and two kit frames that
 * can edit one note together, their replicas relayed as a co-editing host does. Never shipped:
 * the package builds `index.html` and `dev.html` only.
 */
import type { KitPresence } from '@kition/kit-ui-protocol'
import { KitRpcError, mountKit, type KitConnection, type StoredReplica } from '@kition/kit-host'
import { resolveLink, type DevOpen } from '@kition/kit-dev-host'

type File = { bytes: ArrayBuffer; version: number }
type Slot = 'a' | 'b'

/** A `host/open` or `host/openUrl` the editor sent, as `mountKit` checked it. */
type OpenRequest = DevOpen

type WorkspaceHost = {
  files: Map<string, File>
  /** Every `host/open` and `host/openUrl` the editor sent, in order. */
  opens: OpenRequest[]
  /** The replicas the host keeps, by path (`host/replicaWrite`). */
  replicas: Map<string, StoredReplica>
  /** Opens `path` in frame `slot` (`a` by default). */
  open: (path: string, slot?: Slot) => Promise<void>
  /** Closes the editor in `slot`, as closing its tab does. */
  close: (slot?: Slot) => void
  connection: (slot?: Slot) => KitConnection | null
  /** Relays each frame's replica changes and presence to the other, as a co-editing host does. */
  coEdit: () => void
  /** The last presence each frame shared (capability `presence`). */
  presences: Record<Slot, KitPresence | null>
  /** Writes `text` to `path` as another program would (a new version). */
  write: (path: string, text: string) => void
  text: (path: string) => string
  /** The pictures on the web this host downloads (`host/fetchImage`), by address; any other address fails (`io-error`). */
  web: Map<string, { bytes: ArrayBuffer; mime: 'image/png' }>
  /** Every address the editor asked the host to download, in order. */
  fetched: string[]
}

declare global {
  interface Window {
    workspaceHost: WorkspaceHost
  }
}

const opens: OpenRequest[] = []
const web: WorkspaceHost['web'] = new Map()
const fetched: string[] = []

const status = document.getElementById('status') as HTMLElement
const frames: Record<Slot, HTMLIFrameElement> = { a: document.getElementById('kit-a') as HTMLIFrameElement, b: document.getElementById('kit-b') as HTMLIFrameElement }
const connections: Record<Slot, KitConnection | null> = { a: null, b: null }
/**
 * Each frame's `about:blank` from `close`, loaded (or a second gone by). `open` navigates only
 * after it: WebKit may otherwise finish the blank page after the kit's and leave the frame blank,
 * so the kit never announces itself.
 */
const blanked: Record<Slot, Promise<void>> = { a: Promise.resolve(), b: Promise.resolve() }
const files = new Map<string, File>()
const replicas = new Map<string, StoredReplica>()
const encode = (text: string) => new TextEncoder().encode(text).buffer as ArrayBuffer
let relays: Array<() => void> = []
/** The last presence each frame shared, and whether presence goes to the other frame (`coEdit`). */
const presences: Record<Slot, KitPresence | null> = { a: null, b: null }
let sharing = false
const PEOPLE: Record<Slot, { name: string; color: string }> = { a: { name: 'Person A', color: '#e11d48' }, b: { name: 'Person B', color: '#2563eb' } }
const other = (slot: Slot): Slot => (slot === 'a' ? 'b' : 'a')
/** Tells `to` what the other frame's person shares, as a sync server's `presence/peers` would. */
function sharePresence(to: Slot) {
  const from = other(to)
  const presence = presences[from]
  if (!sharing || !connections[from]) return
  connections[to]?.peersChanged(presence ? [{ principal: `person-${from}`, ...PEOPLE[from], ...presence }] : [])
}

async function relay(from: KitConnection, to: KitConnection) {
  const since = await to.replica.version()
  const { changes, blobs } = await from.replica.exportChanges(since)
  if (changes.byteLength) await to.replica.importChanges(changes, blobs)
}

const host: WorkspaceHost = {
  files,
  opens,
  web,
  fetched,
  replicas,
  connection: (slot = 'a') => connections[slot],
  write: (path, text) => {
    files.set(path, { bytes: encode(text), version: (files.get(path)?.version ?? 0) + 1 })
  },
  text: (path) => new TextDecoder().decode(new Uint8Array(files.get(path)?.bytes ?? new ArrayBuffer(0))),
  close: (slot = 'a') => {
    connections[slot]?.close()
    connections[slot] = null
    presences[slot] = null
    sharing = false
    const frame = frames[slot]
    blanked[slot] = new Promise((resolve) => {
      const timer = window.setTimeout(resolve, 1000)
      frame.addEventListener(
        'load',
        () => {
          window.clearTimeout(timer)
          resolve()
        },
        { once: true },
      )
    })
    frame.src = 'about:blank'
  },
  coEdit: () => {
    for (const stop of relays) stop()
    const a = connections.a
    const b = connections.b
    if (!a || !b) return
    relays = [a.replica.onChanged(() => void relay(a, b)), b.replica.onChanged(() => void relay(b, a))]
    sharing = true
    sharePresence('a')
    sharePresence('b')
  },
  presences,
  open: async (path, slot = 'a') => {
    connections[slot]?.close()
    const file = files.get(path)
    if (!file) throw new Error(`${path} is not in the workspace`)
    await blanked[slot]
    frames[slot].src = `../index.html?${Date.now()}`
    status.textContent = `opening ${path}…`
    connections[slot] = await mountKit(frames[slot], {
      origin: location.origin,
      host: { name: 'Workspace test host', version: '1' },
      document: { path, name: path.split('/').pop() ?? path, bytes: file.bytes.slice(0), version: String(file.version) },
      theme: { colorScheme: 'light', tokens: { '--kit-primary': '#5645d4' } },
      locale: 'en-US',
      shortcuts: [],
      handlers: {
        read: ({ path: wanted }) => {
          const found = files.get(wanted)
          if (!found) throw new Error(`${wanted} is not in the workspace`)
          return { bytes: found.bytes.slice(0), version: String(found.version), size: found.bytes.byteLength }
        },
        stamp: ({ path: wanted }) => ({ version: files.has(wanted) ? String(files.get(wanted)!.version) : null }),
        write: ({ path: wanted, bytes }) => {
          const version = (files.get(wanted)?.version ?? 0) + 1
          files.set(wanted, { bytes, version })
          return { path: wanted, version: String(version) }
        },
        replicaRead: ({ path: wanted }) => replicas.get(wanted) ?? null,
        replicaWrite: ({ path: wanted, ...stored }) => {
          replicas.set(wanted, stored)
        },
        changed: () => {},
        presence: (presence) => {
          presences[slot] = presence
          sharePresence(other(slot))
        },
        // Opening (KitLinks 1.0 section 5): every request is kept; a file the workspace does not
        // have is `not-found` unless the request asks to create it, which makes an empty note.
        open: async (params) => {
          opens.push({ method: 'host/open', params })
          const target = resolveLink(params.link, path, files)
          if (!target) throw new KitRpcError(-32000, 'the link is not in the workspace', { reason: 'not-found' })
          if (!target.exists) {
            if (!params.create) throw new KitRpcError(-32000, `${target.path} not found`, { reason: 'not-found' })
            files.set(target.path, { bytes: encode(''), version: 1 })
          }
          return { opened: true }
        },
        openUrl: async (params) => {
          opens.push({ method: 'host/openUrl', params })
          return { opened: true }
        },
        // Pictures pasted by their web address (section 7l), from this host's own little web.
        fetchImage: async ({ url }) => {
          fetched.push(url)
          const picture = web.get(url)
          if (!picture) throw new KitRpcError(-32000, `${url} could not be downloaded`, { reason: 'io-error' })
          return { bytes: picture.bytes.slice(0), mime: picture.mime }
        },
      },
    })
    status.textContent = `${path} open in ${slot}`
  },
}
window.workspaceHost = host
status.textContent = 'ready'
