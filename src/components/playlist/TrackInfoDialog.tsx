import { useEffect, useState } from 'react'
import type { Track, TrackMetadata } from '@/types'
import { useCachedThumbnail } from '@/hooks/use-cached-thumbnail'
import { api } from '@/lib/api'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'

interface TrackInfoDialogProps {
  track: Track | null
  open: boolean
  onOpenChange: (open: boolean) => void
}

function formatDuration(seconds: number | null): string {
  if (seconds === null) return '—'
  const mins = Math.floor(seconds / 60)
  const secs = seconds % 60
  return `${mins}:${secs.toString().padStart(2, '0')}`
}

function formatFileSize(bytes: number | string | null): string {
  if (bytes === null) return '—'
  const value = typeof bytes === 'string' ? Number(bytes) : bytes
  if (!Number.isFinite(value) || value <= 0) return '—'
  if (value < 1024) return `${value} B`
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`
  return `${(value / (1024 * 1024)).toFixed(1)} MB`
}

function formatAttributeValue(value: unknown): string {
  if (value === null || value === undefined) return '—'
  if (typeof value === 'boolean') return value ? 'true' : 'false'
  if (typeof value === 'string' || typeof value === 'number') {
    return String(value)
  }
  return JSON.stringify(value)
}

function MetadataRow({
  label,
  value,
}: {
  label: string
  value: React.ReactNode
}) {
  return (
    <div className="grid grid-cols-[7rem_1fr] gap-2 text-sm">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="min-w-0 wrap-break-word text-foreground">{value}</dd>
    </div>
  )
}

export function TrackInfoDialog({
  track,
  open,
  onOpenChange,
}: TrackInfoDialogProps) {
  const [metadata, setMetadata] = useState<TrackMetadata | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    /* eslint-disable react-hooks/set-state-in-effect -- This effect owns the async metadata load lifecycle for the open track. */
    if (!open || !track) {
      setMetadata(null)
      setError(null)
      return
    }

    let cancelled = false
    setLoading(true)
    setError(null)
    api
      .trackMetadata(track.id)
      .then((result) => {
        if (!cancelled) setMetadata(result)
      })
      .catch((err) => {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : 'Failed to load metadata')
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false)
      })

    return () => {
      cancelled = true
    }
    /* eslint-enable react-hooks/set-state-in-effect */
  }, [open, track])

  const thumbnail = useCachedThumbnail(
    track?.id ?? 0,
    { enabled: open && Boolean(track) },
  )

  const title = metadata?.track.title ?? track?.title ?? 'Unknown Title'
  const performer
    = metadata?.track.performer ?? track?.performer ?? 'Unknown Artist'

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className="flex max-h-[min(85vh,720px)] flex-col overflow-hidden sm:max-w-lg"
      >
        <DialogHeader className="shrink-0 pr-8">
          <DialogTitle>Track info</DialogTitle>
          <DialogDescription>
            Metadata from your library and Telegram document attributes.
          </DialogDescription>
        </DialogHeader>

        {track && (
          <div className="min-h-0 flex-1 overflow-y-auto">
            <div className="flex flex-col gap-5 pr-1">
              <div className="flex items-start gap-4">
                <div className="size-24 shrink-0 overflow-hidden rounded-lg border border-border bg-muted">
                  {thumbnail.url
                    ? (
                        <img
                          src={thumbnail.url}
                          alt=""
                          className="size-full object-cover"
                        />
                      )
                    : (
                        <div className="flex size-full items-center justify-center text-xs text-muted-foreground">
                          No cover
                        </div>
                      )}
                </div>
                <div className="min-w-0 pt-1">
                  <p className="truncate text-base font-medium text-foreground">
                    {title}
                  </p>
                  <p className="truncate text-sm text-muted-foreground">
                    {performer}
                  </p>
                  <p className="mt-2 font-mono text-xs text-muted-foreground">
                    {formatDuration(metadata?.track.duration ?? track.duration)}
                  </p>
                </div>
              </div>

              {loading && (
                <p className="text-sm text-muted-foreground">
                  Loading metadata…
                </p>
              )}

              {error && <p className="text-sm text-destructive">{error}</p>}

              {metadata && (
                <>
                  <section className="space-y-2">
                    <h3 className="text-sm font-medium text-foreground">Track</h3>
                    <dl className="space-y-2 rounded-lg border border-border bg-card/50 p-3">
                      <MetadataRow
                        label="Title"
                        value={metadata.track.title ?? '—'}
                      />
                      <MetadataRow
                        label="Artist"
                        value={metadata.track.performer ?? '—'}
                      />
                      <MetadataRow
                        label="Duration"
                        value={formatDuration(metadata.track.duration)}
                      />
                      <MetadataRow
                        label="MIME type"
                        value={metadata.track.mimeType ?? '—'}
                      />
                      <MetadataRow
                        label="File size"
                        value={formatFileSize(metadata.track.fileSize)}
                      />
                      <MetadataRow label="Source" value={metadata.track.source} />
                      <MetadataRow label="File ID" value={metadata.track.fileId} />
                      <MetadataRow
                        label="Unique ID"
                        value={metadata.track.fileUniqueId}
                      />
                      <MetadataRow
                        label="Added"
                        value={metadata.track.createdAt}
                      />
                    </dl>
                  </section>

                  <section className="space-y-2">
                    <h3 className="text-sm font-medium text-foreground">
                      Document
                    </h3>
                    <dl className="space-y-2 rounded-lg border border-border bg-card/50 p-3">
                      <MetadataRow
                        label="Document ID"
                        value={metadata.document.id}
                      />
                      <MetadataRow label="DC ID" value={metadata.document.dcId} />
                      <MetadataRow
                        label="MIME type"
                        value={metadata.document.mimeType ?? '—'}
                      />
                      <MetadataRow
                        label="Size"
                        value={formatFileSize(metadata.document.size)}
                      />
                      <MetadataRow
                        label="Thumbnail"
                        value={
                          metadata.document.hasRemoteThumb
                            ? 'Remote thumb on Telegram'
                            : 'Embedded / none'
                        }
                      />
                    </dl>
                  </section>

                  <section className="space-y-2">
                    <h3 className="text-sm font-medium text-foreground">
                      Attributes
                    </h3>

                    {metadata.document.attributes.length === 0
                      ? (
                          <p className="rounded-lg border border-border bg-card/50 p-3 text-sm text-muted-foreground">
                            No document attributes available. Re-sync your library to
                            cache them on future imports.
                          </p>
                        )
                      : (
                          <div className="space-y-3">
                            {metadata.document.attributes.map((attribute, index) => {
                              const entries = Object.entries(attribute).filter(
                                ([key]) => key !== 'type',
                              )

                              return (
                                <div
                                  key={`${String(attribute.type)}-${index}`}
                                  className="rounded-lg border border-border bg-card/50 p-3"
                                >
                                  <p className="mb-2 text-sm font-medium text-foreground">
                                    {String(attribute.type)}
                                  </p>
                                  <dl className="space-y-1.5">
                                    {entries.map(([key, value]) => (
                                      <div
                                        key={key}
                                        className="grid grid-cols-[6.5rem_1fr] gap-2 text-sm"
                                      >
                                        <dt className="text-muted-foreground">
                                          {key}
                                        </dt>
                                        <dd className="min-w-0 break-all font-mono text-xs text-foreground">
                                          {formatAttributeValue(value)}
                                        </dd>
                                      </div>
                                    ))}
                                  </dl>
                                </div>
                              )
                            })}
                          </div>
                        )}
                  </section>
                </>
              )}
            </div>
          </div>
        )}
      </DialogContent>
    </Dialog>
  )
}
