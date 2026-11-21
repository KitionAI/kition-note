/**
 * Find, replace, and go to line over the engine (`text.find`, `text.findReplace`,
 * `selection.set`): Enter (and F3) selects the next match after the last one shown; Shift+Enter
 * (Shift+F3) the one before. As in Obsidian, the bar opens with the selected text to find, and
 * replace takes the shown match (Enter in its field) or every match (Mod+Alt+Enter). Matches are
 * Unicode character ranges, as the engine counts. Steps run one after another, and closing waits for
 * the one still running: the keyboard goes back to the note only once the match it asked for is
 * selected, so keys typed after Enter and Escape land after that match.
 */
import { useEffect, useRef, useState } from 'react'
import { Button, createKitStrings, toast } from '@kition/sdk/editor'
import { message, type NoteRun } from './useNote'

type Strings = ReturnType<typeof createKitStrings>
type Match = { from: number; to: number }

export type FindMode = 'find' | 'replace' | 'line'

export function FindBar({ mode, initial = '', run, t, onShown, onClose }: { mode: FindMode; initial?: string; run: NoteRun; t: Strings; onShown: () => void; onClose: () => void }) {
  const [query, setQuery] = useState(initial)
  const [replacement, setReplacement] = useState('')
  const [found, setFound] = useState<{ query: string; matches: Match[]; index: number } | null>(null)
  const input = useRef<HTMLInputElement>(null)
  // The step still running (Enter, F3, a replace), which the next one and closing wait for.
  const busy = useRef<Promise<void>>(Promise.resolve())
  const queue = (work: () => Promise<void>) => {
    const next = busy.current.then(work)
    busy.current = next.catch(() => {})
    return next
  }
  const close = () => {
    void busy.current.then(onClose)
  }
  useEffect(() => {
    input.current?.focus()
    input.current?.select()
  }, [mode])
  // Opened again over another selection: that text to find.
  useEffect(() => {
    if (initial) setQuery(initial)
  }, [initial])

  const show = async (match: Match) => {
    await run('selection.set', { anchor: match.from, focus: match.to })
    onShown()
  }

  const step = async (back: boolean) => {
    if (mode === 'line') {
      const line = Number.parseInt(query, 10)
      if (Number.isInteger(line) && line > 0) {
        await run('selection.set', { line, column: 1 })
        onShown()
        onClose()
      }
      return
    }
    if (!query) return
    let current = found
    if (!current || current.query !== query) {
      const answer = await run<{ matches: Match[] }>('text.find', { find: query, caseSensitive: false, limit: 1000 }).catch(() => ({ matches: [] }))
      current = { query, matches: answer.matches, index: -1 }
    }
    const count = current.matches.length
    if (!count) {
      setFound(current)
      return
    }
    const index = back ? (current.index - 1 + count) % count : (current.index + 1) % count
    const next = { ...current, index }
    setFound(next)
    const match = next.matches[index]
    if (match) await show(match)
  }

  /** Replaces the match shown (else the next one), then shows the next. */
  const replaceOne = async () => {
    if (!query) return
    const shown = found && found.query === query && found.index >= 0 ? found.matches[found.index] : null
    if (!shown) {
      await step(false)
      return
    }
    try {
      await run('selection.set', { anchor: shown.from, focus: shown.to })
      await run('selection.replace', { text: replacement })
      const answer = await run<{ matches: Match[] }>('text.find', { find: query, caseSensitive: false, limit: 1000 }).catch(() => ({ matches: [] }))
      const after = shown.from + [...replacement].length
      const index = answer.matches.findIndex((match) => match.from >= after)
      const next = { query, matches: answer.matches, index: index >= 0 ? index : 0 }
      setFound(next)
      const match = next.matches[next.index]
      if (match) await show(match)
      else onShown()
    } catch (error) {
      toast('error', t('commandFailed'), message(error))
    }
  }

  const replaceAll = async () => {
    if (!query) return
    try {
      const answer = await run<{ replaced?: number; count?: number }>('text.findReplace', { find: query, replace: replacement, occurrence: 'all', caseSensitive: false })
      toast('success', t('replaced', { count: answer.replaced ?? answer.count ?? 0 }))
      setFound(null)
    } catch (error) {
      toast('error', t('commandFailed'), message(error))
    }
  }

  const count = found && found.query === query ? found.matches.length : null
  return (
    <div className="note-find" role="search" data-testid="note-find" data-mode={mode}>
      <input
        ref={input}
        className="note-find-input"
        value={query}
        placeholder={mode === 'line' ? t('goToLinePlaceholder') : t('findPlaceholder')}
        inputMode={mode === 'line' ? 'numeric' : undefined}
        data-testid="note-find-input"
        onChange={(event) => setQuery(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === 'Enter' && (event.metaKey || event.ctrlKey) && event.altKey && mode === 'replace') {
            event.preventDefault()
            void queue(replaceAll)
          } else if (event.key === 'Enter' || event.key === 'F3') {
            event.preventDefault()
            const back = event.shiftKey
            void queue(() => step(back))
          } else if (event.key === 'Escape') {
            event.preventDefault()
            close()
          }
        }}
      />
      {mode === 'replace' ? (
        <input
          className="note-find-input"
          value={replacement}
          placeholder={t('replacePlaceholder')}
          data-testid="note-replace-input"
          onChange={(event) => setReplacement(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              event.preventDefault()
              if ((event.metaKey || event.ctrlKey) && event.altKey) void queue(replaceAll)
              else void queue(replaceOne)
            } else if (event.key === 'Escape') {
              event.preventDefault()
              close()
            }
          }}
        />
      ) : null}
      {count !== null && mode !== 'line' ? (
        <span className="kui-muted" data-testid="note-find-count">
          {count ? t('findCount', { index: (found?.index ?? 0) + 1, count }) : t('findNone')}
        </span>
      ) : null}
      {mode === 'replace' ? (
        <Button variant="ghost" data-testid="note-replace-one" onClick={() => void queue(replaceOne)}>
          {t('replaceOne')}
        </Button>
      ) : null}
      {mode === 'replace' ? (
        <Button variant="ghost" data-testid="note-replace-all" onClick={() => void queue(replaceAll)}>
          {t('replaceAll')}
        </Button>
      ) : null}
      <Button variant="ghost" data-testid="note-find-close" onClick={close}>
        {t('close')}
      </Button>
    </div>
  )
}
