import { Clock3, Cpu, GitBranch, Layers3, MemoryStick, TerminalSquare } from 'lucide-react'

import type { SessionWorkspaceStrategy, TabSummary, WorkspaceSummary } from '@shared/types'

interface StatusBarProps {
  summary: WorkspaceSummary
  consoleOpen: boolean
  defaultSessionStrategy: SessionWorkspaceStrategy
  onToggleConsole: () => void
  focusedTab?: TabSummary | null
}

function formatTime(timestamp: number): string {
  return new Date(timestamp).toLocaleTimeString([], {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit'
  })
}

export function StatusBar({
  summary,
  consoleOpen,
  defaultSessionStrategy,
  onToggleConsole,
  focusedTab
}: StatusBarProps): JSX.Element {
  // Determine which metrics to display
  const displayMetrics = focusedTab?.metrics || {
    cpuPercent: summary.totalCpuPercent,
    memoryMb: summary.totalMemoryMb,
    processCount: summary.totalProcesses
  }
  const displayLabel = focusedTab ? focusedTab.label : 'Workspace Total'
  const displayPid = focusedTab?.pid

  return (
    <footer className="border-t border-white/10 bg-black/40 px-6 py-3 backdrop-blur-2xl shadow-[0_-4px_24px_rgba(0,0,0,0.4)]">
      <div className="flex flex-wrap items-center justify-between gap-6 text-xs text-white">
        <div className="flex flex-wrap items-center gap-4">
          {/* Focused Tab / Workspace Label */}
          <div className="flex items-center gap-2 rounded-full border border-white/10 bg-white/5 px-3 py-1 shadow-inner">
            <Layers3 className="h-3.5 w-3.5 text-white" />
            <span className="font-medium tracking-wide truncate max-w-[150px]">{displayLabel}</span>
          </div>

          {/* PID Display (if focused on a tab) */}
          {displayPid && (
            <div className="flex items-center gap-2 rounded-full border border-white/10 bg-white/5 px-3 py-1 shadow-inner">
              <span className="font-mono text-sentinel-mist">PID: {displayPid}</span>
            </div>
          )}

          {/* CPU */}
          <div className="flex items-center gap-2 rounded-full border border-sentinel-ice/30 bg-sentinel-ice/10 px-3 py-1 shadow-inner">
            <Cpu className="h-3.5 w-3.5 text-sentinel-ice" />
            <span className="font-mono text-sentinel-ice">{displayMetrics.cpuPercent.toFixed(1)}% CPU</span>
          </div>

          {/* RAM */}
          <div className="flex items-center gap-2 rounded-full border border-sentinel-accent/30 bg-sentinel-accent/10 px-3 py-1 shadow-inner">
            <MemoryStick className="h-3.5 w-3.5 text-sentinel-accent" />
            <span className="font-mono text-sentinel-glow">{displayMetrics.memoryMb.toFixed(1)} MB RAM</span>
          </div>

          <div className="flex items-center gap-2 rounded-full border border-white/10 bg-white/5 px-3 py-1 shadow-inner">
            <GitBranch className="h-3.5 w-3.5 text-white" />
            <span className="font-mono">{summary.branch || 'No branch selected'}</span>
          </div>
          <div className="flex items-center gap-2 rounded-full border border-white/10 bg-white/5 px-3 py-1 shadow-inner">
            <TerminalSquare className="h-3.5 w-3.5 text-sentinel-accent" />
            <span className="font-medium tracking-wide">
              {defaultSessionStrategy === 'sandbox-copy' ? 'SANDBOX COPY' : 'GIT WORKTREE'}
            </span>
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-4">
          <button
            className="flex items-center gap-2 rounded-full border border-sentinel-accent/30 bg-sentinel-accent/10 px-4 py-1 text-sentinel-glow transition hover:bg-sentinel-accent/20 shadow-inner"
            onClick={onToggleConsole}
            type="button"
          >
            <TerminalSquare className="h-3.5 w-3.5" />
            <span className="font-medium tracking-wide uppercase">{consoleOpen ? 'Hide Console' : 'Show Console'}</span>
            <span className="ml-1 rounded border border-sentinel-accent/30 bg-black/40 px-1.5 py-0.5 font-mono text-[9px]">Ctrl+~</span>
          </button>
          
          <div className="flex items-center gap-2 text-sentinel-mist bg-white/[0.03] border border-white/10 rounded-full px-3 py-1 shadow-inner">
            <Layers3 className="h-3.5 w-3.5 opacity-70" />
            <span className="font-mono">{(focusedTab?.metrics?.processCount ?? summary.totalProcesses)} procs</span>
          </div>
          <div className="flex items-center gap-2 text-sentinel-mist bg-white/[0.03] border border-white/10 rounded-full px-3 py-1 shadow-inner">
            <Clock3 className="h-3.5 w-3.5 opacity-70" />
            <span className="font-mono">upd {formatTime(summary.lastUpdated)}</span>
          </div>
        </div>
      </div>
    </footer>
  )
}
