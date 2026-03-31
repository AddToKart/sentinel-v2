import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-dialog'
import { toError } from './error-utils'
import { CloudClient } from './cloud-client'

import type {
  ActivityLogEntry,
  CommandHistoryEntry,
  BootstrapPayload,
  CreateSessionInput,
  FileChangeEntry,
  IdeTerminalOutputEvent,
  IdeTerminalState,
  ProjectState,
  SnapshotSummary,
  SessionApplyResult,
  SessionCommitResult,
  SessionDiffUpdate,
  SessionHistoryUpdate,
  SessionMetricsUpdate,
  SentinelApi,
  SessionOutputEvent,
  SessionSummary,
  SessionWorkspaceStrategy,
  WorkspaceAnalytics,
  TabMetricsUpdate,
  TabOutputEvent,
  TabStateUpdate,
  TabSummary,
  WorkspaceContext,
  WorkspacePreferences,
  WorkspaceRemovedEvent,
  WorkspaceSummary
} from '@shared/types'

let lastProject: ProjectState = {
  isGitRepo: false,
  tree: []
}

let activeCloudClient: CloudClient | null = null
const sessionOutputListeners = new Set<(event: SessionOutputEvent) => void>()
const sessionStateListeners = new Set<(session: SessionSummary) => void>()
const trackedCloudSessions = new Set<string>()
const attachedCloudSessions = new Set<string>()
const cloudSessionOutputSeqs = new Map<string, number>()
const cloudSessionAttachPromises = new Map<string, Promise<void>>()

function hasTauriRuntime(): boolean {
  return typeof window !== 'undefined' && typeof (window as any).__TAURI_INTERNALS__ !== 'undefined'
}

function desktopBridgeUnavailableError(): Error {
  return new Error('Sentinel desktop bridge is unavailable.')
}

function ensureTauriRuntime(): void {
  if (!hasTauriRuntime()) {
    throw desktopBridgeUnavailableError()
  }
}

async function invokeCommand<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  ensureTauriRuntime()

  try {
    return await invoke<T>(command, args)
  } catch (error) {
    console.error(`[sentinel] command failed: ${command}`, { args, error })
    throw toError(error)
  }
}

function rememberProject(project: ProjectState): ProjectState {
  lastProject = project
  return project
}

function subscribe<T>(eventName: string, listener: (payload: T) => void): () => void {
  if (!hasTauriRuntime()) {
    return () => {}
  }

  let unlisten: (() => void) | null = null
  let disposed = false

  listen<T>(eventName, (event) => {
    if (!disposed) {
      try {
        listener(event.payload)
      } catch (error) {
        console.error(`[sentinel] listener error for ${eventName}`, { error })
      }
    }
  }).then((fn) => {
    if (disposed) {
      fn()
      return
    }
    unlisten = fn
  }).catch((error) => {
    console.error(`[sentinel] event subscription failed for ${eventName}`, { error })
    unlisten = null
  })

  return () => {
    disposed = true
    if (unlisten) {
      unlisten()
      unlisten = null
    }
  }
}

// Global caches for routing
const sessionModes = new Map<string, 'local' | 'cloud'>()
const workspaceModes = new Map<string, 'local' | 'cloud'>()

function untrackCloudSession(sessionId: string) {
  trackedCloudSessions.delete(sessionId)
  attachedCloudSessions.delete(sessionId)
  cloudSessionOutputSeqs.delete(sessionId)
  cloudSessionAttachPromises.delete(sessionId)
}

function trackSession(session: SessionSummary) {
  sessionModes.set(session.id, session.mode)
  if (session.mode === 'cloud' && session.status !== 'closed' && session.status !== 'error') {
    trackedCloudSessions.add(session.id)
    return
  }

  untrackCloudSession(session.id)
}

function trackWorkspace(workspace: WorkspaceContext) {
  workspaceModes.set(workspace.id, workspace.mode)
}

