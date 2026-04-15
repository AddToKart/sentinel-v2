import { ArrowUpFromLine, Trash2, GitPullRequest } from 'lucide-react'
import type { UnifiedSandboxEntry } from '@shared/types'

interface UnifiedSandboxSummaryProps {
  entries: UnifiedSandboxEntry[]
  conflictCount: number
  pendingPushCount: number
  onPushAll: () => void
  onDiscardAll: () => void
  isLoading: boolean
}

export function UnifiedSandboxSummary({
  entries,
  conflictCount,
  pendingPushCount,
  onPushAll,
  onDiscardAll,
  isLoading
}: UnifiedSandboxSummaryProps) {
  if (entries.length === 0) {
    return null
  }

  return (
    <div className="border-t border-white/10 bg-white/[0.02] px-3 py-2.5">
      <div className="mb-2 flex items-center gap-2">
        <GitPullRequest className="h-3.5 w-3.5 text-sentinel-accent" />
        <span className="text-[11px] font-bold uppercase tracking-[0.24em] text-sentinel-mist/70">
          Unified Sandbox
        </span>
        <span className="ml-auto text-xs font-mono text-white/50">
          {entries.length} file{entries.length !== 1 ? 's' : ''} changed
        </span>
      </div>

      {conflictCount > 0 && (
        <div className="mb-2 rounded border border-amber-500/30 bg-amber-500/5 px-2 py-1.5">
          <span className="text-[11px] text-amber-400/80">
            ⚠ {conflictCount} unresolved conflict{conflictCount !== 1 ? 's' : ''} — must be resolved before push
          </span>
        </div>
      )}

      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={onPushAll}
          disabled={isLoading || conflictCount > 0 || pendingPushCount === 0}
          className={`flex items-center gap-1.5 rounded px-2.5 py-1.5 text-xs font-medium transition ${
            isLoading || conflictCount > 0 || pendingPushCount === 0
              ? 'cursor-not-allowed bg-white/[0.03] text-white/30'
              : 'border border-sentinel-accent/25 bg-sentinel-accent/10 text-sentinel-accent hover:bg-sentinel-accent/20'
          }`}
        >
          <ArrowUpFromLine className="h-3.5 w-3.5" />
          Push All
        </button>
        <button
          type="button"
          onClick={onDiscardAll}
          disabled={isLoading}
          className={`flex items-center gap-1.5 rounded px-2.5 py-1.5 text-xs font-medium transition ${
            isLoading
              ? 'cursor-not-allowed bg-white/[0.03] text-white/30'
              : 'border border-white/10 bg-white/[0.04] text-sentinel-mist hover:bg-white/[0.08] hover:text-white'
          }`}
        >
          <Trash2 className="h-3.5 w-3.5" />
          Discard All
        </button>
      </div>
    </div>
  )
}
