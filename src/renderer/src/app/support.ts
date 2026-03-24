import type {
  IdeTerminalState,
  ProjectState,
  SentinelApi,
  WorkspaceContext,
  WorkspaceSummary
} from '@shared/types'

export type WorkspaceAction = 'delete' | 'pause' | 'stop'

export function emptyProject(): ProjectState {
  return {
    isGitRepo: false,
    tree: [],
    name: undefined,
    path: undefined,
    branch: undefined
  }
}

export function defaultSummary(): WorkspaceSummary {
  return {
    activeSessions: 0,
    activeWorkspaceSessionCount: 0,
    activeWorkspaceTabCount: 0,
    workspaceCount: 0,
    totalSessions: 0,
    totalTabs: 0,
    totalCpuPercent: 0,
    totalMemoryMb: 0,
    totalProcesses: 0,
    lastUpdated: Date.now(),
    defaultSessionStrategy: 'sandbox-copy'
  }
}

export function defaultIdeTerminalState(): IdeTerminalState {
  return {
    status: 'idle',
    shell: 'powershell.exe',
    modifiedPaths: []
  }
}

export function getSentinelBridge(): SentinelApi | null {
  return typeof window !== 'undefined' && typeof window.sentinel !== 'undefined'
    ? window.sentinel
    : null
}

export function missingBridgeMessage(): string {
  return 'Sentinel desktop bridge is unavailable. Run this UI through the Tauri app, not a plain browser tab.'
}

export function sortWorkspaces(workspaces: WorkspaceContext[]): WorkspaceContext[] {
  return [...workspaces].sort((left, right) => {
    if (left.lastActiveAt !== right.lastActiveAt) {
      return right.lastActiveAt - left.lastActiveAt
    }

    return left.name.localeCompare(right.name)
  })
}

export function upsertWorkspace(
  current: WorkspaceContext[],
  workspace: WorkspaceContext
): WorkspaceContext[] {
  const next = current.some((existing) => existing.id === workspace.id)
    ? current.map((existing) => (existing.id === workspace.id ? workspace : existing))
    : [...current, workspace]

  return sortWorkspaces(next)
}
