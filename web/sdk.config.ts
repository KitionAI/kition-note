import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import type { Plugin } from 'vite'

/** This folder, the repository it is in, and the kition-sdk checkout beside it (or KITION_SDK). */
export const web = path.dirname(fileURLToPath(import.meta.url))
export const repo = path.resolve(web, '..')
export const sdk = path.resolve(process.env.KITION_SDK ?? path.join(repo, '..', 'kition-sdk'))
/** The kition-tessera checkout beside it (or KITION_TESSERA): kit-editor's shared components link it. */
export const tessera = path.resolve(process.env.KITION_TESSERA ?? path.join(repo, '..', 'kition-tessera'))

/**
 * kition-sdk's Kit Data Model, engine contract, and Kit UI packages, read in place by name when
 * KITION_LINK=1 and a kition-sdk checkout is there (local development); otherwise they come from npm.
 */
export const aliases = process.env.KITION_LINK !== '1' || !fs.existsSync(path.join(sdk, 'packages/kit-editor/package.json')) ? [] : [
  { find: /^@kition\/kdm$/, replacement: path.join(sdk, 'packages/kdm/src/index.ts') },
  { find: /^@kition\/kit-engine$/, replacement: path.join(sdk, 'packages/kit-engine/src/index.ts') },
  { find: /^@kition\/kit-editor$/, replacement: path.join(sdk, 'packages/kit-editor/src/index.ts') },
  { find: /^@kition\/kit-editor\/styles\.css$/, replacement: path.join(sdk, 'packages/kit-editor/src/styles.css') },
  { find: /^@kition\/kit-protocol$/, replacement: path.join(sdk, 'packages/kit-ui-protocol/src/index.ts') },
  { find: /^@kition\/kit-host$/, replacement: path.join(sdk, 'packages/kit-host/src/index.ts') },
  { find: /^@kition\/kit-dev-host$/, replacement: path.join(sdk, 'packages/kit-dev-host/src/index.ts') },
]

/**
 * kition-sdk's sources take their npm dependencies from here: a bare import from a file there
 * resolves as if it were made here, so the page has one React, one CodeMirror, and one Loro, and
 * the build never needs the kition-sdk checkout's node_modules.
 */
export function dependenciesFromHere(): Plugin {
  const here = path.join(web, 'src', 'main.tsx')
  const inside = (file: string) => file.startsWith(web + path.sep) && !file.includes(`${path.sep}node_modules${path.sep}`)
  return {
    name: 'kit-note-dependencies-from-here',
    enforce: 'pre',
    async resolveId(source, importer, options) {
      if (!importer || inside(importer) || importer.includes(`${path.sep}node_modules${path.sep}`)) return null
      if (/^[./\0]/.test(source) || source.startsWith('@kition/') || source.startsWith('node:') || path.isAbsolute(source)) return null
      return this.resolve(source, here, { ...options, skipSelf: true })
    },
  }
}
