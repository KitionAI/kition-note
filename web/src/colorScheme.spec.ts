import { describe, expect, it } from 'vitest'
import { colorSchemeOf } from './colorScheme'

const root = (colorScheme?: string) => ({ dataset: colorScheme === undefined ? {} : { colorScheme } }) as unknown as HTMLElement

describe('colorSchemeOf', () => {
  it('reads the scheme kit-editor marks the root with', () => {
    expect(colorSchemeOf(root('dark'))).toBe('dark')
    expect(colorSchemeOf(root('light'))).toBe('light')
  })

  it('is light when the host names none or another', () => {
    expect(colorSchemeOf(root())).toBe('light')
    expect(colorSchemeOf(root('sepia'))).toBe('light')
  })
})
