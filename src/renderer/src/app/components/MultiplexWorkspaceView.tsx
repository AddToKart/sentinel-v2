import { Suspense } from 'react'
import { FolderOpen, TerminalSquare } from 'lucide-react'

import type { SessionCommandEntry, SessionSummary } from '@shared/types'

import { AgentDashboard } from '../../components/AgentDashboard'

interface MultiplexWorkspaceViewProps {
  fitNonce: number
  hasProject: boolean
  histories: Record<string, SessionCommandEntry[]>
  maximizedSessionId: string | null
  onDeleteSession: (sessionId: string) => Promise<void>
  sessionDiffs: Record<string, string[]>
  sessions: SessionSummary[]
  windowsBuildNumber?: number
  onCloseSession: (sessionId: string) => Promise<void>
  onOpenProject: () => void
  onPauseSession: (sessionId: string) => Promise<void>
  onResumeSession: (sessionId: string) => Promise<void>
  onToggleMaximize: (sessionId: string) => void
  layoutMode: 'grid' | 'master-stack'
  onSetLayoutMode: (mode: 'grid' | 'master-stack') => void
}

export function MultiplexWorkspaceView({
  fitNonce,
  hasProject,
  histories,
  maximizedSessionId,
  onDeleteSession,
  sessionDiffs,
  sessions,
  windowsBuildNumber,
  onCloseSession,
  onOpenProject,
  onPauseSession,
  onResumeSession,
  onToggleMaximize,
  layoutMode,
  onSetLayoutMode
}: MultiplexWorkspaceViewProps): JSX.Element {
  if (!hasProject) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="max-w-xs text-center p-8 border border-white/10 bg-white/[0.02]">
          <FolderOpen className="mx-auto mb-4 h-10 w-10 text-sentinel-mist/40" />
          <h2 className="mb-2 text-base font-bold text-white/90">Open a Repository</h2>
          <p className="mb-6 text-sm text-sentinel-mist">Select a project folder to start sandbox-copy or Git worktree agent sessions.</p>
          <button
            className="inline-flex h-9 w-full items-center justify-center gap-2 bg-white text-sm font-bold text-sentinel-ink hover:bg-white/90 transition"
            onClick={onOpenProject}
          >
            Open Project
          </button>
        </div>
      </div>
    )
  }

  if (sessions.length === 0) {
    return (
      <div className="flex h-full items-center justify-center text-center text-sentinel-mist">
        <div>
          <TerminalSquare className="mx-auto mb-4 h-10 w-10 opacity-30" />
          <p className="text-sm">No active agents yet. Start one with <strong className="text-white">New Agent</strong> using the workspace strategy selected in the sidebar.</p>
        </div>
      </div>
    )
  }

  return (
    <Suspense fallback={<div className="flex h-full items-center justify-center text-sm text-sentinel-mist">Loading...</div>}>
      <AgentDashboard
        fitNonce={fitNonce}
        histories={histories}
        maximizedSessionId={maximizedSessionId}
        onDelete={onDeleteSession}
        onClose={onCloseSession}
        onPause={onPauseSession}
        onResume={onResumeSession}
        onToggleMaximize={onToggleMaximize}
        sessionDiffs={sessionDiffs}
        sessions={sessions}
        windowsBuildNumber={windowsBuildNumber}
        layoutMode={layoutMode}
        onSetLayoutMode={onSetLayoutMode}
      />
    </Suspense>
  )
}
