import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-dialog'
import { toError } from './error-utils'

import type {
  ActivityLogEntry,
  BootstrapPayload,
  CreateSessionInput,
  IdeTerminalOutputEvent,
  IdeTerminalState,
  ProjectState,
  SessionApplyResult,
  SessionCommitResult,
  SessionDiffUpdate,
  SessionHistoryUpdate,
  SessionMetricsUpdate,
  SentinelApi,
  SessionOutputEvent,
  SessionSummary,
  SessionWorkspaceStrategy,
  TabMetricsUpdate,
  TabOutputEvent,
  TabStateUpdate,
  TabSummary,
  WorkspacePreferences,
  WorkspaceSummary
} from '@shared/types'

let lastProject: ProjectState = {
  isGitRepo: false,
  tree: []
}

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

  void listen<T>(eventName, (event) => {
    if (!disposed) {
      listener(event.payload)
    }
  }).then((fn) => {
    if (disposed) {
      fn()
      return
    }
    unlisten = fn
  }).catch(() => {
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

const api: SentinelApi = {
  async bootstrap() {
    const payload = await invokeCommand<BootstrapPayload>('bootstrap')
    rememberProject(payload.project)
    return payload
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
  async refreshProject() {
    return rememberProject(await invokeCommand<ProjectState>('refresh_project'))
  },
  setDefaultSessionStrategy(strategy: SessionWorkspaceStrategy) {
    return invokeCommand<WorkspacePreferences>('set_default_session_strategy', { strategy })
  },
  createSession(input?: CreateSessionInput) {
    return invokeCommand<SessionSummary>('create_session', { input })
  },
  closeSession(sessionId: string) {
    return invokeCommand<void>('close_session', { sessionId })
  },
  resizeSession(sessionId: string, cols: number, rows: number) {
    return invokeCommand<void>('resize_session', { sessionId, cols, rows })
  },
  sendInput(sessionId: string, data: string) {
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
    return subscribe<SessionOutputEvent>('sentinel:session-output', listener)
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
    return subscribe<SessionSummary>('sentinel:session-state', listener)
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
  onActivityLog(listener: (entry: ActivityLogEntry) => void) {
    return subscribe<ActivityLogEntry>('sentinel:activity-log', listener)
  },
  createStandaloneTerminal(cols: number, rows: number) {
    return invokeCommand<TabSummary>('create_standalone_terminal', { cols, rows })
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
