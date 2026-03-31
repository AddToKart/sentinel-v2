import type { ProjectState, SessionWorkspaceStrategy } from '@shared/types'
import type { SelectedFileEntry, WorkspaceOverlayFile } from '../../workspace-overlay'

export interface SidebarProps {
  project: ProjectState
  refreshing: boolean
  collapsed: boolean
  diffBadges: Record<string, string[]>
  overlayFiles: WorkspaceOverlayFile[]
  defaultSessionStrategy: SessionWorkspaceStrategy
  selectedFileProjectPath?: string
  onOpenProject: () => void
  onRefreshProject: () => void
  onChangeDefaultSessionStrategy: (strategy: SessionWorkspaceStrategy) => void
  onToggleCollapse: () => void
  onFileSelect: (file: SelectedFileEntry) => void
  globalMode: 'multiplex' | 'ide'
  onToggleGlobalMode: (mode: 'multiplex' | 'ide') => void
  onWorkspaceAction: (action: 'delete' | 'stop' | 'pause') => void
}