async function ensureCloudClientConnected(): Promise<CloudClient> {
  if (!activeCloudClient) {
    throw new Error('Sentinel Cloud is not configured yet. Reopen this project as Local or configure the cloud backend first.')
  }

  await activeCloudClient.connect()
  return activeCloudClient
}

async function ensureCloudSessionAttached(
  sessionId: string,
  options: { force?: boolean } = {}
): Promise<void> {
  if (sessionModes.get(sessionId) !== 'cloud') {
    return
  }

  if (!activeCloudClient) {
    return
  }

  if (!options.force && attachedCloudSessions.has(sessionId)) {
    return
  }

  const existingPromise = cloudSessionAttachPromises.get(sessionId)
  if (existingPromise) {
    return existingPromise
  }

  const attachPromise = (async () => {
    const client = await ensureCloudClientConnected()
    const afterSeq = cloudSessionOutputSeqs.get(sessionId) ?? 0
    const session = await client.attachSession(sessionId, afterSeq)
    trackSession(session)
    attachedCloudSessions.add(sessionId)
  })().finally(() => {
    cloudSessionAttachPromises.delete(sessionId)
  })

  cloudSessionAttachPromises.set(sessionId, attachPromise)
  return attachPromise
}

function reattachTrackedCloudSessions(): void {
  attachedCloudSessions.clear()
  for (const sessionId of trackedCloudSessions) {
    void ensureCloudSessionAttached(sessionId, { force: true }).catch((error) => {
      console.warn('[sentinel-cloud] session attach failed', { sessionId, error })
    })
  }
}

