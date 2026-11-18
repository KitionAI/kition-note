/**
 * Opening what a note links (KitLinks 1.0 section 5, Kit UI protocol section 7i): a wikilink or a
 * Markdown link to another file asks the host to open it (`host/open`, capability `open-file`),
 * with the heading or block it names as the opened note's view; a web address goes to the host
 * (`host/openUrl`, capability `open-url`); a link to a heading of this note shows it here. The kit
 * opens no window itself (UI-K8): without the host's capability, a click on such a link does
 * nothing. Requests go only from the person's click or key (OP1, OP6).
 *
 * Link targets are sent as the note stores them (HC-P2): a path relative to the note's folder (or
 * from the workspace root with a leading `/`), or a bare name (`[[Plan]]`), which the host
 * resolves (LR4).
 */
import { openErrorReason, type HostConnection, type KitLinkRef, type KitOpenParams, type KitView } from '@kition/kit-editor'

/** A link's target as `host/open` takes it (Kit UI protocol section 7i): a path or a bare name. */
export type LinkRef = Pick<KitLinkRef, 'path' | 'name'>

const WEB = /^(https?:|mailto:)/i
const SCHEME = /^[a-z][a-z\d+.-]*:/i

export const canOpenFiles = (host: HostConnection) => host.init.capabilities.includes('open-file')
export const canOpenUrls = (host: HostConnection) => host.init.capabilities.includes('open-url')

/** Whether the host refused an open because the target does not exist (`not-found`). */
export function isNotFound(error: unknown) {
  return openErrorReason(error) === 'not-found'
}

function decode(value: string) {
  try {
    return decodeURIComponent(value)
  } catch {
    return value
  }
}

/** A wikilink target as a link ref: a path when it names a folder or an extension, a bare name otherwise. */
export function wikilinkRef(target: string): LinkRef {
  const clean = target.trim()
  return clean.includes('/') || /\.[a-z0-9]{1,8}$/i.test(clean) ? { path: clean } : { name: clean }
}

/** The view a wikilink's `#heading` or `#^block` part names in its target. */
export function wikilinkView(link: { heading?: string | null; blockId?: string | null }): KitView | undefined {
  if (link.blockId) return { blockId: link.blockId }
  if (link.heading) return { heading: link.heading.split('#').map((part) => part.trim()).filter(Boolean) }
  return undefined
}

/** Asks the host to open `link`; `create` makes a missing note first (OP4). Answers whether it opened. */
export async function openLink(host: HostConnection, link: LinkRef, options: { view?: KitView; beside?: boolean; create?: boolean } = {}) {
  const params: KitOpenParams = { link, ...(options.view ? { view: options.view } : {}), placement: options.beside ? 'beside' : 'tab', ...(options.create ? { create: { kit: 'kit.note' } } : {}) }
  const answer = await host.peer.request('host/open', params)
  return answer?.opened !== false
}

/** Asks the host to open a web address (OP7: `https:`, `http:`, `mailto:` only). */
export async function openUrl(host: HostConnection, url: string) {
  const answer = await host.peer.request('host/openUrl', { url })
  return answer?.opened !== false
}

/** What a Markdown link's `href` is: a web address, a heading of this note, or another file (with its section). */
export type MarkdownTarget = { kind: 'web'; url: string } | { kind: 'here'; section: string } | { kind: 'file'; path: string; section?: string } | { kind: 'other' }

export function markdownTarget(href: string): MarkdownTarget {
  let value = href.trim()
  if (value.startsWith('<') && value.endsWith('>')) value = value.slice(1, -1).trim()
  if (WEB.test(value)) return { kind: 'web', url: value }
  if (!value || value.startsWith('//') || SCHEME.test(value)) return { kind: 'other' }
  const hash = value.indexOf('#')
  const query = value.indexOf('?')
  const end = [hash, query].filter((index) => index >= 0).reduce((least, index) => Math.min(least, index), value.length)
  const path = decode(value.slice(0, end))
  const section = hash >= 0 ? decode(value.slice(hash + 1)) : ''
  if (!path) return section ? { kind: 'here', section } : { kind: 'other' }
  return { kind: 'file', path, ...(section ? { section } : {}) }
}
