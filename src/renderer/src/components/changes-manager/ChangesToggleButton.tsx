import { GitPullRequest } from 'lucide-react'

interface ChangesToggleButtonProps {
  onChangeCount: number
  hasConflicts: boolean
  onClick: () => void
}

export function ChangesToggleButton({ onChangeCount, hasConflicts, onClick }: ChangesToggleButtonProps) {
  if (onChangeCount === 0) {
    return null
  }

  return (
    <button
      type="button"
      onClick={onClick}
      className={`relative flex h-7 w-7 items-center justify-center rounded transition ${
        hasConflicts
          ? 'border border-amber-500/30 bg-amber-500/10 text-amber-400/80 hover:bg-amber-500/20'
          : 'border border-white/10 bg-white/[0.04] text-sentinel-mist hover:bg-white/[0.08] hover:text-white'
      }`}
      title="AI Changes Manager"
    >
      <GitPullRequest className="h-3.5 w-3.5" />
      {onChangeCount > 0 && (
        <span className={`absolute -right-1 -top-1 flex h-3.5 min-w-[14px] items-center justify-center rounded-full px-0.5 text-[9px] font-bold ${
          hasConflicts
            ? 'bg-amber-500/80 text-black'
            : 'bg-sentinel-accent/80 text-black'
        }`}>
          {onChangeCount > 99 ? '99+' : onChangeCount}
        </span>
      )}
    </button>
  )
}
