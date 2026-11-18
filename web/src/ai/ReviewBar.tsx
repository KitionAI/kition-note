/**
 * The AI review bar (Kition's `DocumentAgentReviewBar` and `DocumentAgentSelectionToolbar`, rebuilt
 * on `host/generate`): a bar over the note while a suggestion is written and shown. The note does
 * not change until Accept, which replaces the reviewed range in one engine command; Reject drops
 * the suggestion and Try again asks once more. The selection bar offers the rewrites for a
 * selection when the host runs models.
 */
import { CheckIcon as Check, ExpandIcon as Expand, MessageSquareIcon as MessageSquare, Minimize2Icon as Minimize2, RotateCcwIcon as RotateCcw, SparklesIcon as Sparkles, WandSparklesIcon as WandSparkles, XIcon as X } from '@kition/tessera-icons/react'
import { Button, createKitStrings } from '@kition/kit-editor'
import { wordDiff, type AssistKind } from './assist'

type Strings = ReturnType<typeof createKitStrings>

/** A suggestion under review: the range it replaces (UTF-16, as CodeMirror counts) and the text there when it was asked. */
export type Review = {
  kind: AssistKind
  from: number
  to: number
  original: string
  status: 'running' | 'ready' | 'failed'
  suggestion?: string
  model?: string
  reason?: string
  message?: string
}

const KIND_LABEL: Record<AssistKind, string> = { improve: 'aiImprove', shorten: 'aiShorten', expand: 'aiExpand' }

/** The words of a refusal reason (`host/generate`'s `error.data.reason`). */
function refusal(t: Strings, reason: string | undefined) {
  const key = `aiRefused.${reason ?? 'failed'}`
  const text = t(key)
  return text === key ? t('aiRefused.failed') : text
}

export function ReviewBar({ review, t, onAccept, onReject, onRetry }: { review: Review; t: Strings; onAccept: () => void; onReject: () => void; onRetry: () => void }) {
  const parts = review.status === 'ready' && review.suggestion !== undefined ? wordDiff(review.original, review.suggestion) : []
  return (
    <div className="note-ai-review" role="region" aria-label={t('aiReview')} data-testid="note-ai-review" data-status={review.status}>
      <div className="note-ai-review-head">
        <span className="note-ai-review-icon" aria-hidden="true">
          <Sparkles size={15} />
        </span>
        <strong>{t('aiReview')}</strong>
        <span className="kui-muted">{t(KIND_LABEL[review.kind])}</span>
        <span className="note-ai-review-spacer" />
        {review.status === 'running' ? (
          <>
            <span className="kui-muted" data-testid="note-ai-running">
              {t('aiWriting')}
            </span>
            <Button variant="ghost" onClick={onReject} data-testid="note-ai-cancel">
              <X size={15} />
              {t('aiCancel')}
            </Button>
          </>
        ) : review.status === 'failed' ? (
          <>
            <Button variant="ghost" onClick={onRetry} data-testid="note-ai-retry">
              <RotateCcw size={15} />
              {t('aiRetry')}
            </Button>
            <Button variant="ghost" onClick={onReject} data-testid="note-ai-dismiss">
              <X size={15} />
              {t('aiDismiss')}
            </Button>
          </>
        ) : (
          <>
            <Button variant="ghost" onClick={onRetry} data-testid="note-ai-retry">
              <RotateCcw size={15} />
              {t('aiRetry')}
            </Button>
            <Button variant="ghost" onClick={onReject} data-testid="note-ai-reject">
              <X size={15} />
              {t('aiReject')}
            </Button>
            <Button variant="primary" onClick={onAccept} data-testid="note-ai-accept">
              <Check size={15} />
              {t('aiAccept')}
            </Button>
          </>
        )}
      </div>
      {review.status === 'failed' ? (
        <div className="note-ai-review-body note-ai-review-error" data-testid="note-ai-error" data-reason={review.reason}>
          {refusal(t, review.reason)}
        </div>
      ) : review.status === 'ready' ? (
        <div className="note-ai-review-body" data-testid="note-ai-diff">
          {parts.map((part, index) =>
            part.kind === 'same' ? (
              <span key={index}>{part.text}</span>
            ) : part.kind === 'removed' ? (
              <del key={index} className="note-ai-removed">
                {part.text}
              </del>
            ) : (
              <ins key={index} className="note-ai-added">
                {part.text}
              </ins>
            ),
          )}
        </div>
      ) : null}
    </div>
  )
}

/**
 * The selection's actions, floating over the bottom of the note: the rewrites when the host runs
 * models (`onAssist`), and Comment when the host keeps comment threads and the person may start one
 * (`onComment`).
 */
export function SelectionBar({ t, onAssist, onComment }: { t: Strings; onAssist?: (kind: AssistKind) => void; onComment?: () => void }) {
  const button = (kind: AssistKind, Icon: typeof Sparkles) => (
    <Button
      variant="ghost"
      // Keeps the editor's selection, which the suggestion replaces.
      onMouseDown={(event) => event.preventDefault()}
      onClick={() => onAssist?.(kind)}
      data-testid={`note-ai-${kind}`}
    >
      <Icon size={15} />
      {t(KIND_LABEL[kind])}
    </Button>
  )
  return (
    <div className="note-ai-selection" role="toolbar" aria-label={t(onAssist ? 'aiSelection' : 'commentAddTitle')} data-testid="note-ai-selection">
      {onAssist ? (
        <>
          {button('improve', WandSparkles)}
          {button('shorten', Minimize2)}
          {button('expand', Expand)}
        </>
      ) : null}
      {onComment ? (
        <Button variant="ghost" title={t('commentAddTitle')} onMouseDown={(event) => event.preventDefault()} onClick={onComment} data-testid="note-comment-add">
          <MessageSquare size={15} />
          {t('commentAdd')}
        </Button>
      ) : null}
    </div>
  )
}
