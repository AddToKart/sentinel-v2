import { X, ChevronDown, ChevronRight } from 'lucide-react'
import { useState } from 'react'
import type { AgentFileChange, UnifiedSandboxEntry } from '@shared/types'

interface FileDiffViewerProps {
  agentId: string
  filePath: string
  change: AgentFileChange | undefined
  onClose: () => void
}

export function FileDiffViewer({ filePath, change, onClose }: FileDiffViewerProps) {
  const [expanded, setExpanded] = useState(true)

  if (!change) {
    return (
      <div className="rounded border border-white/10 bg-white/[0.03] p-3">
        <div className="flex items-center justify-between">
          <span className="text-xs font-mono text-sentinel-mist/60">{filePath}</span>
          <button type="button" onClick={onClose} className="text-sentinel-mist/60 hover:text-white">
            <X className="h-3.5 w-3.5" />
          </button>
        </div>
        <p className="mt-2 text-xs text-sentinel-mist/50">No diff content available for this file.</p>
      </div>
    )
  }

  return (
    <div className="rounded border border-white/10 bg-white/[0.03]">
      <button
        type="button"
        onClick={() => setExpanded(!expanded)}
        className="flex w-full items-center justify-between rounded-t px-3 py-2 transition hover:bg-white/[0.04]"
      >
        <div className="flex items-center gap-2">
          {expanded ? (
            <ChevronDown className="h-3.5 w-3.5 text-sentinel-mist/60" />
          ) : (
            <ChevronRight className="h-3.5 w-3.5 text-sentinel-mist/60" />
          )}
          <span className="text-xs font-mono text-white/80">{filePath}</span>
        </div>
        <div className="flex items-center gap-2">
          <span className="text-[10px] font-mono text-emerald-200">+{change.additions}</span>
          <span className="text-[10px] font-mono text-rose-300/90">-{change.deletions}</span>
          <button type="button" onClick={(e) => { e.stopPropagation(); onClose() }} className="text-sentinel-mist/60 hover:text-white">
            <X className="h-3.5 w-3.5" />
          </button>
        </div>
      </button>

      {expanded && change.diffContent && (
        <div className="border-t border-white/8 bg-[#04070b] p-3 font-mono text-[11px] leading-relaxed max-h-64 overflow-y-auto">
          {change.diffContent.split('\n').map((line, i) => {
            let lineClass = 'text-white/50'
            if (line.startsWith('+') && !line.startsWith('+++')) lineClass = 'text-emerald-200'
            else if (line.startsWith('-') && !line.startsWith('---')) lineClass = 'text-rose-300/90'
            else if (line.startsWith('@@')) lineClass = 'text-sentinel-ice'
            return (
              <div key={i} className={`${lineClass} whitespace-pre`}>
                {line}
              </div>
            )
          })}
        </div>
      )}

      {expanded && !change.diffContent && (
        <div className="border-t border-white/8 p-3">
          <p className="text-xs text-sentinel-mist/50">
            {change.isBinary ? 'Binary file - diff not available' : 'No diff content available'}
          </p>
        </div>
      )}
    </div>
  )
}

interface ConflictResolverProps {
  entry: UnifiedSandboxEntry
  changesByAgent: Record<string, AgentFileChange[]>
  onResolve: (filePath: string, winningAgentId: string) => void
}

export function ConflictResolver({ entry, changesByAgent, onResolve }: ConflictResolverProps) {
  const conflictingAgents = entry.conflictAgentIds || []
  const allAgents = [entry.sourceAgentId, ...conflictingAgents]

  return (
    <div className="rounded border border-amber-500/30 bg-amber-500/5 p-3">
      <div className="mb-2 flex items-center gap-2">
        <span className="text-xs font-medium text-amber-400/80">⚠ Conflict</span>
        <span className="text-xs font-mono text-white/70">{entry.filePath}</span>
      </div>
      <p className="mb-3 text-[11px] text-sentinel-mist/60">
        Multiple agents modified this file. Choose which version to keep:
      </p>
      <div className="flex flex-wrap gap-2">
        {allAgents.map((agentId) => {
          const agentChanges = changesByAgent[agentId] || []
          const change = agentChanges.find(c => c.filePath === entry.filePath)
          return (
            <button
              key={agentId}
              type="button"
              onClick={() => onResolve(entry.filePath, agentId)}
              className="flex items-center gap-1.5 rounded border border-white/10 bg-white/[0.04] px-2.5 py-1.5 text-xs text-white/80 transition hover:bg-white/[0.08] hover:text-white"
            >
              <span className="flex h-4 w-4 items-center justify-center rounded bg-sentinel-accent/15 text-[8px] font-bold uppercase text-sentinel-accent">
                {agentId.charAt(0)}
              </span>
              <span>{agentId}</span>
              {change && (
                <span className="text-[10px] font-mono text-sentinel-mist/50">
                  +{change.additions}/-{change.deletions}
                </span>
              )}
            </button>
          )
        })}
      </div>
    </div>
  )
}
