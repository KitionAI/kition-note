/**
 * A test host for the note's AI review (tests/ai.mjs): serves `host/generate` with scripted
 * answers or refusals, the host action `design.fromRecord`, and `host/openAgent`, over one
 * in-memory note. Never shipped: the package builds `index.html` and `dev.html` only.
 */
import { KitRpcError, mountKit, type KitConnection, type KitGenerateParams } from '@kition/sdk/host'

type Answer = { text: string } | { refuse: string }

type AiHost = {
  open: (name: string, text: string) => Promise<void>
  connection: () => KitConnection | null
  /** The next answers of `host/generate`, in order; with none queued the host refuses with `unavailable`. */
  answers: Answer[]
  /** Holds each `host/generate` until `release()` (a slow model). */
  hold: boolean
  release: () => void
  generated: KitGenerateParams[]
  actions: Array<{ id: string; params: Record<string, unknown> }>
  agent: Array<string | undefined>
  file: () => string
  /** `?generate=0` mounts without the capability. */
}

declare global {
  interface Window {
    aiHost: AiHost
  }
}

const frame = document.getElementById('kit') as HTMLIFrameElement
const status = document.getElementById('status') as HTMLElement
const serve = new URLSearchParams(location.search).get('generate') !== '0'
let bytes = new ArrayBuffer(0)
let version = 1
let connection: KitConnection | null = null
let waiting: Array<() => void> = []

const host: AiHost = {
  answers: [],
  hold: false,
  release: () => {
    const now = waiting
    waiting = []
    for (const go of now) go()
  },
  generated: [],
  actions: [],
  agent: [],
  connection: () => connection,
  file: () => new TextDecoder().decode(new Uint8Array(bytes)),
  open: async (name, text) => {
    connection?.close()
    bytes = new TextEncoder().encode(text).buffer
    version = 1
    frame.src = `../index.html?${Date.now()}`
    connection = await mountKit(frame, {
      origin: location.origin,
      host: { name: 'AI test host', version: '1' },
      document: { path: name, name, bytes: bytes.slice(0), version: String(version) },
      theme: { colorScheme: 'light', tokens: { '--kit-primary': '#5645d4' } },
      locale: 'en-US',
      shortcuts: [],
      // Menus, so the AI menu's rows are menu items the test clicks.
      ui: { modes: { allowed: ['menus', 'line', 'classic', 'focus'], default: 'menus' } },
      handlers: {
        read: () => ({ bytes: bytes.slice(0), version: String(version), size: bytes.byteLength }),
        stamp: () => ({ version: String(version) }),
        write: ({ bytes: written }) => {
          bytes = written
          version += 1
          return { path: name, version: String(version) }
        },
        changed: () => {},
        openAgent: (params) => {
          host.agent.push(params?.prompt)
        },
        ...(serve
          ? {
              generate: async (params: KitGenerateParams) => {
                host.generated.push(params)
                if (host.hold) await new Promise<void>((go) => waiting.push(go))
                const next = host.answers.shift()
                if (!next) throw new KitRpcError(-32000, 'no model configured', { reason: 'unavailable' })
                if ('refuse' in next) throw new KitRpcError(-32000, `refused: ${next.refuse}`, { reason: next.refuse })
                return { text: next.text, model: 'test-model' }
              },
              actions: {
                'design.fromRecord': (params: Record<string, unknown>) => {
                  host.actions.push({ id: 'design.fromRecord', params })
                  return { done: true, message: 'Design started' }
                },
              },
            }
          : {}),
      },
    })
    status.textContent = `${name} open in ${connection.kit.id}`
  },
}
window.aiHost = host
status.textContent = 'ready'