const api: SentinelApi = {
  async bootstrap() {
    const payload = await invokeCommand<BootstrapPayload>('bootstrap')
    rememberProject(payload.project)
    
    // Track modes for routing
    payload.workspaces.forEach(trackWorkspace)
    payload.sessions.forEach(trackSession)

    // Initialize cloud client if configured
    if (payload.cloudConfig?.enabled && payload.cloudConfig.url) {
      if (!activeCloudClient) {
        activeCloudClient = new CloudClient({
          url: payload.cloudConfig.url,
          authToken: payload.preferences.cloudToken || '',
          onOpen: () => {
            reattachTrackedCloudSessions()
          },
          onClose: () => {
            attachedCloudSessions.clear()
          },
          onMessage: (msg) => {
            if (msg.type === 'session.output') {
              if (typeof msg.seq === 'number') {
                cloudSessionOutputSeqs.set(msg.sessionId, msg.seq)
              }
              const event: SessionOutputEvent = {
                sessionId: msg.sessionId,
                data: msg.data
              }
              sessionOutputListeners.forEach((l) => l(event))
            } else if (msg.type === 'session.replay' && Array.isArray(msg.chunks)) {
              for (const chunk of msg.chunks) {
                if (!chunk || typeof chunk.data !== 'string') {
                  continue
                }

                if (typeof chunk.seq === 'number') {
                  const lastSeq = cloudSessionOutputSeqs.get(msg.sessionId) ?? 0
                  if (chunk.seq <= lastSeq) {
                    continue
                  }
                  cloudSessionOutputSeqs.set(msg.sessionId, chunk.seq)
                }

                const event: SessionOutputEvent = {
                  sessionId: msg.sessionId,
                  data: chunk.data
                }
                sessionOutputListeners.forEach((l) => l(event))
              }
            } else if (msg.type === 'session.state') {
              trackSession(msg.session)
              if (msg.session.status === 'closed' || msg.session.status === 'error') {
                attachedCloudSessions.delete(msg.session.id)
              }
              sessionStateListeners.forEach((l) => l(msg.session))
            }
          }
        })
      }
      void activeCloudClient.connect().catch((error) => {
        console.warn('[sentinel-cloud] bootstrap connection failed', { error })
      })

      for (const session of payload.sessions) {
        if (session.mode !== 'cloud') {
          continue
        }

        void ensureCloudSessionAttached(session.id).catch((error) => {
          console.warn('[sentinel-cloud] initial session attach failed', {
            sessionId: session.id,
            error
          })
        })
      }
    }

    return payload
  },

  async pickProjectDirectory() {
    ensureTauriRuntime()
    let selected: string | string[] | null
    try {
      selected = await open({
        directory: true,
        multiple: false,
        defaultPath: lastProject.path
      })
    } catch (error) {
      console.error('[sentinel] project picker failed', { error })
      throw toError(error)
    }

    if (!selected || Array.isArray(selected)) {
      return null
    }

    return selected
  },

  async selectProject() {
    ensureTauriRuntime()
    let selected: string | string[] | null
    try {
      selected = await open({
        directory: true,
        multiple: false,
        defaultPath: lastProject.path
      })
    } catch (error) {
      console.error('[sentinel] project picker failed', { error })
      throw toError(error)
    }

    if (!selected || Array.isArray(selected)) {
      return lastProject
    }

    return rememberProject(
      await invokeCommand<ProjectState>('load_project', { candidatePath: selected })
    )
  },

  async createWorkspace(candidatePath: string, name?: string, mode?: 'local' | 'cloud') {
    const ws = await invokeCommand<WorkspaceContext>('create_workspace', {
      candidatePath,
      name,
      mode
    })
    trackWorkspace(ws)
    return ws
  },

  async listWorkspaces() {
    const list = await invokeCommand<WorkspaceContext[]>('list_workspaces')
    list.forEach(trackWorkspace)
    return list
  },

  async switchWorkspace(workspaceId: string) {
    const ws = await invokeCommand<WorkspaceContext>('switch_workspace', { workspaceId })
    trackWorkspace(ws)
    return ws
  },

  closeWorkspace(workspaceId: string, closeSessions: boolean) {
    return invokeCommand<void>('close_workspace', { workspaceId, closeSessions })
  },

  stopWorkspace(workspaceId: string) {
    return invokeCommand<void>('stop_workspace', { workspaceId })
  },

  pauseWorkspace(workspaceId: string) {
    return invokeCommand<void>('pause_workspace', { workspaceId })
  },

  getActiveWorkspace() {
    return invokeCommand<WorkspaceContext | null>('get_active_workspace')
  },

  async refreshProject() {
    return rememberProject(await invokeCommand<ProjectState>('refresh_project'))
  },

  setDefaultSessionStrategy(strategy: SessionWorkspaceStrategy) {
    return invokeCommand<WorkspacePreferences>('set_default_session_strategy', { strategy })
  },

  async createSession(input?: CreateSessionInput) {
    const activeWs = await this.getActiveWorkspace()
    if (activeWs?.mode === 'cloud') {
      const client = await ensureCloudClientConnected()
      
      // Ensure the cloud backend knows about this workspace first
      await client.ensureWorkspace({
        id: activeWs.id,
        name: activeWs.name,
        primaryCheckoutPath: activeWs.project.path || '',
        repoUrl: activeWs.repoUrl,
        defaultBranch: activeWs.project.branch
      })

      // Inject the current workspace ID for the cloud backend
      const cloudInput = { ...input, workspaceId: activeWs.id }
      const session = await client.createSession(cloudInput as any)
      trackSession(session)
      await ensureCloudSessionAttached(session.id)
      return session
    }
    const session = await invokeCommand<SessionSummary>('create_session', { input })
    trackSession(session)
    return session
  },

  async closeSession(sessionId: string) {
    if (sessionModes.get(sessionId) === 'cloud') {
      const client = await ensureCloudClientConnected()
      return client.closeSession(sessionId)
    }
    return invokeCommand<void>('close_session', { sessionId })
  },

  pauseSession(sessionId: string) {
    return invokeCommand<void>('pause_session', { sessionId })
  },

  resumeSession(sessionId: string) {
    return invokeCommand<SessionSummary>('resume_session', { sessionId })
  },

  deleteSession(sessionId: string) {
    return invokeCommand<void>('delete_session', { sessionId })
  },

  async resizeSession(sessionId: string, cols: number, rows: number) {
    if (sessionModes.get(sessionId) === 'cloud') {
      const client = await ensureCloudClientConnected()
      return client.resizeSession(sessionId, cols, rows)
    }
    return invokeCommand<void>('resize_session', { sessionId, cols, rows })
  },

  async sendInput(sessionId: string, data: string) {
    if (sessionModes.get(sessionId) === 'cloud') {
      const client = await ensureCloudClientConnected()
      return client.sendInput(sessionId, data)
    }
    return invokeCommand<void>('send_input', { sessionId, data })
  },

  ensureIdeTerminal() {
    return invokeCommand<IdeTerminalState>('ensure_ide_terminal')
  },

  resizeIdeTerminal(cols: number, rows: number) {
    return invokeCommand<void>('resize_ide_terminal', { cols, rows })
  },

  sendIdeTerminalInput(data: string) {
    return invokeCommand<void>('send_ide_terminal_input', { data })
  },

  writeIdeFile(relativePath: string, content: string) {
    return invokeCommand<void>('write_ide_file', { relativePath, content })
  },

  applyIdeWorkspace() {
    return invokeCommand('apply_ide_workspace')
  },

  discardIdeWorkspaceChanges() {
    return invokeCommand<void>('discard_ide_workspace_changes')
  },

  readFile(filePath: string) {
    return invokeCommand<string>('read_file', { filePath })
  },

  readFileDiff(sessionId: string, filePath: string) {
    return invokeCommand<string>('read_file_diff', { sessionId, filePath })
  },

  writeSessionFile(sessionId: string, relativePath: string, content: string) {
    return invokeCommand<void>('write_session_file', {
      sessionId,
      relativePath,
      content
    })
  },

  applySession(sessionId: string) {
    return invokeCommand<SessionApplyResult>('apply_session', { sessionId })
  },

  commitSession(sessionId: string, message: string) {
    return invokeCommand<SessionCommitResult>('commit_session', { sessionId, message })
  },

  discardSessionChanges(sessionId: string) {
    return invokeCommand<void>('discard_session_changes', { sessionId })
  },

  revealInFileExplorer(filePath: string) {
    return invokeCommand<void>('reveal_in_file_explorer', { filePath })
  },

  openInSystemEditor(filePath: string) {
    return invokeCommand<void>('open_in_system_editor', { filePath })
  },

  onSessionOutput(listener: (event: SessionOutputEvent) => void) {
    sessionOutputListeners.add(listener)
    const localUnsubscribe = subscribe<SessionOutputEvent>('sentinel:session-output', listener)
    return () => {
      sessionOutputListeners.delete(listener)
      localUnsubscribe()
    }
  },

  onProjectState(listener: (project: ProjectState) => void) {
    return subscribe<ProjectState>('sentinel:project-state', (project) => {
      listener(rememberProject(project))
    })
  },

  onIdeTerminalOutput(listener: (event: IdeTerminalOutputEvent) => void) {
    return subscribe<IdeTerminalOutputEvent>('sentinel:ide-terminal-output', listener)
  },

  onSessionState(listener: (session: SessionSummary) => void) {
    sessionStateListeners.add(listener)
    const localUnsubscribe = subscribe<SessionSummary>('sentinel:session-state', (s) => {
      trackSession(s)
      listener(s)
    })
    return () => {
      sessionStateListeners.delete(listener)
      localUnsubscribe()
    }
  },

  onIdeTerminalState(listener: (state: IdeTerminalState) => void) {
    return subscribe<IdeTerminalState>('sentinel:ide-terminal-state', listener)
  },

  onSessionMetrics(listener: (payload: SessionMetricsUpdate) => void) {
    return subscribe<SessionMetricsUpdate>('sentinel:session-metrics', listener)
  },

  onSessionHistory(listener: (payload: SessionHistoryUpdate) => void) {
    return subscribe<SessionHistoryUpdate>('sentinel:session-history', listener)
  },

  onSessionDiff(listener: (payload: SessionDiffUpdate) => void) {
    return subscribe<SessionDiffUpdate>('sentinel:session-diff', listener)
  },

  onWorkspaceState(listener: (summary: WorkspaceSummary) => void) {
    return subscribe<WorkspaceSummary>('sentinel:workspace-state', listener)
  },

  onWorkspaceCreated(listener: (workspace: WorkspaceContext) => void) {
    return subscribe<WorkspaceContext>('sentinel:workspace-created', (workspace) => {
      trackWorkspace(workspace)
      listener(workspace)
    })
  },

  onWorkspaceUpdated(listener: (workspace: WorkspaceContext) => void) {
    return subscribe<WorkspaceContext>('sentinel:workspace-updated', (workspace) => {
      trackWorkspace(workspace)
      listener(workspace)
    })
  },

  onWorkspaceSwitched(listener: (workspace: WorkspaceContext) => void) {
    return subscribe<WorkspaceContext>('sentinel:workspace-switched', (workspace) => {
      trackWorkspace(workspace)
      listener(workspace)
      rememberProject(workspace.project)
    })
  },

  onWorkspaceRemoved(listener: (payload: WorkspaceRemovedEvent) => void) {
    return subscribe<WorkspaceRemovedEvent>('sentinel:workspace-removed', (payload) => {
      workspaceModes.delete(payload.workspaceId)
      listener(payload)
    })
  },

  onActivityLog(listener: (entry: ActivityLogEntry) => void) {
    return subscribe<ActivityLogEntry>('sentinel:activity-log', listener)
  },

  createStandaloneTerminal(cwd: string | undefined, label: string | undefined, cols: number, rows: number) {
    return invokeCommand<TabSummary>('create_standalone_terminal', { cwd, label, cols, rows })
  },

  closeTab(tabId: string) {
    return invokeCommand<void>('close_tab', { tabId })
  },

  resizeTab(tabId: string, cols: number, rows: number) {
    return invokeCommand<void>('resize_tab', { tabId, cols, rows })
  },

  sendTabInput(tabId: string, data: string) {
    return invokeCommand<void>('send_tab_input', { tabId, data })
  },

  searchCommandHistory(workspaceId: string, query: string, limit?: number) {
    return invokeCommand<CommandHistoryEntry[]>('search_command_history', {
      workspaceId,
      query,
      limit
    })
  },

  getFileChangeTimeline(workspaceId: string, filePath?: string, limit?: number) {
    return invokeCommand<FileChangeEntry[]>('get_file_change_timeline', {
      workspaceId,
      filePath,
      limit
    })
  },

  getWorkspaceAnalytics(workspaceId: string) {
    return invokeCommand<WorkspaceAnalytics>('get_workspace_analytics', { workspaceId })
  },

  exportAuditLog(
    workspaceId: string,
    startTimestamp?: number,
    endTimestamp?: number,
    format?: 'json' | 'csv'
  ) {
    return invokeCommand<string>('export_audit_log', {
      workspaceId,
      startTimestamp,
      endTimestamp,
      format
    })
  },

  createWorkspaceSnapshot(workspaceId: string, name: string, description?: string) {
    return invokeCommand<SnapshotSummary>('create_workspace_snapshot', {
      workspaceId,
      name,
      description
    })
  },

  restoreWorkspaceSnapshot(snapshotId: string) {
    return invokeCommand<WorkspaceContext>('restore_workspace_snapshot', { snapshotId })
  },

  listWorkspaceSnapshots(workspaceId: string) {
    return invokeCommand<SnapshotSummary[]>('list_workspace_snapshots', { workspaceId })
  },

  onTabOutput(listener: (event: TabOutputEvent) => void) {
    return subscribe<TabOutputEvent>('sentinel:tab-output', listener)
  },

  onTabState(listener: (payload: TabStateUpdate) => void) {
    return subscribe<TabStateUpdate>('sentinel:tab-state', listener)
  },

  onTabMetrics(listener: (payload: TabMetricsUpdate) => void) {
    return subscribe<TabMetricsUpdate>('sentinel:tab-metrics', listener)
  }
}

if (hasTauriRuntime()) {
  window.sentinel = api
}

export { api as tauriSentinel }
