import type { CSSProperties } from 'react'
import { GitBranch, PanelLeft, Plus, TerminalSquare } from 'lucide-react'

import type { ProjectState, WorkspaceContext } from '@shared/types'

import { WorkspaceSwitcher } from '../../components/WorkspaceSwitcher'
import type { WorkspaceAction } from '../support'

interface AppHeaderProps {
  activeWorkspaceId: string | null
  globalMode: 'ide' | 'multiplex'
  hasProject: boolean
  project: ProjectState
  workspaces: WorkspaceContext[]
  onCreateSession: () => void
  onCreateStandaloneTerminal: () => void
  onOpenProject: () => void
  onSwitchWorkspace: (workspaceId: string) => void
  onToggleSidebar: () => void
  onWorkspaceAction: (workspaceId: string, action: WorkspaceAction) => void
}

export function AppHeader({
  activeWorkspaceId,
  globalMode,
  hasProject,
  project,
  workspaces,
  onCreateSession,
  onCreateStandaloneTerminal,
  onOpenProject,
  onSwitchWorkspace,
  onToggleSidebar,
  onWorkspaceAction
}: AppHeaderProps): JSX.Element {
  return (
    <header
      className="relative z-20 flex h-11 shrink-0 items-center border-b border-white/10 bg-black/30 px-3"
      style={{ WebkitAppRegion: 'drag' } as CSSProperties}
    >
      <div
        className="z-10 flex min-w-0 items-center gap-3 pr-3"
        style={{ WebkitAppRegion: 'no-drag' } as CSSProperties}
      >
        <div className="flex shrink-0 items-center gap-3">
          <button
            className="inline-flex h-7 w-7 items-center justify-center text-sentinel-mist transition hover:text-white"
            onClick={onToggleSidebar}
            title="Toggle sidebar"
          >
            <PanelLeft className="h-4 w-4" />
          </button>

          <span className="whitespace-nowrap text-sm font-semibold tracking-tight text-white/90">
            Sentinel
          </span>
        </div>

        <div className="min-w-0 flex-1 max-w-[480px]">
          <WorkspaceSwitcher
            activeWorkspaceId={activeWorkspaceId}
            onCreateWorkspace={onOpenProject}
            onSwitchWorkspace={onSwitchWorkspace}
            onWorkspaceAction={onWorkspaceAction}
            workspaces={workspaces}
          />
        </div>

        {project.name && (
          <div className="hidden min-w-0 items-center gap-1.5 border border-white/10 bg-white/[0.04] px-2.5 py-1 text-[11px] text-sentinel-mist xl:flex">
            <GitBranch className="h-3 w-3 shrink-0" />
            <span className="truncate max-w-[220px]">{project.name}</span>
            {project.branch && <span className="text-sentinel-mist/55">·</span>}
            {project.branch && <span className="truncate max-w-[140px]">{project.branch}</span>}
          </div>
        )}

        {globalMode !== 'ide' && (
          <div
            className="ml-4 flex shrink-0 items-center gap-2"
            style={{ WebkitAppRegion: 'no-drag' } as CSSProperties}
          >
            <div className="flex items-center rounded-md border border-white/10 bg-black/40 p-0.5 shadow-sm">
              <button
                className="inline-flex h-7 items-center gap-1.5 rounded-sm px-3 text-[11px] font-semibold text-sentinel-mist transition-colors hover:bg-white/[0.08] hover:text-white"
                onClick={onCreateStandaloneTerminal}
                title="New Terminal"
              >
                <TerminalSquare className="h-3.5 w-3.5 text-white/55" />
                <span className="hidden xl:inline">New Terminal</span>
              </button>

              <div className="mx-0.5 h-3.5 w-px bg-white/10" />

              <button
                className="inline-flex h-7 items-center gap-1.5 rounded-sm px-3 text-[11px] font-semibold text-sentinel-accent transition-colors hover:bg-sentinel-accent/20 disabled:opacity-40"
                disabled={!hasProject}
                onClick={onCreateSession}
                title="New Agent"
              >
                <Plus className="h-4 w-4" />
                <span className="hidden xl:inline">New Agent</span>
              </button>
            </div>
          </div>
        )}
      </div>

      <div className="ml-auto w-[140px] shrink-0" />
    </header>
  )
}
