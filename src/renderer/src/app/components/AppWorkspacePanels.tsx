import type { ReactNode, RefObject } from 'react'
import { Group, Panel, PanelImperativeHandle, Separator } from 'react-resizable-panels'

import type {
  ProjectState,
  SessionWorkspaceStrategy,
  TabSummary,
  WorkspaceSummary
} from '@shared/types'

import { Sidebar } from '../../components/Sidebar'
import { StandaloneTerminalTile } from '../../components/StandaloneTerminalTile'
import { StatusBar } from '../../components/StatusBar'
import { WorkspaceTabs } from '../../components/WorkspaceTabs'
import type { SelectedFileEntry, WorkspaceOverlayFile } from '../../workspace-overlay'
import type { WorkspaceAction } from '../support'

interface AppWorkspacePanelsProps {
  activeStandaloneTab: TabSummary | null
  activeTabId: string
  activeWorkspaceId: string | null
  consoleOpen: boolean
  defaultSessionStrategy: SessionWorkspaceStrategy
  diffBadges: Record<string, string[]>
  fitNonce: number
  globalMode: 'ide' | 'multiplex'
  ideContent: ReactNode
  ideTabIds: string[]
  keepIdeMounted: boolean
  multiplexContent: ReactNode
  overlayFiles: WorkspaceOverlayFile[]
  project: ProjectState
  refreshingProject: boolean
  selectedFile: SelectedFileEntry | null
  shellViewportRef: RefObject<HTMLDivElement | null>
  sidebarCollapsed: boolean
  sidebarPanelRef: RefObject<PanelImperativeHandle | null>
  statusBarCollapsed: boolean
  summary: WorkspaceSummary
  visibleTabs: TabSummary[]
  windowsBuildNumber?: number
  onChangeDefaultSessionStrategy: (strategy: SessionWorkspaceStrategy) => void
  onFileSelect: (file: SelectedFileEntry) => void
  onOpenProject: () => void
  onRefreshProject: () => void
  onTabClose: (tabId: string) => void
  onTabSelect: (tabId: string) => void
  onToggleConsole: () => void
  onToggleGlobalMode: (mode: 'ide' | 'multiplex') => void
  onToggleSidebar: () => void
  onToggleStatusBarCollapse: () => void
  onWorkspaceAction: (workspaceId: string, action: WorkspaceAction) => void
}

export function AppWorkspacePanels({
  activeStandaloneTab,
  activeTabId,
  activeWorkspaceId,
  consoleOpen,
  defaultSessionStrategy,
  diffBadges,
  fitNonce,
  globalMode,
  ideContent,
  ideTabIds,
  keepIdeMounted,
  multiplexContent,
  overlayFiles,
  project,
  refreshingProject,
  selectedFile,
  shellViewportRef,
  sidebarCollapsed,
  sidebarPanelRef,
  statusBarCollapsed,
  summary,
  visibleTabs,
  windowsBuildNumber,
  onChangeDefaultSessionStrategy,
  onFileSelect,
  onOpenProject,
  onRefreshProject,
  onTabClose,
  onTabSelect,
  onToggleConsole,
  onToggleGlobalMode,
  onToggleSidebar,
  onToggleStatusBarCollapse,
  onWorkspaceAction
}: AppWorkspacePanelsProps): JSX.Element {
  return (
    <div className="flex flex-1 min-h-0 overflow-hidden" ref={shellViewportRef}>
      <Group orientation="horizontal">
        <Panel
          panelRef={sidebarPanelRef}
          defaultSize={18}
          minSize={0}
          collapsible
          collapsedSize={0}
          className="transition-[flex-basis,width,max-width,min-width] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)]"
          style={{ overflow: 'hidden' }}
        >
          <Sidebar
            collapsed={sidebarCollapsed}
            defaultSessionStrategy={defaultSessionStrategy}
            diffBadges={diffBadges}
            globalMode={globalMode}
            onChangeDefaultSessionStrategy={onChangeDefaultSessionStrategy}
            onFileSelect={onFileSelect}
            onOpenProject={onOpenProject}
            onRefreshProject={onRefreshProject}
            onToggleCollapse={onToggleSidebar}
            onToggleGlobalMode={onToggleGlobalMode}
            onWorkspaceAction={(action) => {
              if (!activeWorkspaceId) {
                return
              }

              onWorkspaceAction(activeWorkspaceId, action)
            }}
            overlayFiles={overlayFiles}
            project={project}
            refreshing={refreshingProject}
            selectedFileProjectPath={selectedFile?.projectPath}
          />
        </Panel>

        <Separator
          className={`relative bg-transparent transition-[width,opacity] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)] ${
            sidebarCollapsed
              ? 'pointer-events-none w-0 opacity-0'
              : 'w-[3px] opacity-100 hover:bg-sentinel-accent/20 active:bg-sentinel-accent/40'
          }`}
        />

        <Panel className="flex flex-col min-h-0 min-w-0" defaultSize={82}>
          {globalMode !== 'ide' && (
            <WorkspaceTabs
              activeTabId={activeTabId}
              onTabClose={onTabClose}
              onTabSelect={onTabSelect}
              tabs={visibleTabs.filter((tab) => !ideTabIds.includes(tab.id))}
            />
          )}

          <div className="relative flex-1 min-h-0 overflow-hidden">
            <div
              className={`absolute inset-0 min-h-0 overflow-hidden ${
                activeTabId === 'dashboard' ? 'opacity-100 z-10' : 'opacity-0 z-0 pointer-events-none'
              }`}
            >
              {multiplexContent}
            </div>

            {activeStandaloneTab && (
              <div className="absolute inset-0 min-h-0 overflow-hidden opacity-100 z-10">
                <StandaloneTerminalTile
                  key={activeStandaloneTab.id}
                  fitNonce={fitNonce}
                  onClose={() => onTabClose(activeStandaloneTab.id)}
                  tab={activeStandaloneTab}
                  windowsBuildNumber={windowsBuildNumber}
                />
              </div>
            )}

            {(keepIdeMounted || globalMode === 'ide') && (
              <div
                aria-hidden={globalMode !== 'ide'}
                className={`absolute inset-0 min-h-0 overflow-hidden transition-opacity duration-150 ${
                  globalMode === 'ide' ? 'opacity-100 z-20' : 'opacity-0 z-0 pointer-events-none'
                }`}
              >
                {ideContent}
              </div>
            )}
          </div>

          <StatusBar
            collapsed={statusBarCollapsed}
            consoleOpen={consoleOpen}
            defaultSessionStrategy={defaultSessionStrategy}
            focusedTab={activeStandaloneTab}
            onToggleCollapse={onToggleStatusBarCollapse}
            onToggleConsole={onToggleConsole}
            summary={summary}
          />
        </Panel>
      </Group>
    </div>
  )
}
