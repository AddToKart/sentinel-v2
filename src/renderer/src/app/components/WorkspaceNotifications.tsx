import { useEffect, useRef, useState } from 'react'
import type { CSSProperties } from 'react'
import { createPortal } from 'react-dom'
import { Bell, CheckCheck, CheckCircle2, TriangleAlert, X } from 'lucide-react'

import type { WorkspaceNotification } from '../hooks/useWorkspaceNotifications'

interface WorkspaceNotificationsProps {
  bellRinging: boolean
  notifications: WorkspaceNotification[]
  previewNotification: WorkspaceNotification | null
  unreadCount: number
  onClearNotifications: () => void
  onClearPreviewNotification: () => void
  onDismissNotification: (notificationId: string) => void
  onMarkAllRead: () => void
}

function formatNotificationTime(timestamp: number): string {
  const deltaSeconds = Math.max(0, Math.round((Date.now() - timestamp) / 1000))
  if (deltaSeconds < 15) {
    return 'just now'
  }
  if (deltaSeconds < 60) {
    return `${deltaSeconds}s ago`
  }

  const minutes = Math.round(deltaSeconds / 60)
  if (minutes < 60) {
    return `${minutes}m ago`
  }

  const hours = Math.round(minutes / 60)
  if (hours < 24) {
    return `${hours}h ago`
  }

  const days = Math.round(hours / 24)
  return `${days}d ago`
}

