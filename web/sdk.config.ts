import path from 'node:path'
import { fileURLToPath } from 'node:url'
import type { Plugin } from 'vite'

/** This folder, the repository it is in, and the kition-sdk checkout beside it (or KITION_SDK). */
export const web = path.dirname(fileURLToPath(import.meta.url))
export const repo = path.resolve(web, '..')
export const sdk = path.resolve(process.env.KITION_SDK ?? path.join(repo, '..', 'kition-sdk'))
/** The kition-tessera checkout beside it (or KITION_TESSERA), which KITION_LINK=1 links as @kition/tessera. */
export const tessera = path.resolve(process.env.KITION_TESSERA ?? path.join(repo, '..', 'kition-tessera'))

/**
 * With KITION_LINK=1 (.pnpmfile.cjs), @kition/sdk and @kition/tessera are the checkouts' built
 * packages, outside node_modules. Their bare imports (React, Tessera) resolve as if they were made
 * here, so the page has one React and one Tessera, and the build never needs the
 * checkouts' node_modules.
 */
export function dependenciesFromHere(): Plugin {
  const here = path.join(web, 'src', 'main.tsx')
  const inside = (file: string) => file.startsWith(web + path.sep) && !file.includes(`${path.sep}node_modules${path.sep}`)
  return {
    name: 'kit-note-dependencies-from-here',
    enforce: 'pre',
    async resolveId(source, importer, options) {
      if (!importer || inside(importer) || importer.includes(`${path.sep}node_modules${path.sep}`)) return null
      if (/^[./\0]/.test(source) || source.startsWith('@kition/sdk/') || source.startsWith('node:') || path.isAbsolute(source)) return null
      return this.resolve(source, here, { ...options, skipSelf: true })
    },
  }
}
