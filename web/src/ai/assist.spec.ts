import { describe, expect, it } from 'vitest'
import { assistMessages, cleanSuggestion, wordDiff } from './assist'

describe('cleanSuggestion', () => {
  it('takes off a fence or quotes around the whole answer and keeps the selection\'s own edges', () => {
    expect(cleanSuggestion('```markdown\nNew text.\n```', 'Old text.\n')).toBe('New text.\n')
    expect(cleanSuggestion('“New text.”', ' Old text.')).toBe(' New text.')
    expect(cleanSuggestion('"Quoted" stays', '"Quoted" was')).toBe('"Quoted" stays')
    expect(cleanSuggestion('a\r\nb', 'x')).toBe('a\nb')
  })
})

describe('wordDiff', () => {
  it('marks removed and added words, keeping the same ones', () => {
    expect(wordDiff('Goals for the quarter.', 'Our goals for this quarter.')).toEqual([
      { kind: 'removed', text: 'Goals ' },
      { kind: 'added', text: 'Our goals ' },
      { kind: 'same', text: 'for ' },
      { kind: 'removed', text: 'the ' },
      { kind: 'added', text: 'this ' },
      { kind: 'same', text: 'quarter.' },
    ])
  })
})

describe('assistMessages', () => {
  it('sends the instruction, the context, and the text, with a system rule to answer Markdown only', () => {
    const [system, user] = assistMessages({ kind: 'shorten', text: 'Long text.', before: 'Before.', after: 'After.', title: 'Plan.md' })
    expect(system.role).toBe('system')
    const body = user.parts.map((part) => ('text' in part ? part.text : '')).join('')
    expect(body).toContain('Shorten')
    expect(body).toContain('Long text.')
    expect(body).toContain('Before.')
    expect(body).toContain('Note: Plan.md')
  })
})
