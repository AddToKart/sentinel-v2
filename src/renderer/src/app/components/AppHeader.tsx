import type { CSSProperties } from 'react'
import { GitBranch, LayoutGrid, PanelLeft, Plus, Sidebar as SidebarIcon, TerminalSquare } from 'lucide-react'

import type { ProjectState, WorkspaceContext } from '@shared/types'

import { WorkspaceSwitcher } from '../../components/WorkspaceSwitcher'
import { ChangesToggleButton } from '../../components/changes-manager/ChangesToggleButton'
import { WorkspaceNotifications } from './WorkspaceNotifications'
import type { WorkspaceNotification } from '../hooks/useWorkspaceNotifications'
import type { WorkspaceAction } from '../support'

interface AppHeaderProps {
  activeWorkspaceId: string | null
  bellRinging: boolean
  globalMode: 'ide' | 'multiplex'
  hasProject: boolean
  notifications: WorkspaceNotification[]
  previewNotification: WorkspaceNotification | null
  project: ProjectState
  runningSessionCountsByWorkspace: Record<string, number>
  sessionCountsByWorkspace: Record<string, number>
  tabCountsByWorkspace: Record<string, number>
  unreadNotificationCountsByWorkspace: Record<string, number>
  unreadNotificationCount: number
  workspaces: WorkspaceContext[]
  onClearNotifications: () => void
  onClearPreviewNotification: () => void
  onCreateSession: () => void
  onCreateStandaloneTerminal: () => void
  onDismissNotification: (notificationId: string) => void
  onMarkAllNotificationsRead: () => void
  onOpenProject: () => void
  onSwitchWorkspace: (workspaceId: string) => void
  onToggleSidebar: () => void
  onToggleChangesManager: () => void
  onWorkspaceAction: (workspaceId: string, action: WorkspaceAction) => void
  layoutMode: 'grid' | 'master-stack'
  onSetLayoutMode: (mode: 'grid' | 'master-stack') => void
  changesCount: number
  hasConflicts: boolean
}

export function AppHeader({
  activeWorkspaceId,
  bellRinging,
  globalMode,
  hasProject,
  notifications,
  previewNotification,
  project,
  runningSessionCountsByWorkspace,
  sessionCountsByWorkspace,
  tabCountsByWorkspace,
  unreadNotificationCountsByWorkspace,
  unreadNotificationCount,
  workspaces,
  onClearNotifications,
  onClearPreviewNotification,
  onCreateSession,
  onCreateStandaloneTerminal,
  onDismissNotification,
  onMarkAllNotificationsRead,
  onOpenProject,
  onSwitchWorkspace,
  onToggleSidebar,
  onToggleChangesManager,
  onWorkspaceAction,
  layoutMode,
  onSetLayoutMode,
  changesCount,
  hasConflicts
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
            runningSessionCounts={runningSessionCountsByWorkspace}
            sessionCounts={sessionCountsByWorkspace}
            tabCounts={tabCountsByWorkspace}
            unreadNotificationCounts={unreadNotificationCountsByWorkspace}
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

      <div
        className="ml-auto flex shrink-0 items-center gap-2"
        style={{ WebkitAppRegion: 'no-drag' } as CSSProperties}
      >
        {activeWorkspaceId && (sessionCountsByWorkspace[activeWorkspaceId] ?? 0) >= 3 && (
          <div className="mr-2 flex items-center gap-0.5 rounded-md border border-white/10 bg-black/40 p-0.5 shadow-sm">
            <button
              className={`rounded px-2 py-0.5 text-[10px] transition flex items-center gap-1.5 ${
                layoutMode === 'grid' ? 'bg-sentinel-accent/15 text-sentinel-accent' : 'text-sentinel-mist hover:text-white hover:bg-white/5'
              }`}
              onClick={() => onSetLayoutMode('grid')}
              title="Grid Layout"
              type="button"
            >
              <LayoutGrid className="h-3 w-3" />
            </button>
            <button
              className={`rounded px-2 py-0.5 text-[10px] transition flex items-center gap-1.5 ${
                layoutMode === 'master-stack' ? 'bg-sentinel-accent/15 text-sentinel-accent' : 'text-sentinel-mist hover:text-white hover:bg-white/5'
              }`}
              onClick={() => onSetLayoutMode('master-stack')}
              title="Master-Stack Layout"
              type="button"
            >
              <SidebarIcon className="h-3 w-3" />
            </button>
          </div>
        )}
        <WorkspaceNotifications
          bellRinging={bellRinging}
          notifications={notifications}
          onClearNotifications={onClearNotifications}
          onClearPreviewNotification={onClearPreviewNotification}
          onDismissNotification={onDismissNotification}
          onMarkAllRead={onMarkAllNotificationsRead}
          previewNotification={previewNotification}
          unreadCount={unreadNotificationCount}
        />
        <ChangesToggleButton
          onChangeCount={changesCount}
          hasConflicts={hasConflicts}
          onClick={onToggleChangesManager}
        />
      </div>
    </header>
  )
}
