/**
 * A test host that keeps the note's comment threads itself (tests/comments.mjs), as Kition One
 * does (Kit UI protocol 2.6 section 7k): it serves `comment.create` (a new thread on the kit's
 * anchor, its id answered), shows its threads to the kit (`comments/show`), asks where anchors are
 * (`comments/locate`), and hears which marker the person clicked (`comments/activated`). The role
 * picks the document's permissions: `editor`, `commenter` (read-only, may comment), or `viewer`.
 * Never shipped: the package builds `index.html` and `dev.html` only.
 */
import { mountKit, type KitConnection } from '@kition/kit-host'
import type { KitCommentAnchor, KitShownThread } from '@kition/kit-ui-protocol'

type Role = 'editor' | 'commenter' | 'viewer'

type CommentsHost = {
  open: (name: string, text: string, role?: Role) => Promise<void>
  connection: () => KitConnection | null
  /** Every `comment.create` the kit sent, in order. */
  created: Array<{ anchor: KitCommentAnchor; text?: string }>
  /** Every thread the person clicked (`comments/activated`). */
  activated: string[]
  /** The host's threads; `show` sends them all to the kit. */
  threads: KitShownThread[]
  show: () => void
  locate: (anchors: KitCommentAnchor[]) => Promise<boolean[]>
}

declare global {
  interface Window {
    commentsHost: CommentsHost
  }
}

const frame = document.getElementById('kit') as HTMLIFrameElement
const status = document.getElementById('status') as HTMLElement
let bytes = new ArrayBuffer(0)
let version = 1
let connection: KitConnection | null = null
let next = 1

const host: CommentsHost = {
  created: [],
  activated: [],
  threads: [],
  connection: () => connection,
  show: () => connection?.showComments(host.threads),
  locate: async (anchors) => (connection ? connection.locateComments(anchors) : anchors.map(() => false)),
  open: async (name, text, role = 'editor') => {
    connection?.close()
    bytes = new TextEncoder().encode(text).buffer
    version = 1
    frame.src = `../index.html?${Date.now()}`
    connection = await mountKit(frame, {
      origin: location.origin,
      host: { name: 'Comments test host', version: '1' },
      document: {
        path: name,
        name,
        bytes: bytes.slice(0),
        version: String(version),
        ...(role === 'editor' ? {} : { readOnly: true }),
        comments: { me: 'person-1', name: 'Person One', canComment: role !== 'viewer', othersReadOnly: false },
      },
      theme: { colorScheme: 'light', tokens: { '--kit-primary': '#5645d4' } },
      locale: 'en-US',
      shortcuts: [],
      handlers: {
        read: () => ({ bytes: bytes.slice(0), version: String(version), size: bytes.byteLength }),
        stamp: () => ({ version: String(version) }),
        write: ({ bytes: written }) => {
          bytes = written
          version += 1
          return { path: name, version: String(version) }
        },
        changed: () => {},
        services: {
          // The host writes the comment itself (One asks the person beside the document); here the
          // thread is made at once.
          'comment.create': ({ anchor, text: body }) => {
            host.created.push({ anchor, ...(body !== undefined ? { text: body } : {}) })
            const id = `thread-${next++}`
            host.threads.push({ id, anchor, resolved: false, count: 1 })
            window.setTimeout(() => host.show(), 0)
            return { thread: id }
          },
        },
        comments: {
          activated: (id) => {
            host.activated.push(id)
          },
        },
      },
    })
    status.textContent = `${name} open as ${role}`
    host.show()
  },
}
window.commentsHost = host
status.textContent = 'ready'
