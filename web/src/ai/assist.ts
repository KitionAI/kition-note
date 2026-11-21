/**
 * The note's AI suggestions (Kit UI protocol 2.4 section 7j): the kit builds the prompt from the
 * note's own text, asks the host to run the person's model (`host/generate`), and shows the answer
 * for review. Nothing changes until the person accepts it; accepting is one engine command
 * (`text.replace` with `expected`, so a stale suggestion is refused), one undo step. The host
 * never gives the kit a key (UI-G2), and the kit never generates in the background (UI-G1).
 */
import type { KitGenerateParams, KitGeneratePurpose, KitGenerateResult } from '@kition/sdk/protocol'
import { hostRequestErrorReason } from '@kition/sdk/protocol'
import type { HostConnection } from '@kition/sdk/editor'

export type AssistKind = 'improve' | 'shorten' | 'expand'

/**
 * What the note generates for (`purpose`, Kit UI protocol 2.4 section 7j): improve, shorten, and
 * expand are the defined `rewrite` purpose (UI-G4), which every host that lists `generate` serves.
 */
export const NOTE_GENERATE_PURPOSE = 'rewrite' satisfies KitGeneratePurpose

/** The host action the note offers (capability `action:design.fromRecord`). */
export const DESIGN_ACTION = 'design.fromRecord'

const INSTRUCTIONS: Record<AssistKind, string> = {
  improve: 'Improve the writing of the text: clearer and more fluent, with spelling and grammar fixed. Keep its meaning and length about the same.',
  shorten: 'Shorten the text to about half its length, keeping its key points.',
  expand: 'Expand the text with more detail and explanation, keeping its tone. At most about twice its length.',
}

const SYSTEM = [
  'You rewrite a part of a Markdown note.',
  'Answer with the replacement Markdown only: no explanation, no quotes around it, no code fence around it.',
  'Write in the language of the text.',
  'Keep its Markdown syntax: links, wikilinks ([[...]]), embeds, tags (#tag), footnotes, block ids (^id), list markers and indentation.',
].join(' ')

/** How much of the note around the selection goes with the request, as context only. */
const CONTEXT_CHARS = 1200
/** The longest selection the note sends (UI-G2: only what the note holds, and not more than needed). */
export const MAX_SELECTION_CHARS = 20000

export type AssistRequest = { kind: AssistKind; text: string; before: string; after: string; title: string }

export function assistMessages({ kind, text, before, after, title }: AssistRequest): KitGenerateParams['messages'] {
  const context = [title ? `Note: ${title}` : '', before ? `Text before it:\n${before.slice(-CONTEXT_CHARS)}` : '', after ? `Text after it:\n${after.slice(0, CONTEXT_CHARS)}` : ''].filter(Boolean).join('\n\n')
  const user = `${INSTRUCTIONS[kind]}\n\n${context ? `${context}\n\n` : ''}The text to rewrite:\n${text}`
  return [
    { role: 'system', parts: [{ type: 'text', text: SYSTEM }] },
    { role: 'user', parts: [{ type: 'text', text: user }] },
  ]
}

/**
 * The model's answer as replacement text: a fence or quotes the model put around the whole answer
 * come off, and the selection's own leading and trailing line breaks stay.
 */
export function cleanSuggestion(answer: string, original: string): string {
  let text = answer.replace(/\r\n?/g, '\n').trim()
  const fence = text.match(/^```[\w-]*\n([\s\S]*?)\n```$/)
  if (fence) text = fence[1]
  else if (text.length > 1 && /^["“].*["”]$/s.test(text) && !/^["“]/.test(original.trim())) text = text.slice(1, -1)
  const lead = original.match(/^\s*/)?.[0] ?? ''
  const trail = original.match(/\s*$/)?.[0] ?? ''
  return lead + text + trail
}

export type GenerateOutcome = { ok: true; text: string; model?: string } | { ok: false; reason: string; message: string }

/** Asks the host to run the model; a refusal answers its reason (`unavailable`, `not-allowed`, `quota`, …). */
export async function generateSuggestion(host: HostConnection, request: AssistRequest): Promise<GenerateOutcome> {
  const params: KitGenerateParams = { purpose: NOTE_GENERATE_PURPOSE, output: 'text', messages: assistMessages(request), options: { temperature: 0.4 } }
  try {
    const answer = (await host.peer.request('host/generate', params)) as KitGenerateResult
    if (typeof answer?.text !== 'string' || !answer.text.trim()) return { ok: false, reason: 'failed', message: 'The model answered no text.' }
    return { ok: true, text: cleanSuggestion(answer.text, request.text), ...(answer.model ? { model: answer.model } : {}) }
  } catch (error) {
    return { ok: false, reason: hostRequestErrorReason(error) ?? 'failed', message: error instanceof Error ? error.message : String(error) }
  }
}

/** Whether the host runs models for kits (capability `generate`) and the host action `id`. */
export const canGenerate = (host: HostConnection) => host.init.capabilities.includes('generate')
export const canAct = (host: HostConnection, id: string) => host.init.capabilities.includes(`action:${id}`)

/** One part of a word diff between the original and the suggestion. */
export type DiffPart = { kind: 'same' | 'removed' | 'added'; text: string }

/** A word-level diff (each word with the whitespace after it) for the review preview; long texts fall back to removed + added. */
export function wordDiff(before: string, after: string): DiffPart[] {
  const split = (text: string) => text.match(/\S+\s*|\s+/g) ?? []
  const a = split(before)
  const b = split(after)
  if (a.length * b.length > 4_000_000) return [{ kind: 'removed', text: before }, { kind: 'added', text: after }]
  const rows = a.length + 1
  const cols = b.length + 1
  const lcs = new Uint32Array(rows * cols)
  for (let i = a.length - 1; i >= 0; i -= 1) {
    for (let j = b.length - 1; j >= 0; j -= 1) {
      lcs[i * cols + j] = a[i] === b[j] ? lcs[(i + 1) * cols + j + 1] + 1 : Math.max(lcs[(i + 1) * cols + j], lcs[i * cols + j + 1])
    }
  }
  const parts: DiffPart[] = []
  const push = (kind: DiffPart['kind'], text: string) => {
    const last = parts.at(-1)
    if (last?.kind === kind) last.text += text
    else parts.push({ kind, text })
  }
  let i = 0
  let j = 0
  while (i < a.length && j < b.length) {
    if (a[i] === b[j]) {
      push('same', a[i])
      i += 1
      j += 1
    } else if (lcs[(i + 1) * cols + j] >= lcs[i * cols + j + 1]) push('removed', a[i++])
    else push('added', b[j++])
  }
  while (i < a.length) push('removed', a[i++])
  while (j < b.length) push('added', b[j++])
  return parts
}
