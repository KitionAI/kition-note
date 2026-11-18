/**
 * Pictures pasted by their web address: HTML copied from a web page carries its pictures as
 * `<img src="https://…">`, often signed addresses that expire within minutes, which the paste turns
 * into `![alt](https://…)`. Each such picture is kept as a workspace file (through the host, Kit
 * UI protocol section 7l) and the note links the kept file in place of the address, so the
 * picture outlives the address.
 */

/** `![alt](address "title")` with an `https:` or `http:` address, as `pasteHtml` writes it (parentheses escaped). */
const REMOTE_IMAGE = /!\[((?:\\.|[^\]\\])*)\]\((https?:\/\/(?:\\.|[^\s()\\])+)(\s+"(?:\\.|[^"\\])*")?\)/gi

/** An address as written in Markdown, with its escaped parentheses as they are. */
const unescaped = (address: string) => address.replace(/\\([()])/g, '$1')

/**
 * `markdown` with each picture given by a web address kept by `keep` (called once per distinct
 * address, with a 1-based index, answering the link target of the kept file) and linked there. A
 * picture `keep` refuses stays linked by its address and is listed in `failed`.
 */
export async function keepRemoteImages(markdown: string, keep: (url: string, index: number) => Promise<string>) {
  const kept = new Map<string, string | null>()
  const failed: Array<{ url: string; error: unknown }> = []
  for (const match of markdown.matchAll(REMOTE_IMAGE)) {
    const url = unescaped(match[2] ?? '')
    if (kept.has(url)) continue
    try {
      kept.set(url, await keep(url, kept.size + 1))
    } catch (error) {
      kept.set(url, null)
      failed.push({ url, error })
    }
  }
  if (!kept.size) return { text: markdown, failed }
  const text = markdown.replace(REMOTE_IMAGE, (whole, alt: string, address: string, title: string | undefined) => {
    const target = kept.get(unescaped(address))
    return target ? `![${alt}](${target}${title ?? ''})` : whole
  })
  return { text, failed }
}
