import type { KitEngineInfo, KitModuleManifest } from '@kition/sdk/editor'

/** The commands that write only comment threads (kit.json `collaboration.comments.commands`): a commenter runs them. */
export const COMMENT_COMMANDS = ['comment.add', 'comment.reply', 'comment.resolve', 'comment.move', 'comment.delete'] as const

/**
 * The engine (`bindings/wasm`, WebAssembly built by scripts/build-engine.sh), from this package's
 * own folder. It draws the note itself, so it gets the faces it draws with before the note opens
 * (Kition platform E9; copied from kition-fonts by build-engine.sh): Inter for prose, JetBrains Mono
 * for code, Droid Sans Fallback for Chinese and Japanese, Noto Color Emoji for emoji in color.
 */
export function engineManifest(): KitModuleManifest {
  return {
    kitId: 'kit.note',
    base: new URL('./engine/', document.baseURI).href,
    module: 'kition_note_kit',
    documentClass: 'NoteDocument',
    fonts: ['Inter-Variable.ttf', 'JetBrainsMono-Variable.ttf', 'DroidSansFallbackFull.ttf', 'Noto-COLRv1.ttf'].map((file) => new URL(`./fonts/${file}`, document.baseURI).href),
    commentCommands: COMMENT_COMMANDS,
  }
}

/** What the shell reads from the engine's `inspect()`. */
export type NoteInfo = { characters: number; words: number; lines: number; openThreads: number; readOnly: boolean; heading: string | null }

export function describe(info: KitEngineInfo): NoteInfo {
  const inspected = info.inspected as Partial<NoteInfo>
  return {
    characters: inspected.characters ?? 0,
    words: inspected.words ?? 0,
    lines: inspected.lines ?? 0,
    openThreads: inspected.openThreads ?? 0,
    readOnly: inspected.readOnly === true,
    heading: inspected.heading ?? null,
  }
}