export function WorkspaceNotifications({
  bellRinging,
  notifications,
  previewNotification,
  unreadCount,
  onClearNotifications,
  onClearPreviewNotification,
  onDismissNotification,
  onMarkAllRead
}: WorkspaceNotificationsProps): JSX.Element {
  const [open, setOpen] = useState(false)
  const [panelStyle, setPanelStyle] = useState<CSSProperties>({ top: 0, left: 0, width: 360 })

  const triggerRef = useRef<HTMLButtonElement | null>(null)

  useEffect(() => {
    if (!open) {
      return
    }

    function updatePosition(): void {
      const rect = triggerRef.current?.getBoundingClientRect()
      if (!rect) {
        return
      }

      const viewportPadding = 12
      const width = Math.min(360, window.innerWidth - viewportPadding * 2)
      const left = Math.min(
        Math.max(viewportPadding, rect.right - width),
        window.innerWidth - width - viewportPadding
      )

      setPanelStyle({
        top: rect.bottom + 10,
        left,
        width
      })
    }

    function handleEscape(event: KeyboardEvent): void {
      if (event.key === 'Escape') {
        setOpen(false)
      }
    }

    updatePosition()
    window.addEventListener('resize', updatePosition)
    window.addEventListener('scroll', updatePosition, true)
    window.addEventListener('keydown', handleEscape)

    return () => {
      window.removeEventListener('resize', updatePosition)
      window.removeEventListener('scroll', updatePosition, true)
      window.removeEventListener('keydown', handleEscape)
    }
  }, [open])

  return (
    <>
      <div style={{ WebkitAppRegion: 'no-drag' } as CSSProperties}>
        <button
          ref={triggerRef}
          className={`relative inline-flex h-8 w-8 items-center justify-center border border-white/10 bg-white/[0.04] text-sentinel-mist transition hover:border-white/20 hover:bg-white/[0.08] hover:text-white ${bellRinging ? 'workspace-bell-ring text-white' : ''}`}
          onClick={() => {
            setOpen((current) => {
              const next = !current
              if (next) {
                onMarkAllRead()
                onClearPreviewNotification()
              }
              return next
            })
          }}
          title="Workspace notifications"
          type="button"
        >
          <Bell className="h-4 w-4" />
          {unreadCount > 0 && (
            <span className="absolute -right-1 -top-1 inline-flex min-w-[18px] items-center justify-center border border-amber-200/30 bg-amber-300 px-1 text-[10px] font-semibold text-[#11161b]">
              {unreadCount > 9 ? '9+' : unreadCount}
            </span>
          )}
        </button>
      </div>

      {!open && previewNotification && typeof document !== 'undefined' && createPortal(
        <div
          className="workspace-notification-preview fixed z-[135] w-[320px] border border-white/10 bg-[#09131b]/96 shadow-[0_24px_60px_rgba(0,0,0,0.55)] backdrop-blur-2xl"
          style={{
            top: (triggerRef.current?.getBoundingClientRect().bottom ?? 0) + 12,
            left: Math.max(
              12,
              (triggerRef.current?.getBoundingClientRect().right ?? 332) - 320
            )
          }}
        >
          <div className="flex items-start gap-3 px-3 py-3">
            <div
              className={`mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center border ${
                previewNotification.tone === 'success'
                  ? 'border-emerald-400/25 bg-emerald-400/12 text-emerald-200'
                  : 'border-rose-400/25 bg-rose-400/12 text-rose-200'
              }`}
            >
              {previewNotification.tone === 'success'
                ? <CheckCircle2 className="h-4 w-4" />
                : <TriangleAlert className="h-4 w-4" />}
            </div>

            <div className="min-w-0 flex-1">
              <div className="text-[10px] font-semibold uppercase tracking-[0.22em] text-sentinel-mist">
                {previewNotification.workspaceName}
              </div>
              <div className="mt-1 text-sm font-semibold text-white">
                {previewNotification.title}
              </div>
              <div className="mt-1 text-[12px] leading-5 text-sentinel-mist/90">
                {previewNotification.preview}
              </div>
            </div>
          </div>
        </div>,
        document.body
      )}

      {open && typeof document !== 'undefined' && createPortal(
        <>
          <div
            className="fixed inset-0 z-[120]"
            onClick={() => setOpen(false)}
          />

          <div
            className="fixed z-[130] border border-white/10 bg-[#09131b]/98 shadow-[0_28px_90px_rgba(0,0,0,0.55)] backdrop-blur-2xl"
            style={{
              ...panelStyle,
              WebkitAppRegion: 'no-drag'
            } as CSSProperties}
          >
            <div className="flex items-center justify-between gap-3 border-b border-white/10 px-4 py-3">
              <div>
                <div className="text-[10px] font-semibold uppercase tracking-[0.24em] text-sentinel-mist">
                  Workspace Alerts
                </div>
                <div className="mt-1 text-sm text-white/90">
                  Activity from workspaces you are not currently viewing.
                </div>
              </div>

              <div className="flex items-center gap-2">
                <button
                  className="inline-flex h-8 items-center gap-1 border border-white/10 bg-white/[0.04] px-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-sentinel-mist transition hover:border-white/20 hover:bg-white/[0.08] hover:text-white"
                  onClick={onMarkAllRead}
                  type="button"
                >
                  <CheckCheck className="h-3.5 w-3.5" />
                  Read
                </button>
                <button
                  className="inline-flex h-8 w-8 items-center justify-center border border-white/10 bg-white/[0.04] text-sentinel-mist transition hover:border-white/20 hover:bg-white/[0.08] hover:text-white"
                  onClick={() => {
                    onClearPreviewNotification()
                    onClearNotifications()
                  }}
                  type="button"
                >
                  <X className="h-3.5 w-3.5" />
                </button>
              </div>
            </div>

            <div className="max-h-[420px] overflow-auto p-2">
              {notifications.length === 0 ? (
                <div className="border border-dashed border-white/10 bg-white/[0.02] px-4 py-8 text-center text-sm text-sentinel-mist">
                  No workspace notifications yet.
                </div>
              ) : (
                <div className="space-y-2">
                  {notifications.map((notification) => (
                    <div
                      key={notification.id}
                      className={`border px-3 py-3 ${
                        notification.unread
                          ? 'border-white/16 bg-white/[0.06]'
                          : 'border-white/8 bg-white/[0.03]'
                      }`}
                    >
                      <div className="flex items-start gap-3">
                        <div
                          className={`mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center border ${
                            notification.tone === 'success'
                              ? 'border-emerald-400/25 bg-emerald-400/12 text-emerald-200'
                              : 'border-rose-400/25 bg-rose-400/12 text-rose-200'
                          }`}
                        >
                          {notification.tone === 'success'
                            ? <CheckCircle2 className="h-4 w-4" />
                            : <TriangleAlert className="h-4 w-4" />}
                        </div>

                        <div className="min-w-0 flex-1">
                          <div className="flex items-center gap-2">
                            <div className="truncate text-sm font-semibold text-white">
                              {notification.title}
                            </div>
                            {notification.unread && (
                              <span className="inline-flex h-2 w-2 shrink-0 bg-amber-300" />
                            )}
                          </div>
                          <div className="mt-1 text-[11px] uppercase tracking-[0.18em] text-sentinel-mist">
                            {notification.workspaceName} · {formatNotificationTime(notification.createdAt)}
                          </div>
                          <div className="mt-2 text-[12px] leading-5 text-sentinel-mist/92">
                            {notification.preview}
                          </div>
                        </div>

                        <button
                          className="inline-flex h-8 w-8 shrink-0 items-center justify-center text-sentinel-mist transition hover:bg-white/10 hover:text-white"
                          onClick={() => onDismissNotification(notification.id)}
                          type="button"
                        >
                          <X className="h-4 w-4" />
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
        </>,
        document.body
      )}
    </>
  )
}
