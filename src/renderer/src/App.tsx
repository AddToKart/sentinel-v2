import { useCallback, useEffect, useRef, useState } from 'react'
import { FolderOpen, PanelLeft, Plus, RefreshCw, TerminalSquare } from 'lucide-react'
import type { PanelImperativeHandle } from 'react-resizable-panels'

import { AppHeader } from './app/components/AppHeader'
import { AppWorkspacePanels } from './app/components/AppWorkspacePanels'
import { BridgeUnavailableScreen } from './app/components/BridgeUnavailableScreen'
import { IdeWorkspaceView } from './app/components/IdeWorkspaceView'
import { MultiplexWorkspaceView } from './app/components/MultiplexWorkspaceView'
import { useKeyboardShortcuts } from './app/hooks/useKeyboardShortcuts'
import { useSentinelBootstrap } from './app/hooks/useSentinelBootstrap'
import {
  defaultIdeTerminalState,
  defaultSummary,
  emptyProject,
  getSentinelBridge,
  missingBridgeMessage,
  type WorkspaceAction
} from './app/support'
import { ConsoleDrawer } from './components/ConsoleDrawer'
import { GlobalActionBar } from './components/GlobalActionBar'
import { getErrorMessage } from './error-utils'
import { clearIdeTerminalOutput, clearSessionOutput } from './terminal-stream'
import { clearTabOutput } from './tab-stream'
import {
  buildWorkspaceOverlayFiles,
  collectProjectPaths,
  type SelectedFileEntry
} from './workspace-overlay'

import type {
  ActivityLogEntry,
  IdeTerminalState,
  ProjectState,
  SentinelApi,
  SessionCommandEntry,
  SessionSummary,
  SessionWorkspaceStrategy,
  TabSummary,
  WorkspaceContext,
  WorkspaceSummary
} from '@shared/types'

