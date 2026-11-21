/**
 * Export and print (Kition's `DocumentExportDialog` rebuilt on the engine): the engine lays the
 * note out and writes the HTML or PDF (`exportHtml`, `exportPdf` extras); the dialog says, before
 * the person exports, what the pages cannot draw yet (`page.layout`'s `unsupported`). Files go
 * through the host only: a new file it creates (KitFiles `files/create`) when it serves files,
 * else the host's export write; printing hands the PDF to the host's print dialog (`files/print`).
 */
import { useEffect, useState } from 'react'
import { Button, Dialog, toast, createKitStrings, type HostConnection } from '@kition/sdk/editor'
import type { Session } from './useNote'
import { message } from './useNote'

type Strings = ReturnType<typeof createKitStrings>

export type ExportKind = 'html' | 'pdf' | 'print'
type PageSize = 'letter' | 'a4'

type Unsupported = { missingGlyphs: number; missingSample: string; images: number; embeds: number; math: number; mermaid: number; html: number }
type LayoutAnswer = { pages: number; unsupported: Unsupported }

/** The note's name with another extension. */
function exportName(name: string, extension: string) {
  const base = name.split('/').pop() ?? name
  const dot = base.lastIndexOf('.')
  return `${dot > 0 ? base.slice(0, dot) : base}.${extension}`
}

/** Whether the host prints (KitFiles `print`). */
export const canPrint = (host: HostConnection) => host.kitFiles.canPrint

/** What the pages leave out, as sentences for the dialog. */
function gaps(t: Strings, unsupported: Unsupported): string[] {
  const out: string[] = []
  if (unsupported.missingGlyphs) out.push(t('exportMissingGlyphs', { count: unsupported.missingGlyphs, sample: unsupported.missingSample }))
  if (unsupported.images + unsupported.embeds) out.push(t('exportImages', { count: unsupported.images + unsupported.embeds }))
  if (unsupported.math) out.push(t('exportMath', { count: unsupported.math }))
  if (unsupported.mermaid) out.push(t('exportMermaid', { count: unsupported.mermaid }))
  if (unsupported.html) out.push(t('exportHtmlBlocks', { count: unsupported.html }))
  return out
}

/** A new file beside the note, of type `mime` (`files/create`'s `mime`). */
async function writeExport(host: HostConnection, name: string, mime: string, bytes: Uint8Array) {
  if (host.kitFiles.served) {
    const created = await host.kitFiles.create(name, { mime })
    await host.kitFiles.write(created.handle, bytes)
    return created.name
  }
  return host.exportFile(name, bytes)
}

export function ExportDialog({ kind, host, session, t, onClose }: { kind: ExportKind; host: HostConnection; session: Session; t: Strings; onClose: () => void }) {
  const [size, setSize] = useState<PageSize>('letter')
  const [layout, setLayout] = useState<LayoutAnswer | null>(null)
  const [busy, setBusy] = useState(false)
  const doc = session.doc
  const name = host.init.document.name

  useEffect(() => {
    if (!doc || kind === 'html') return undefined
    let current = true
    void doc
      .execute('page.layout', { pageSize: size, page: 1 })
      .then((answer) => {
        if (current) setLayout(JSON.parse(answer) as LayoutAnswer)
      })
      .catch(() => {
        if (current) setLayout(null)
      })
    return () => {
      current = false
    }
  }, [doc, kind, size])

  const run = async () => {
    if (!doc) return
    setBusy(true)
    try {
      if (kind === 'html') {
        const html = await doc.invoke<string>('exportHtml', true)
        const written = await writeExport(host, exportName(name, 'html'), 'text/html', new TextEncoder().encode(html))
        toast('success', t('exported', { name: written }))
      } else {
        const pdf = await doc.invoke<Uint8Array>('exportPdf', size)
        if (kind === 'print') {
          await host.kitFiles.print({ bytes: pdf, title: name })
        } else {
          const written = await writeExport(host, exportName(name, 'pdf'), 'application/pdf', pdf)
          toast('success', t('exported', { name: written }))
        }
      }
      onClose()
    } catch (error) {
      toast('error', t('exportFailed'), message(error))
    } finally {
      setBusy(false)
    }
  }

  const title = kind === 'html' ? t('exportHtmlTitle') : kind === 'print' ? t('printTitle') : t('exportPdfTitle')
  const notes = layout ? gaps(t, layout.unsupported) : []
  return (
    <Dialog
      title={title}
      onClose={onClose}
      uiId={`control:${kind === 'html' ? 'export-html' : kind === 'print' ? 'print' : 'export-pdf'}`}
      testId="note-export-dialog"
      actions={
        <>
          <Button variant="ghost" onClick={onClose} data-testid="note-export-cancel">
            {t('cancel')}
          </Button>
          <Button variant="primary" disabled={busy || !doc} onClick={() => void run()} data-testid="note-export-run">
            {kind === 'print' ? t('print') : t('export')}
          </Button>
        </>
      }
    >
      {kind === 'html' ? (
        <p className="note-export-text">{t('exportHtmlBody')}</p>
      ) : (
        <>
          <div className="note-export-sizes" role="radiogroup" aria-label={t('pageSize')}>
            {(['letter', 'a4'] as const).map((option) => (
              <Button key={option} variant={size === option ? 'primary' : 'ghost'} role="radio" aria-checked={size === option} onClick={() => setSize(option)} data-testid={`note-export-size-${option}`}>
                {t(option === 'letter' ? 'pageLetter' : 'pageA4')}
              </Button>
            ))}
          </div>
          {layout ? (
            <p className="note-export-text" data-testid="note-export-pages">
              {t('exportPages', { count: layout.pages })}
            </p>
          ) : null}
          {notes.length ? (
            <ul className="note-export-gaps" data-testid="note-export-gaps">
              {notes.map((note) => (
                <li key={note}>{note}</li>
              ))}
            </ul>
          ) : null}
        </>
      )}
    </Dialog>
  )
}
