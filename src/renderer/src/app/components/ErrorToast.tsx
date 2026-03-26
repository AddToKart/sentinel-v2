import { AlertTriangle, X } from 'lucide-react'
import { useEffect } from 'react'

interface ErrorToastProps {
  message: string | null
  onDismiss: () => void
}

export function ErrorToast({ message, onDismiss }: ErrorToastProps): JSX.Element | null {
  useEffect(() => {
    if (!message) {
      return
    }

    const timer = window.setTimeout(() => {
      onDismiss()
    }, 6500)

    return () => {
      window.clearTimeout(timer)
    }
  }, [message, onDismiss])

  if (!message) {
    return null
  }

  return (
    <div className="pointer-events-none fixed right-4 top-4 z-[190] flex w-[min(430px,calc(100vw-2rem))] justify-end">
      <div
        className="workspace-notification-preview pointer-events-auto w-full border border-rose-400/25 bg-[#160b10]/96 shadow-[0_24px_60px_rgba(0,0,0,0.55)] backdrop-blur-2xl"
        role="alert"
      >
        <div className="flex items-start gap-3 px-4 py-3">
          <div className="mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center border border-rose-400/20 bg-rose-400/10 text-rose-200">
            <AlertTriangle className="h-4 w-4" />
          </div>

          <div className="min-w-0 flex-1">
            <div className="text-[10px] font-semibold uppercase tracking-[0.24em] text-rose-200/80">
              Sentinel Alert
            </div>
            <div className="mt-1 text-sm leading-6 text-rose-50">
              {message}
            </div>
          </div>

          <button
            className="inline-flex h-8 w-8 shrink-0 items-center justify-center border border-white/10 text-sentinel-mist transition hover:border-white/20 hover:text-white"
            onClick={onDismiss}
            type="button"
          >
            <X className="h-4 w-4" />
          </button>
        </div>
      </div>
    </div>
  )
}
