/**
 * A code block's label at its top right, as Obsidian's live preview draws it: the block's
 * language, which turns into a copy button under the pointer; a click copies the block's code.
 * A block without a language shows a copy icon instead. The engine reports the blocks in view
 * (`code.blocks`, document pixels).
 */
import { useEffect, useRef, useState } from 'react'
import { createKitStrings, type KitCanvasView } from '@kition/sdk/editor'
import { CopyIcon } from '@kition/tessera/icons/react'

type Strings = ReturnType<typeof createKitStrings>

/** Language names as Obsidian labels code blocks, by the fence's language id. */
const LANGUAGES: Record<string, string> = {
  js: 'JavaScript', javascript: 'JavaScript', jsx: 'JSX', mjs: 'JavaScript', ts: 'TypeScript', typescript: 'TypeScript', tsx: 'TSX',
  py: 'Python', python: 'Python', rs: 'Rust', rust: 'Rust', go: 'Go', golang: 'Go', java: 'Java', kt: 'Kotlin', kotlin: 'Kotlin',
  c: 'C', h: 'C', cpp: 'C++', 'c++': 'C++', cs: 'C#', csharp: 'C#', swift: 'Swift', rb: 'Ruby', ruby: 'Ruby', php: 'PHP',
  sh: 'Shell', shell: 'Shell', bash: 'Bash', zsh: 'Zsh', console: 'Console', ps1: 'PowerShell', powershell: 'PowerShell',
  json: 'JSON', yaml: 'YAML', yml: 'YAML', toml: 'TOML', ini: 'INI', xml: 'XML', html: 'HTML', css: 'CSS', scss: 'SCSS', less: 'Less',
  sql: 'SQL', md: 'Markdown', markdown: 'Markdown', diff: 'Diff', dart: 'Dart', scala: 'Scala', lua: 'Lua', r: 'R', dockerfile: 'Dockerfile',
}

/** A fence's language as its label: Obsidian's name for it, else the id as written. */
export function languageLabel(language: string) {
  return LANGUAGES[language.toLowerCase()] ?? language
}

export type CodeBlock = { top: number; bottom: number; right: number; left?: number; callout?: boolean; language: string; from: number; to: number }

export function CodeFlairs({ blocks, view, read, t }: { blocks: CodeBlock[]; view: KitCanvasView | null; read: (from: number, to: number) => Promise<string>; t: Strings }) {
  const [copied, setCopied] = useState<number | null>(null)
  // The pointer in document pixels: a callout's block shows its copy button only under it.
  const anchor = useRef<HTMLSpanElement>(null)
  const [pointer, setPointer] = useState<{ x: number; y: number } | null>(null)
  const inCallout = blocks.some((block) => block.callout)
  useEffect(() => {
    if (!inCallout || !view) return undefined
    const move = (event: PointerEvent) => {
      const surface = anchor.current?.parentElement?.getBoundingClientRect()
      if (!surface) return
      setPointer({ x: view.x + (event.clientX - surface.left) / view.zoom, y: view.y + (event.clientY - surface.top) / view.zoom })
    }
    const leave = () => setPointer(null)
    window.addEventListener('pointermove', move)
    document.documentElement.addEventListener('pointerleave', leave)
    return () => {
      window.removeEventListener('pointermove', move)
      document.documentElement.removeEventListener('pointerleave', leave)
    }
  }, [inCallout, view])
  if (!view) return null
  return (
    <>
      <span ref={anchor} hidden />
      {blocks.map((block) => {
        const left = (block.right - view.x) * view.zoom
        const top = (block.top - view.y) * view.zoom
        if (top + (block.bottom - block.top) * view.zoom < 0 || top > view.height) return null
        // A callout renders its code block as Obsidian's reading view does: no label, the copy
        // button only while the pointer is over the block.
        const hovered = pointer !== null && pointer.y >= block.top && pointer.y <= block.bottom && pointer.x >= (block.left ?? -Infinity) && pointer.x <= block.right
        if (block.callout && !hovered) return null
        const copy = async () => {
          try {
            await navigator.clipboard.writeText(await read(block.from, block.to))
            setCopied(block.from)
            window.setTimeout(() => setCopied((shown) => (shown === block.from ? null : shown)), 1500)
          } catch {
            setCopied(null)
          }
        }
        return (
          <button
            key={`${block.from}-${block.top}`}
            type="button"
            className="note-code-flair"
            data-testid="note-code-flair"
            data-language={block.language}
            style={{ left, top: top + 6 * view.zoom }}
            title={t('code.copy')}
            onMouseDown={(event) => {
              event.preventDefault()
              event.stopPropagation()
            }}
            onClick={() => void copy()}
          >
            {block.language && !block.callout ? (
              <>
                <span className="note-code-flair-language">{copied === block.from ? t('code.copied') : languageLabel(block.language)}</span>
                <span className="note-code-flair-copy">{copied === block.from ? t('code.copied') : t('code.copy')}</span>
              </>
            ) : copied === block.from ? (
              <span className="note-code-flair-language">{t('code.copied')}</span>
            ) : (
              // A block without a language shows a copy icon, as Obsidian does.
              <CopyIcon className="note-code-flair-icon" size={16} aria-hidden="true" />
            )}
          </button>
        )
      })}
    </>
  )
}
