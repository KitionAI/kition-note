/**
 * The note's comments (rule M23): threads beside the text in the replica, never in the `.md`. The
 * threads are the engine's (`comment.threads`), drawn by kit-editor's `CommentsPanel`; a new thread
 * anchors to the engine's selection (`selection.get`), or to the whole note without one.
 */
import { useEffect, useMemo, useState } from 'react'
import { Button, CommentsPanel, toast, createKitStrings, type CommentsPanelProps, type HostConnection } from '@kition/kit-editor'
import type { CommentAnchor, CommentThread } from '@kition/kdm'
import { message, type NoteRun, type Session } from '../useNote'

type Strings = ReturnType<typeof createKitStrings>

/** kit-editor's comment panel labels, from the shell's strings. */
const COMMENT_LABELS: Record<keyof NonNullable<CommentsPanelProps['labels']>, string> = {
  title: 'comments.title',
  empty: 'comments.empty',
  newComment: 'comments.new',
  placeholder: 'comments.placeholder',
  replyPlaceholder: 'comments.replyPlaceholder',
  comment: 'comments.comment',
  reply: 'comments.reply',
  cancel: 'comments.cancel',
  resolve: 'comments.resolve',
  reopen: 'comments.reopen',
  delete: 'comments.delete',
  resolved: 'comments.resolved',
  readOnlyHistory: 'comments.history',
  you: 'comments.you',
}

/** The note's root object and its text field, as comment anchors name them (`model/schema.json`). */
const NOTE_ROOT_ID = 'note'
const NOTE_BODY = 'body'

/** Who comments here and how (Kit UI protocol `document.comments`); absent: unknown, as `readOnly` allows. */
function commentAccess(host: HostConnection) {
  const comments = host.init.document.comments
  return {
    me: comments?.me ?? null,
    name: comments?.name,
    canComment: comments ? comments.canComment : host.readOnly !== true,
    othersReadOnly: comments?.othersReadOnly ?? false,
  }
}

type EngineThread = { id: string; resolved: boolean; quote?: string; from?: number; to?: number; comments: Array<{ id: string; author: string; name?: string; body: string; created: string }> }

export function NoteComments({ host, session, run, t }: { host: HostConnection; session: Session; run: NoteRun; t: Strings }) {
  const access = useMemo(() => commentAccess(host), [host])
  const [pending, setPending] = useState<CommentAnchor | null>(null)
  const [threads, setThreads] = useState<EngineThread[]>([])
  // A comment may change no text, so the panel asks again after its own commands too.
  const [asked, askAgain] = useState(0)
  const doc = session.doc
  const revision = session.change.revision
  useEffect(() => {
    if (!doc) return undefined
    let current = true
    // Read straight from the engine: a read through the session would count as a change and ask again.
    void doc
      .execute('comment.threads', {})
      .then((answer) => {
        if (current) setThreads((JSON.parse(answer) as { threads?: EngineThread[] }).threads ?? [])
      })
      .catch(() => {
        if (current) setThreads([])
      })
    return () => {
      current = false
    }
  }, [doc, revision, asked])
  const shown = useMemo<CommentThread[]>(
    () =>
      threads.map((thread) => ({
        id: thread.id,
        resolved: thread.resolved,
        anchor: { object: NOTE_ROOT_ID, field: NOTE_BODY, ...(thread.quote !== undefined ? { quote: thread.quote } : {}), ...(thread.from !== undefined && thread.to !== undefined ? { range: { start: thread.from, end: thread.to } } : {}) },
        comments: thread.comments.map((comment) => ({ id: comment.id, author: comment.author, ...(comment.name ? { name: comment.name } : {}), body: comment.body, created: comment.created })),
      })) as unknown as CommentThread[],
    [threads],
  )
  const labels = useMemo(() => Object.fromEntries(Object.entries(COMMENT_LABELS).map(([key, id]) => [key, t(id)])), [t])
  const who = { ...(access.me ? { author: access.me } : {}), ...(access.name ? { name: access.name } : {}), created: new Date().toISOString() }
  const act = (command: string, params: Record<string, unknown>) => {
    void run(command, params)
      .then(() => askAgain((value) => value + 1))
      .catch((error: unknown) => toast('error', t('commandFailed'), message(error)))
  }

  /** A new thread at the engine's selection (Unicode characters), or on the whole note. */
  const startThread = async () => {
    const anchor: CommentAnchor = { object: NOTE_ROOT_ID, field: NOTE_BODY }
    const selection = await run<{ from: number; to: number; text: string }>('selection.get', {}).catch(() => null)
    if (selection && selection.to > selection.from) setPending({ ...anchor, range: { start: selection.from, end: selection.to }, quote: selection.text })
    else setPending(anchor)
  }

  return (
    <aside className="kui-panel note-kit-panel note-kit-comments" data-testid="note-comments">
      {access.canComment ? (
        <div className="note-kit-comments-header">
          <Button variant="ghost" data-testid="note-comments-new" onMouseDown={(event) => event.preventDefault()} onClick={() => void startThread()}>
            {t('comments.new')}
          </Button>
        </div>
      ) : null}
      <CommentsPanel
        threads={shown}
        me={access.me}
        canComment={access.canComment}
        othersReadOnly={access.othersReadOnly}
        pendingAnchor={pending}
        labels={labels}
        testId="note-comment-panel"
        anchorLabel={(anchor) => (anchor.quote ? `“${anchor.quote}”` : null)}
        onAdd={(anchor, body) => {
          setPending(null)
          act('comment.add', { ...(anchor.range ? { from: anchor.range.start, to: anchor.range.end } : {}), body, ...who })
        }}
        onCancelNew={() => setPending(null)}
        onReply={(thread, body) => act('comment.reply', { thread, body, ...who })}
        onResolve={(thread, resolved) => act('comment.resolve', { thread, resolved })}
        onDelete={(thread, comment) => act('comment.delete', { thread, comment })}
      />
    </aside>
  )
}