export default function App(): JSX.Element {
  const [project, setProject] = useState<ProjectState>(emptyProject())
  const [workspaces, setWorkspaces] = useState<WorkspaceContext[]>([])
  const [activeWorkspaceId, setActiveWorkspaceId] = useState<string | null>(null)
  const [sessions, setSessions] = useState<SessionSummary[]>([])
  const [workspaceSummary, setWorkspaceSummary] = useState<WorkspaceSummary>(defaultSummary())
  const [sessionHistories, setSessionHistories] = useState<Record<string, SessionCommandEntry[]>>({})
  const [sessionDiffs, setSessionDiffs] = useState<Record<string, string[]>>({})
  const [activityLog, setActivityLog] = useState<ActivityLogEntry[]>([])
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false)
  const [consoleOpen, setConsoleOpen] = useState(false)
  const [fitNonce, setFitNonce] = useState(0)
  const [maximizedSessionId, setMaximizedSessionId] = useState<string | null>(null)
  const [errorMessage, setErrorMessage] = useState<string | null>(null)
  const [selectedFile, setSelectedFile] = useState<SelectedFileEntry | null>(null)
  const [globalActionBarOpen, setGlobalActionBarOpen] = useState(false)
  const [globalMode, setGlobalMode] = useState<'multiplex' | 'ide'>('multiplex')
  const [keepIdeMounted, setKeepIdeMounted] = useState(false)
  const [refreshingProject, setRefreshingProject] = useState(false)
  const [defaultSessionStrategy, setDefaultSessionStrategy] = useState<SessionWorkspaceStrategy>('sandbox-copy')
  const [ideTerminalState, setIdeTerminalState] = useState<IdeTerminalState>(defaultIdeTerminalState())
  const [windowsBuildNumber, setWindowsBuildNumber] = useState<number | undefined>(undefined)
  const [tabs, setTabs] = useState<TabSummary[]>([])
  const [ideTabIds, setIdeTabIds] = useState<string[]>([])
  const [activeTabId, setActiveTabId] = useState<string>('dashboard')
  const [activeIdeTerminalId, setActiveIdeTerminalId] = useState<string>('ide-workspace')
  const [ideTerminalCollapsed, setIdeTerminalCollapsed] = useState(false)
  const [statusBarCollapsed, setStatusBarCollapsed] = useState(false)

  const sidebarPanelRef = useRef<PanelImperativeHandle | null>(null)
  const ideTerminalPanelRef = useRef<PanelImperativeHandle | null>(null)
  const shellViewportRef = useRef<HTMLDivElement | null>(null)
  const fitTimerRef = useRef<number | null>(null)
  const workspacesRef = useRef<WorkspaceContext[]>([])

  const bridgeAvailable = Boolean(getSentinelBridge())
  const activeWorkspace =
    workspaces.find((workspace) => workspace.id === activeWorkspaceId) ?? null
  const visibleSessions = activeWorkspaceId
    ? sessions.filter((session) => session.workspaceId === activeWorkspaceId)
    : []
  const visibleTabs = activeWorkspaceId
    ? tabs.filter((tab) => tab.workspaceId === activeWorkspaceId)
    : []

  function requestTerminalFit(delay = 140): void {
    if (fitTimerRef.current) {
      window.clearTimeout(fitTimerRef.current)
    }

    fitTimerRef.current = window.setTimeout(() => {
      fitTimerRef.current = null
      setFitNonce((value) => value + 1)
    }, delay)
  }

  function requireSentinelBridge(): SentinelApi | null {
    const sentinel = getSentinelBridge()
    if (!sentinel) {
      setErrorMessage(missingBridgeMessage())
      return null
    }

    return sentinel
  }

  useEffect(() => {
    workspacesRef.current = workspaces
  }, [workspaces])

  useSentinelBootstrap({
    workspacesRef,
    setActivityLog,
    setActiveWorkspaceId,
    setDefaultSessionStrategy,
    setErrorMessage,
    setIdeTerminalState,
    setMaximizedSessionId,
    setProject,
    setSelectedFile,
    setSessionDiffs,
    setSessionHistories,
    setSessions,
    setTabs,
    setWindowsBuildNumber,
    setWorkspaceSummary,
    setWorkspaces
  })

  useEffect(() => {
    const observer = new ResizeObserver(() => {
      requestTerminalFit()
    })

    if (shellViewportRef.current) {
      observer.observe(shellViewportRef.current)
    }

    return () => {
      observer.disconnect()
      if (fitTimerRef.current) {
        window.clearTimeout(fitTimerRef.current)
        fitTimerRef.current = null
      }
    }
  }, [])

  useEffect(() => {
    requestTerminalFit(120)
  }, [sidebarCollapsed, consoleOpen, visibleSessions.length, maximizedSessionId, activeTabId])

  useEffect(() => {
    if (globalMode === 'ide') {
      setKeepIdeMounted(true)
    }
  }, [globalMode])

  useEffect(() => {
    if (project.path) {
      setSidebarCollapsed(false)
      sidebarPanelRef.current?.expand()
    }
  }, [project.path])

  useEffect(() => {
    if (activeTabId === 'dashboard') {
      return
    }

    if (!visibleTabs.some((tab) => tab.id === activeTabId)) {
      setActiveTabId(visibleTabs[0]?.id ?? 'dashboard')
    }
  }, [activeTabId, visibleTabs])

  useEffect(() => {
    if (activeWorkspace) {
      setDefaultSessionStrategy(activeWorkspace.defaultSessionStrategy)
      return
    }

    setDefaultSessionStrategy('sandbox-copy')
  }, [activeWorkspace])

  useEffect(() => {
    setIdeTabIds((current) => current.filter((tabId) => tabs.some((tab) => tab.id === tabId)))
  }, [tabs])

  useEffect(() => {
    const activeSessionIds = new Set(sessions.map((session) => session.id))

    setSessionHistories((current) =>
      Object.fromEntries(
        Object.entries(current).filter(([sessionId]) => activeSessionIds.has(sessionId))
      )
    )
    setSessionDiffs((current) =>
      Object.fromEntries(
        Object.entries(current).filter(([sessionId]) => activeSessionIds.has(sessionId))
      )
    )
  }, [sessions])

  useEffect(() => {
    if (activeIdeTerminalId === 'ide-workspace') {
      return
    }

    const hasVisibleIdeTab = visibleTabs.some(
      (tab) => ideTabIds.includes(tab.id) && tab.id === activeIdeTerminalId
    )

    if (!hasVisibleIdeTab) {
      setActiveIdeTerminalId('ide-workspace')
    }
  }, [activeIdeTerminalId, ideTabIds, visibleTabs])

  const toggleIdeTerminal = useCallback(() => {
    setIdeTerminalCollapsed((current) => {
      if (current) {
        ideTerminalPanelRef.current?.expand()
      } else {
        ideTerminalPanelRef.current?.collapse()
      }
      return !current
    })
  }, [])

  useKeyboardShortcuts({
    onToggleConsole: () => setConsoleOpen((value) => !value),
    onToggleGlobalActionBar: () => setGlobalActionBarOpen((value) => !value),
    onToggleIdeTerminal: toggleIdeTerminal
  })

  function toggleSidebar(): void {
    if (sidebarCollapsed) {
      sidebarPanelRef.current?.expand()
    } else {
      sidebarPanelRef.current?.collapse()
    }
    setSidebarCollapsed((value) => !value)
  }

  async function handleOpenProject(): Promise<void> {
    const sentinel = requireSentinelBridge()
    if (!sentinel) {
      return
    }

    try {
      const previousProjectPath = project.path
      const nextProject = await sentinel.selectProject()
      if (nextProject.path !== previousProjectPath) {
        clearIdeTerminalOutput()
      }
      setProject(nextProject)
      setSelectedFile(null)
    } catch (error) {
      const message = getErrorMessage(error)
      if (message !== 'Dialog cancelled') {
        setErrorMessage(`Failed to open project: ${message}`)
      }
    }
  }

  async function handleRefreshProject(): Promise<void> {
    if (!project.path) {
      return
    }

    const sentinel = requireSentinelBridge()
    if (!sentinel) {
      return
    }

    setRefreshingProject(true)
    try {
      setProject(await sentinel.refreshProject())
    } catch (error) {
      setErrorMessage(`Failed to refresh: ${getErrorMessage(error)}`)
    } finally {
      setRefreshingProject(false)
    }
  }

  async function handleWorkspaceAction(
    workspaceId: string,
    action: WorkspaceAction
  ): Promise<void> {
    const sentinel = requireSentinelBridge()
    if (!sentinel) {
      return
    }

    try {
      if (action === 'delete') {
        await sentinel.closeWorkspace(workspaceId, true)
      } else if (action === 'stop') {
        await sentinel.stopWorkspace(workspaceId)
      } else {
        await sentinel.pauseWorkspace(workspaceId)
      }
    } catch (error) {
      setErrorMessage(`Failed to ${action} workspace: ${getErrorMessage(error)}`)
    }
  }

  async function handleSwitchWorkspace(workspaceId: string): Promise<void> {
    if (workspaceId === activeWorkspaceId) {
      return
    }

    const sentinel = requireSentinelBridge()
    if (!sentinel) {
      return
    }

    try {
      await sentinel.switchWorkspace(workspaceId)
      setActiveTabId('dashboard')
      setSelectedFile(null)
    } catch (error) {
      setErrorMessage(`Failed to switch workspace: ${getErrorMessage(error)}`)
    }
  }

  async function handleCreateSession(): Promise<void> {
    if (!activeWorkspace || !project.path) {
      return
    }

    const sentinel = requireSentinelBridge()
    if (!sentinel) {
      return
    }

    try {
      await sentinel.createSession({ workspaceStrategy: defaultSessionStrategy })
    } catch (error) {
      setErrorMessage(`Failed to start session: ${getErrorMessage(error)}`)
    }
  }

  async function handleCloseSession(sessionId: string): Promise<void> {
    if (maximizedSessionId === sessionId) {
      setMaximizedSessionId(null)
    }

    const sentinel = requireSentinelBridge()
    if (!sentinel) {
      return
    }

    try {
      await sentinel.closeSession(sessionId)
      clearSessionOutput(sessionId)
      setSessions((current) => current.filter((session) => session.id !== sessionId))
      setSessionHistories((current) => {
        const next = { ...current }
        delete next[sessionId]
        return next
      })
      setSessionDiffs((current) => {
        const next = { ...current }
        delete next[sessionId]
        return next
      })
    } catch (error) {
      setErrorMessage(`Failed to close session: ${getErrorMessage(error)}`)
    }
  }

  async function handleChangeDefaultSessionStrategy(
    strategy: SessionWorkspaceStrategy
  ): Promise<void> {
    const sentinel = requireSentinelBridge()
    if (!sentinel) {
      return
    }

    try {
      const nextPreferences = await sentinel.setDefaultSessionStrategy(strategy)
      setDefaultSessionStrategy(nextPreferences.defaultSessionStrategy)
    } catch (error) {
      setErrorMessage(`Failed to update workspace strategy: ${getErrorMessage(error)}`)
    }
  }

  async function handleCreateStandaloneTerminal(): Promise<void> {
    if (!activeWorkspace) {
      return
    }

    const sentinel = requireSentinelBridge()
    if (!sentinel) {
      return
    }

    try {
      const newTab = await sentinel.createStandaloneTerminal(undefined, undefined, 80, 24)
      setTabs((current) => [...current, newTab])
      setActiveTabId(newTab.id)
    } catch (error) {
      setErrorMessage(`Failed to create terminal: ${getErrorMessage(error)}`)
    }
  }

  async function handleCreateIdeTerminal(): Promise<void> {
    const sentinel = requireSentinelBridge()
    if (!sentinel) {
      return
    }

    try {
      const newTab = await sentinel.createStandaloneTerminal(
        ideTerminalState?.workspacePath,
        undefined,
        80,
        24
      )
      setIdeTabIds((current) => [...current, newTab.id])
      setTabs((current) => [...current, newTab])
      setActiveIdeTerminalId(newTab.id)
    } catch (error) {
      setErrorMessage(getErrorMessage(error))
    }
  }

  async function handleCloseTab(tabId: string): Promise<void> {
    const sentinel = requireSentinelBridge()
    if (!sentinel) {
      return
    }

    const tabToClose = tabs.find((tab) => tab.id === tabId)
    if (!tabToClose) {
      return
    }

    setTabs((currentTabs) => currentTabs.filter((tab) => tab.id !== tabId))
    setIdeTabIds((current) => current.filter((id) => id !== tabId))
    clearTabOutput(tabId)

    try {
      await sentinel.closeTab(tabId)
    } catch (error) {
      const message = getErrorMessage(error)
      if (message.toLowerCase().includes('not found')) {
        return
      }

      setTabs((currentTabs) => {
        if (currentTabs.some((tab) => tab.id === tabId)) {
          return currentTabs
        }
        return [...currentTabs, tabToClose]
      })
      setErrorMessage(`Failed to close tab: ${message}`)
    }
  }

  const hasProject = Boolean(project.path) && Boolean(activeWorkspace)
  const overlayFiles = buildWorkspaceOverlayFiles({
    projectPath: project.path,
    ideTerminalState,
    sessions: visibleSessions,
    sessionDiffs,
    globalMode,
    maximizedSessionId
  })
  const projectPaths = collectProjectPaths(project.tree)
  const diffBadges = Object.fromEntries(
    overlayFiles.map((file) => [
      file.projectPath,
      [projectPaths.has(file.projectPath) ? 'modified' : 'new']
    ])
  )
  const activeStandaloneTab =
    activeTabId !== 'dashboard'
      ? visibleTabs.find((tab) => tab.id === activeTabId) ?? null
      : null

  const multiplexContent = (
    <MultiplexWorkspaceView
      fitNonce={fitNonce}
      hasProject={hasProject}
      histories={sessionHistories}
      maximizedSessionId={maximizedSessionId}
      onCloseSession={handleCloseSession}
      onOpenProject={() => { void handleOpenProject() }}
      onToggleMaximize={(id) => setMaximizedSessionId((current) => current === id ? null : id)}
      sessionDiffs={sessionDiffs}
      sessions={visibleSessions}
      windowsBuildNumber={windowsBuildNumber}
    />
  )

  const ideContent = (
    <IdeWorkspaceView
      activeTerminalId={activeIdeTerminalId}
      fitNonce={fitNonce}
      ideTerminalCollapsed={ideTerminalCollapsed}
      ideTerminalPanelRef={ideTerminalPanelRef}
      ideTerminalState={ideTerminalState}
      onCloseSelectedFile={() => setSelectedFile(null)}
      onCloseTerminal={async (id) => {
        await handleCloseTab(id)
        if (activeIdeTerminalId === id) {
          setActiveIdeTerminalId('ide-workspace')
        }
      }}
      onCreateTerminal={handleCreateIdeTerminal}
      onSelectTerminal={setActiveIdeTerminalId}
      onToggleCollapse={toggleIdeTerminal}
      projectPath={project.path}
      selectedFile={selectedFile}
      tabs={visibleTabs.filter((tab) => ideTabIds.includes(tab.id))}
      windowsBuildNumber={windowsBuildNumber}
    />
  )

  const globalActions = [
    { id: 'new-agent', label: 'New Agent', icon: <Plus className="h-4 w-4" />, execute: () => void handleCreateSession() },
    { id: 'open-project', label: 'Open Repository', icon: <FolderOpen className="h-4 w-4" />, execute: () => void handleOpenProject() },
    { id: 'refresh-project', label: 'Refresh Tree', icon: <RefreshCw className="h-4 w-4" />, execute: () => void handleRefreshProject() },
    { id: 'toggle-sidebar', label: 'Toggle Sidebar', icon: <PanelLeft className="h-4 w-4" />, execute: toggleSidebar },
    { id: 'toggle-console', label: 'Toggle Console', icon: <TerminalSquare className="h-4 w-4" />, execute: () => setConsoleOpen((value) => !value) },
    { id: 'sandbox-mode', label: 'Use Sandbox Copy', icon: <TerminalSquare className="h-4 w-4" />, execute: () => void handleChangeDefaultSessionStrategy('sandbox-copy') },
    { id: 'worktree-mode', label: 'Use Git Worktree', icon: <TerminalSquare className="h-4 w-4" />, execute: () => void handleChangeDefaultSessionStrategy('git-worktree') },
    { id: 'ide-mode', label: 'Switch to IDE Mode', icon: <TerminalSquare className="h-4 w-4" />, execute: () => setGlobalMode('ide') },
    { id: 'multiplex-mode', label: 'Switch to Multiplex Mode', icon: <TerminalSquare className="h-4 w-4" />, execute: () => setGlobalMode('multiplex') }
  ]

  if (!bridgeAvailable) {
    return <BridgeUnavailableScreen />
  }

  return (
    <div className="flex h-[100dvh] w-screen flex-col overflow-hidden bg-[#060a0f] text-white select-none">
      {errorMessage && (
        <div className="shrink-0 bg-rose-500/10 px-4 py-2 text-sm text-rose-200 border-b border-rose-500/20">
          {errorMessage}
          <button className="ml-3 underline opacity-70 hover:opacity-100" onClick={() => setErrorMessage(null)}>dismiss</button>
        </div>
      )}

      <AppHeader
        activeWorkspaceId={activeWorkspaceId}
        globalMode={globalMode}
        hasProject={hasProject}
        onCreateSession={() => { void handleCreateSession() }}
        onCreateStandaloneTerminal={() => { void handleCreateStandaloneTerminal() }}
        onOpenProject={() => { void handleOpenProject() }}
        onSwitchWorkspace={(workspaceId) => { void handleSwitchWorkspace(workspaceId) }}
        onToggleSidebar={toggleSidebar}
        onWorkspaceAction={(workspaceId, action) => { void handleWorkspaceAction(workspaceId, action) }}
        project={project}
        workspaces={workspaces}
      />

      <AppWorkspacePanels
        activeStandaloneTab={activeStandaloneTab}
        activeTabId={activeTabId}
        activeWorkspaceId={activeWorkspaceId}
        consoleOpen={consoleOpen}
        defaultSessionStrategy={defaultSessionStrategy}
        diffBadges={diffBadges}
        fitNonce={fitNonce}
        globalMode={globalMode}
        ideContent={ideContent}
        ideTabIds={ideTabIds}
        keepIdeMounted={keepIdeMounted}
        multiplexContent={multiplexContent}
        onChangeDefaultSessionStrategy={(strategy) => { void handleChangeDefaultSessionStrategy(strategy) }}
        onFileSelect={(file) => {
          setSelectedFile(file)
          setGlobalMode('ide')
        }}
        onOpenProject={() => { void handleOpenProject() }}
        onRefreshProject={() => { void handleRefreshProject() }}
        onTabClose={(tabId) => { void handleCloseTab(tabId) }}
        onTabSelect={setActiveTabId}
        onToggleConsole={() => setConsoleOpen((value) => !value)}
        onToggleGlobalMode={setGlobalMode}
        onToggleSidebar={toggleSidebar}
        onToggleStatusBarCollapse={() => setStatusBarCollapsed((value) => !value)}
        onWorkspaceAction={(workspaceId, action) => { void handleWorkspaceAction(workspaceId, action) }}
        overlayFiles={overlayFiles}
        project={project}
        refreshingProject={refreshingProject}
        selectedFile={selectedFile}
        shellViewportRef={shellViewportRef}
        sidebarCollapsed={sidebarCollapsed}
        sidebarPanelRef={sidebarPanelRef}
        statusBarCollapsed={statusBarCollapsed}
        summary={workspaceSummary}
        visibleTabs={visibleTabs}
        windowsBuildNumber={windowsBuildNumber}
      />

      <div
        className={`fixed inset-x-0 bottom-0 z-40 flex h-[36vh] flex-col overflow-hidden bg-[#060c14]/98 shadow-2xl backdrop-blur-2xl transition-transform duration-300 ease-in-out ${
          consoleOpen ? 'translate-y-0 border-t border-sentinel-accent/20' : 'translate-y-full'
        }`}
      >
        <ConsoleDrawer
          entries={activityLog}
          onToggleOpen={() => setConsoleOpen((value) => !value)}
          open={consoleOpen}
        />
      </div>

      <GlobalActionBar
        actions={globalActions}
        isOpen={globalActionBarOpen}
        onClose={() => setGlobalActionBarOpen(false)}
      />
    </div>
  )
}
