import { Clock3, Cpu, GitBranch, Layers3, MemoryStick, TerminalSquare } from 'lucide-react'

import type { SessionWorkspaceStrategy, TabSummary, WorkspaceSummary } from '@shared/types'

interface StatusBarProps {
  summary: WorkspaceSummary
  consoleOpen: boolean
  defaultSessionStrategy: SessionWorkspaceStrategy
  onToggleConsole: () => void
  focusedTab?: TabSummary | null
  collapsed: boolean
  onToggleCollapse: () => void
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
  focusedTab,
  collapsed,
  onToggleCollapse
}: StatusBarProps): JSX.Element {
  // Determine which metrics to display
  const displayMetrics = focusedTab?.metrics || {
    cpuPercent: summary.totalCpuPercent,
    memoryMb: summary.totalMemoryMb,
    processCount: summary.totalProcesses
  }
  const displayLabel = focusedTab ? focusedTab.label : (summary.activeWorkspaceName ?? 'Workspace Total')
  const displayPid = focusedTab?.pid

  if (collapsed) {
    return (
      <footer className="absolute bottom-0 right-4 z-50">
        <button
          onClick={onToggleCollapse}
          className="flex items-center justify-center rounded-t-md border-x border-t border-white/10 bg-black/60 px-3 py-1 text-sentinel-mist backdrop-blur-2xl transition hover:bg-black/80 hover:text-white"
          title="Expand Status Bar"
        >
          <Layers3 className="h-3.5 w-3.5 opacity-70" />
        </button>
      </footer>
    )
  }

  return (
    <footer className="shrink-0 border-t border-white/10 bg-black/40 px-4 py-1.5 backdrop-blur-2xl shadow-[0_-4px_24px_rgba(0,0,0,0.4)]">
      <div className="flex items-center justify-between gap-3 text-xs text-white">
        <div className="flex items-center gap-2.5 min-w-0">
          <div className="flex items-center gap-2 rounded-full border border-white/10 bg-white/5 px-2.5 py-0.5 shadow-inner">
            <Layers3 className="h-3 w-3 text-white" />
            <span className="font-medium tracking-wide truncate max-w-[120px]">{displayLabel}</span>
          </div>

          {/* PID Display (if focused on a tab) */}
          {displayPid && (
            <div className="flex items-center gap-2 rounded-full border border-white/10 bg-white/5 px-2.5 py-0.5 shadow-inner">
              <span className="font-mono text-sentinel-mist">PID: {displayPid}</span>
            </div>
          )}

          {/* CPU */}
          <div className="flex items-center gap-2 rounded-full border border-sentinel-ice/30 bg-sentinel-ice/10 px-2.5 py-0.5 shadow-inner">
            <Cpu className="h-3 w-3 text-sentinel-ice" />
            <span className="font-mono text-sentinel-ice">{displayMetrics.cpuPercent.toFixed(1)}%</span>
          </div>

          {/* RAM */}
          <div className="flex items-center gap-2 rounded-full border border-sentinel-accent/30 bg-sentinel-accent/10 px-2.5 py-0.5 shadow-inner">
            <MemoryStick className="h-3 w-3 text-sentinel-accent" />
            <span className="font-mono text-sentinel-glow group-hover:block">{displayMetrics.memoryMb.toFixed(1)} MB</span>
          </div>

          <div className="hidden sm:flex items-center gap-2 rounded-full border border-white/10 bg-white/5 px-2.5 py-0.5 shadow-inner">
            <GitBranch className="h-3 w-3 text-white" />
            <span className="font-mono truncate max-w-[100px]">{summary.branch || 'None'}</span>
          </div>
          <div className="hidden lg:flex items-center gap-2 rounded-full border border-white/10 bg-white/5 px-2.5 py-0.5 shadow-inner">
            <Layers3 className="h-3 w-3 text-white" />
            <span className="font-mono">{summary.workspaceCount} ws</span>
          </div>
          <div className="hidden lg:flex items-center gap-2 rounded-full border border-white/10 bg-white/5 px-2.5 py-0.5 shadow-inner">
            <TerminalSquare className="h-3 w-3 text-sentinel-accent" />
            <span className="font-medium tracking-wide">
              {defaultSessionStrategy === 'sandbox-copy' ? 'SANDBOX' : 'WORKTREE'}
            </span>
          </div>
        </div>

        <div className="flex items-center gap-2.5 shrink-0">
          <button
            className="flex items-center gap-2 rounded-full border border-sentinel-accent/30 bg-sentinel-accent/10 px-4 py-1 text-sentinel-glow transition hover:bg-sentinel-accent/20 shadow-inner"
            onClick={onToggleConsole}
            type="button"
          >
            <TerminalSquare className="h-3.5 w-3.5" />
            <span className="font-medium tracking-wide uppercase">{consoleOpen ? 'Hide Console' : 'Show Console'}</span>
            <span className="ml-1 rounded border border-sentinel-accent/30 bg-black/40 px-1.5 py-0.5 font-mono text-[9px]">Ctrl+J</span>
          </button>
          
          <div className="flex items-center gap-2 text-sentinel-mist bg-white/[0.03] border border-white/10 rounded-full px-3 py-1 shadow-inner">
            <Layers3 className="h-3.5 w-3.5 opacity-70" />
            <span className="font-mono">
              {focusedTab?.metrics?.processCount ?? summary.totalProcesses} procs
            </span>
          </div>
          <div className="hidden md:flex items-center gap-2 text-sentinel-mist bg-white/[0.03] border border-white/10 rounded-full px-3 py-1 shadow-inner">
            <TerminalSquare className="h-3.5 w-3.5 opacity-70" />
            <span className="font-mono">
              {summary.activeWorkspaceSessionCount}/{summary.totalSessions} sessions
            </span>
          </div>
          <div className="flex items-center gap-2 text-sentinel-mist bg-white/[0.03] border border-white/10 rounded-full px-3 py-1 shadow-inner">
            <Clock3 className="h-3.5 w-3.5 opacity-70" />
            <span className="font-mono">upd {formatTime(summary.lastUpdated)}</span>
          </div>

          <button
            onClick={onToggleCollapse}
            className="flex items-center justify-center rounded-full border border-white/10 bg-white/[0.03] p-1.5 text-sentinel-mist transition hover:bg-white/[0.08] hover:text-white"
            title="Collapse Status Bar"
          >
            <svg width="12" height="12" viewBox="0 0 15 15" fill="none" xmlns="http://www.w3.org/2000/svg" className="h-3 w-3">
              <path d="M7.5 12L0 4.5L1.05 3.45L7.5 9.9L13.95 3.45L15 4.5L7.5 12Z" fill="currentColor"></path>
            </svg>
          </button>
        </div>
      </div>
    </footer>
  )
}
