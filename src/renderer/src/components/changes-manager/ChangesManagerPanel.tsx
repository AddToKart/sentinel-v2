import { X, Loader2 } from 'lucide-react'
import { useState, useMemo } from 'react'
import type { AgentFileChange, UnifiedSandboxEntry } from '@shared/types'
import { AgentChangesGroup } from './AgentChangesGroup'
import { FileDiffViewer, ConflictResolver } from './FileDiffViewer'
import { UnifiedSandboxSummary } from './UnifiedSandboxSummary'

interface ChangesManagerPanelProps {
  isOpen: boolean
  isLoading: boolean
  agentChanges: Record<string, AgentFileChange[]>
  agentLabels: Record<string, string>
  agentCliTools: Record<string, string>
  unifiedEntries: UnifiedSandboxEntry[]
  conflictCount: number
  pendingPushCount: number
  selectedFile: { agentId: string; filePath: string } | null
  onClose: () => void
  onFileClick: (agentId: string, filePath: string) => void
  onPushAll: () => Promise<void>
  onPushAgent: (agentId: string) => Promise<void>
  onDiscardAll: () => Promise<void>
  onDiscardAgent: (agentId: string) => Promise<void>
  onResolveConflict: (filePath: string, winningAgentId: string) => Promise<void>
}

export function ChangesManagerPanel({
  isOpen,
  isLoading,
  agentChanges,
  agentLabels,
  agentCliTools,
  unifiedEntries,
  conflictCount,
  pendingPushCount,
  selectedFile,
  onClose,
  onFileClick,
  onPushAll,
  onPushAgent,
  onDiscardAll,
  onDiscardAgent,
  onResolveConflict
}: ChangesManagerPanelProps) {
  const [expandedAgents, setExpandedAgents] = useState<Record<string, boolean>>({})

  const agentIds = useMemo(() => Object.keys(agentChanges).sort(), [agentChanges])

  const changesByAgent = useMemo(() => {
    const result: Record<string, AgentFileChange[]> = {}
    for (const agentId of agentIds) {
      result[agentId] = agentChanges[agentId] || []
    }
    return result
  }, [agentChanges, agentIds])

  const conflicts = useMemo(() => {
    return unifiedEntries.filter(e => e.status === 'conflicted')
  }, [unifiedEntries])

  const selectedChange = useMemo(() => {
    if (!selectedFile) return undefined
    return changesByAgent[selectedFile.agentId]?.find(c => c.filePath === selectedFile.filePath)
  }, [selectedFile, changesByAgent])

  const toggleAgent = (agentId: string) => {
    setExpandedAgents(prev => ({ ...prev, [agentId]: !prev[agentId] }))
  }

  if (!isOpen) return null

  return (
    <div className="fixed inset-y-0 right-0 z-50 flex">
      <div
        className="fixed inset-0 bg-black/40 backdrop-blur-sm"
        onClick={onClose}
      />
      <div className="relative flex h-full w-80 flex-col border-l border-white/10 bg-[#081018] shadow-2xl">
        {/* Header */}
        <div className="flex items-center justify-between border-b border-white/10 px-3 py-2.5">
          <div className="flex items-center gap-2">
            <span className="text-[11px] font-bold uppercase tracking-[0.24em] text-sentinel-mist/70">
              AI Changes
            </span>
            {pendingPushCount > 0 && (
              <span className="flex h-4 min-w-[16px] items-center justify-center rounded-full bg-sentinel-accent/15 px-1 text-[9px] font-bold text-sentinel-accent">
                {pendingPushCount}
              </span>
            )}
          </div>
          <div className="flex items-center gap-1">
            {isLoading && <Loader2 className="h-3.5 w-3.5 animate-spin text-sentinel-accent" />}
            <button
              type="button"
              onClick={onClose}
              className="flex h-6 w-6 items-center justify-center rounded text-sentinel-mist/60 transition hover:bg-white/[0.08] hover:text-white"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          </div>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto px-3 py-2">
          {agentIds.length === 0 && !isLoading && (
            <div className="flex h-full items-center justify-center">
              <p className="text-xs text-sentinel-mist/50">No AI changes detected</p>
            </div>
          )}

          {agentIds.map(agentId => (
            <AgentChangesGroup
              key={agentId}
              agentId={agentId}
              agentLabel={agentLabels[agentId] || agentId}
              cliTool={agentCliTools[agentId] || 'unknown'}
              changes={changesByAgent[agentId] || []}
              isExpanded={!!expandedAgents[agentId]}
              onToggle={() => toggleAgent(agentId)}
              onFileClick={onFileClick}
              onPushAgent={onPushAgent}
              onDiscardAgent={onDiscardAgent}
            />
          ))}

          {/* Conflicts section */}
          {conflicts.length > 0 && (
            <div className="mt-4">
              <div className="mb-2 flex items-center gap-2">
                <span className="text-[11px] font-bold uppercase tracking-[0.24em] text-amber-400/80">
                  Conflicts
                </span>
                <span className="flex h-4 min-w-[16px] items-center justify-center rounded-full bg-amber-500/15 px-1 text-[9px] font-bold text-amber-400/80">
                  {conflicts.length}
                </span>
              </div>
              {conflicts.map(conflict => (
                <ConflictResolver
                  key={conflict.id}
                  entry={conflict}
                  changesByAgent={changesByAgent}
                  onResolve={onResolveConflict}
                />
              ))}
            </div>
          )}

          {/* Selected file diff */}
          {selectedFile && (
            <div className="mt-4">
              <div className="mb-2">
                <span className="text-[11px] font-bold uppercase tracking-[0.24em] text-sentinel-mist/70">
                  Diff Viewer
                </span>
              </div>
              <FileDiffViewer
                agentId={selectedFile.agentId}
                filePath={selectedFile.filePath}
                change={selectedChange}
                onClose={() => onFileClick('', '')}
              />
            </div>
          )}
        </div>

        {/* Footer */}
        <UnifiedSandboxSummary
          entries={unifiedEntries}
          conflictCount={conflictCount}
          pendingPushCount={pendingPushCount}
          onPushAll={onPushAll}
          onDiscardAll={onDiscardAll}
          isLoading={isLoading}
        />
      </div>
    </div>
  )
}
