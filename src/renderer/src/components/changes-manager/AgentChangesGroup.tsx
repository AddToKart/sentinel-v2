import { ChevronDown, ChevronRight, FilePlus, FileMinus, FileEdit } from 'lucide-react'
import { useState } from 'react'
import type { AgentFileChange } from '@shared/types'

interface AgentChangesGroupProps {
  agentId: string
  agentLabel: string
  cliTool: string
  changes: AgentFileChange[]
  isExpanded: boolean
  onToggle: () => void
  onFileClick: (agentId: string, filePath: string) => void
  onPushAgent: (agentId: string) => void
  onDiscardAgent: (agentId: string) => void
}

function getOperationIcon(operation: string) {
  switch (operation) {
    case 'created':
      return <FilePlus className="h-3 w-3 text-emerald-200" />
    case 'deleted':
      return <FileMinus className="h-3 w-3 text-rose-300/90" />
    default:
      return <FileEdit className="h-3 w-3 text-sentinel-ice" />
  }
}

function getOperationLabel(operation: string) {
  switch (operation) {
    case 'created': return 'new'
    case 'deleted': return 'del'
    default: return 'mod'
  }
}

export function AgentChangesGroup({
  agentId,
  agentLabel,
  cliTool,
  changes,
  isExpanded,
  onToggle,
  onFileClick,
  onPushAgent,
  onDiscardAgent
}: AgentChangesGroupProps) {
  const [showActions, setShowActions] = useState(false)

  if (changes.length === 0) {
    return null
  }

  const totalAdditions = changes.reduce((sum, c) => sum + c.additions, 0)
  const totalDeletions = changes.reduce((sum, c) => sum + c.deletions, 0)

  return (
    <div className="mb-3">
      <button
        type="button"
        onClick={onToggle}
        onMouseEnter={() => setShowActions(true)}
        onMouseLeave={() => setShowActions(false)}
        className="flex w-full items-center gap-2 rounded px-2 py-1.5 transition hover:bg-white/[0.04]"
      >
        {isExpanded ? (
          <ChevronDown className="h-3.5 w-3.5 text-sentinel-mist/60" />
        ) : (
          <ChevronRight className="h-3.5 w-3.5 text-sentinel-mist/60" />
        )}
        <div className="flex flex-1 items-center gap-2">
          <span className="flex h-5 w-5 items-center justify-center rounded bg-sentinel-accent/15 text-[9px] font-bold uppercase tracking-widest text-sentinel-accent">
            {agentLabel.charAt(0)}
          </span>
          <div className="flex flex-1 flex-col items-start">
            <span className="text-xs font-medium text-white/80">{agentLabel}</span>
            <span className="text-[10px] text-sentinel-mist/60">{cliTool}</span>
          </div>
          <div className="flex items-center gap-1.5 text-[10px] font-mono">
            {totalAdditions > 0 && (
              <span className="text-emerald-200">+{totalAdditions}</span>
            )}
            {totalDeletions > 0 && (
              <span className="text-rose-300/90">-{totalDeletions}</span>
            )}
          </div>
        </div>
        {showActions && (
          <div className="flex items-center gap-1">
            <button
              type="button"
              onClick={(e) => { e.stopPropagation(); onPushAgent(agentId) }}
              className="rounded px-1.5 py-0.5 text-[10px] font-medium text-sentinel-accent transition hover:bg-sentinel-accent/15"
            >
              Push
            </button>
            <button
              type="button"
              onClick={(e) => { e.stopPropagation(); onDiscardAgent(agentId) }}
              className="rounded px-1.5 py-0.5 text-[10px] font-medium text-rose-300/90 transition hover:bg-rose-400/15"
            >
              Discard
            </button>
          </div>
        )}
      </button>

      {isExpanded && (
        <div className="ml-4 mt-1 border-l border-white/8 pl-2">
          {changes.map((change) => (
            <button
              key={change.id}
              type="button"
              onClick={() => onFileClick(agentId, change.filePath)}
              className="group flex w-full items-center gap-2 rounded px-2 py-1 text-left transition hover:bg-white/[0.04]"
            >
              {getOperationIcon(change.operation)}
              <span className="flex-1 truncate text-xs font-mono text-white/70 group-hover:text-white/90">
                {change.filePath}
              </span>
              <span className="text-[9px] uppercase tracking-widest text-sentinel-mist/50">
                {getOperationLabel(change.operation)}
              </span>
              <div className="flex items-center gap-1 text-[10px] font-mono">
                {change.additions > 0 && (
                  <span className="text-emerald-200">+{change.additions}</span>
                )}
                {change.deletions > 0 && (
                  <span className="text-rose-300/90">-{change.deletions}</span>
                )}
              </div>
            </button>
          ))}
        </div>
      )}
    </div>
  )
}
