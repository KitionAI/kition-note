/**
 * The open note's session: kit-editor's `useKitSession` over the engine, which holds the note and
 * draws it. What the note adds is how a change of its file on disk comes in:
 *
 * - A stored replica opens whatever the file's stamp (`replicaMergesFile`): the engine merges
 *   what the file holds that the replica does not (`openWithReplica`, design 2.5), so a note
 *   changed on disk while it was closed keeps its comments and history.
 * - A change on disk while the note has no unsaved edits merges into the open note
 *   (`followFile`), keeping its undo history and the caret; with unsaved edits kit-editor raises
 *   `conflict` and the person chooses (`mergeFromDisk`, `keepMine`).
 */
import { useCallback, useEffect, useMemo, useRef } from 'react'
import { toast, useKitSession, createKitStrings, type HostConnection, type KitEngine, type KitSessionOptions } from '@kition/sdk/editor'
import type { describe } from './engine'

export type Session = ReturnType<typeof useKitSession<ReturnType<typeof describe>>>
type Strings = ReturnType<typeof createKitStrings>

export const message = (error: unknown) => (error instanceof Error ? error.message : String(error))

/** The session options, wired to the open note once it is open. */
export class NoteSessionBridge {
  followFile: ((engine: KitEngine) => Promise<boolean>) | null = null
  readonly options: KitSessionOptions = {
    replicaMergesFile: true,
    followFile: (engine) => (this.followFile ? this.followFile(engine) : false),
  }
}

export function useNote(session: Session, host: HostConnection, bridge: NoteSessionBridge, t: Strings) {
  const latest = useRef(session)
  latest.current = session
  const doc = session.doc

  /** Runs an engine command and answers its parsed result. */
  const run = useCallback(async <R = unknown>(command: string, params: Record<string, unknown> = {}): Promise<R> => {
    const answer = await latest.current.run(command, params)
    try {
      return JSON.parse(String(answer)) as R
    } catch {
      return answer as R
    }
  }, [])

  /**
   * Merges the file on disk into the note, keeping every edit made here (`mergeFile`, design 2.5).
   * A note that held no unsaved edits holds the file afterwards, and is saved.
   */
  const mergeFile = useCallback(
    async (engine: KitEngine) => {
      const { bytes } = await host.files.read(host.init.document.path)
      await engine.invoke<string>('mergeFile', new Uint8Array(bytes))
      // An engine call is no change to kit-editor: a command through the session refreshes the editor
      // (the canvas repaints the merged note).
      if (latest.current.doc === engine) await latest.current.run('selection.get', {}).catch(() => undefined)
    },
    [host],
  )

  // The file changed on disk (Kit UI protocol `document/fileChanged`, standard H6). Without
  // unsaved edits the note takes the change in where it is; before the note is open, kit-editor
  // reopens it instead.
  useEffect(() => {
    if (!doc) return undefined
    const follow = async (engine: KitEngine) => {
      if (engine !== doc) return false
      await mergeFile(engine)
      return true
    }
    bridge.followFile = follow
    return () => {
      if (bridge.followFile === follow) bridge.followFile = null
    }
  }, [bridge, doc, mergeFile])

  // With unsaved edits kit-editor raises `conflict`, and the person chooses: merge the file's changes
  // into the note, keeping every unsaved edit, or keep the note as it is, which the next save
  // writes over the file.
  const mergeFromDisk = useCallback(async () => {
    if (!doc) return
    try {
      await mergeFile(doc)
      latest.current.keepMine()
    } catch (error) {
      toast('error', t('diskMergeFailed'), message(error))
    }
  }, [doc, mergeFile, t])
  const keepMine = useCallback(() => latest.current.keepMine(), [])

  return useMemo(() => ({ run, diskChanged: session.conflict && doc !== null, mergeFromDisk, keepMine }), [doc, keepMine, mergeFromDisk, run, session.conflict])
}

export type NoteRun = ReturnType<typeof useNote>['run']
