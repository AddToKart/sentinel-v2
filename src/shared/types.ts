export type WorkspaceMode = 'local' | 'cloud'

export type SessionStatus = 'starting' | 'ready' | 'closing' | 'paused' | 'closed' | 'error'

export type CleanupState = 'active' | 'removed' | 'preserved' | 'failed'

export type SessionWorkspaceStrategy = 'sandbox-copy' | 'git-worktree'

export type TabType = 'dashboard' | 'terminal'

export type TabStatus = 'starting' | 'ready' | 'closing' | 'closed' | 'error'

export interface ProcessMetrics {
  cpuPercent: number
  memoryMb: number
  threadCount: number
  handleCount: number
  processCount: number
}

export interface SessionMetricsUpdate {
  sessionId: string
  workspaceId: string
  pid?: number
  processIds: number[]
  metrics: ProcessMetrics
  sampledAt: number
}

export interface ProjectNode {
  name: string
  path: string
  kind: 'file' | 'directory'
  children?: ProjectNode[]
}

export interface ProjectState {
  path?: string
  name?: string
  branch?: string
  isGitRepo: boolean
  tree: ProjectNode[]
}

export interface SessionSummary {
  id: string
  workspaceId: string
  label: string
  projectRoot: string
  cwd: string
  workspacePath: string
  workspaceStrategy: SessionWorkspaceStrategy
  branchName?: string
  status: SessionStatus
  cleanupState: CleanupState
  shell: string
  pid?: number
  createdAt: number
  startupCommand?: string
  exitCode?: number | null
  error?: string
  metrics: ProcessMetrics
  mode: WorkspaceMode
}

export interface TabSummary {
  id: string
  workspaceId: string
  tabType: TabType
  label: string
  status: TabStatus
  cwd: string
  shell: string
  pid?: number
  createdAt: number
  exitCode?: number | null
  error?: string
  metrics: ProcessMetrics
}

export interface TabMetricsUpdate {
  tabId: string
  workspaceId: string
  pid?: number
  processIds: number[]
  metrics: ProcessMetrics
  sampledAt: number
}

export interface TabOutputEvent {
  tabId: string
  data: string
}

export interface TabStateUpdate {
  tabId: string
  workspaceId: string
  status: TabStatus
  pid?: number
  exitCode?: number | null
  error?: string
}

export interface WorkspaceContext {
  id: string
  name: string
  project: ProjectState
  repoUrl?: string
  sessionIds: string[]
  tabIds: string[]
  createdAt: number
  lastActiveAt: number
  defaultSessionStrategy: SessionWorkspaceStrategy
  mode: WorkspaceMode
}

export interface WorkspaceRemovedEvent {
  workspaceId: string
}

export interface WorkspaceSummary {
  activeSessions: number
  activeWorkspaceSessionCount: number
  activeWorkspaceTabCount: number
  workspaceCount: number
  totalSessions: number
  totalTabs: number
  totalCpuPercent: number
  totalMemoryMb: number
  totalProcesses: number
  lastUpdated: number
  defaultSessionStrategy: SessionWorkspaceStrategy
  activeWorkspaceId?: string
  activeWorkspaceName?: string
  projectPath?: string
  projectName?: string
  branch?: string
}

export interface ActivityLogEntry {
  id: string
  workspaceId?: string
  timestamp: number
  scope: 'git' | 'workspace'
  status: 'started' | 'completed' | 'failed'
  command: string
  cwd: string
  detail?: string
}

export interface CommandHistoryEntry {
  id: number
  sessionId: string
  workspaceId: string
  command: string
  timestamp: number
  source: string
  exitCode?: number | null
  durationMs?: number
  cwd?: string
}

export interface FileChangeEntry {
  id: number
  sessionId: string
  workspaceId: string
  filePath: string
  changeType: string
  beforeHash?: string
  afterHash?: string
  timestamp: number
  fileSize?: number
}

export interface AuditLogEntry {
  id: number
  workspaceId?: string
  sessionId?: string
  tabId?: string
  timestamp: number
  actionType: string
  resourceType: string
  resourceId: string
  details?: string
  userId?: string
}

export interface WorkspaceAnalytics {
  workspaceId: string
  totalSessions: number
  activeSessions: number
  totalTabs: number
  activeTabs: number
  totalCommands: number
  totalFileChanges: number
  uniqueFilesChanged: number
  totalActivityEntries: number
  totalSnapshots: number
  averageSessionCpuPercent: number
  averageSessionMemoryMb: number
  latestActivityAt?: number
  latestSnapshotAt?: number
}

export interface SnapshotSummary {
  id: string
  workspaceId: string
  name: string
  description?: string
  createdAt: number
  fileCount: number
  sessionCount: number
}

export interface SessionCommandEntry {
  id: string
  command: string
  timestamp: number
  source: 'interactive' | 'startup'
}

export interface SessionHistoryUpdate {
  sessionId: string
  workspaceId: string
  entries: SessionCommandEntry[]
}

export interface SessionDiffUpdate {
  sessionId: string
  workspaceId: string
  modifiedPaths: string[]
  updatedAt: number
}

export interface SessionSyncConflict {
  path: string
  reason: 'project-changed' | 'project-path-blocked'
  detail?: string
}

export interface SessionApplyResult {
  sessionId: string
  workspaceStrategy: SessionWorkspaceStrategy
  appliedPaths: string[]
  remainingPaths: string[]
  conflicts: SessionSyncConflict[]
}

export interface SessionCommitResult {
  sessionId: string
  workspaceStrategy: SessionWorkspaceStrategy
  appliedPaths: string[]
  committedPaths: string[]
  remainingPaths: string[]
  conflicts: SessionSyncConflict[]
  createdCommit: boolean
  commitMessage: string
  commitHash?: string
}

export interface WorkspacePreferences {
  defaultSessionStrategy: SessionWorkspaceStrategy
  lastWorkspaceId?: string
  cloudToken?: string
}

export interface IdeTerminalState {
  status: 'idle' | SessionStatus
  cwd?: string
  workspacePath?: string
  shell: string
  pid?: number
  createdAt?: number
  exitCode?: number | null
  error?: string
  modifiedPaths: string[]
}

export interface BootstrapPayload {
  project: ProjectState
  workspaces: WorkspaceContext[]
  activeWorkspaceId?: string
  sessions: SessionSummary[]
  tabs: TabSummary[]
  summary: WorkspaceSummary
  activityLog: ActivityLogEntry[]
  metrics: SessionMetricsUpdate[]
  tabMetrics: TabMetricsUpdate[]
  histories: SessionHistoryUpdate[]
  diffs: SessionDiffUpdate[]
  preferences: WorkspacePreferences
  ideTerminal: IdeTerminalState
  windowsBuildNumber?: number
  cloudConfig?: {
    url: string
    enabled: boolean
  }
}

export interface CreateSessionInput {
  label?: string
  startupCommand?: string
  cols?: number
  rows?: number
  workspaceStrategy?: SessionWorkspaceStrategy
}

export interface AgentFileChange {
  id: string
  workspaceId: string
  agentId: string
  sandboxId: string
  filePath: string
  operation: 'created' | 'modified' | 'deleted' | 'renamed'
  diffContent?: string
  additions: number
  deletions: number
  timestamp: number
  unifiedStatus: 'pending' | 'merged' | 'conflicted' | 'pushed' | 'discarded'
  fileSize?: number
  isBinary: boolean
}

export interface UnifiedSandboxEntry {
  id: string
  workspaceId: string
  filePath: string
  sourceAgentId: string
  conflictAgentIds?: string[]
  status: 'clean' | 'conflicted' | 'pushed'
  lastUpdatedAt: number
}

export interface ChangesManagerState {
  agentChanges: AgentFileChange[]
  unifiedEntries: UnifiedSandboxEntry[]
  totalChangedFiles: number
  conflictCount: number
  pendingPushCount: number
}

export interface ChangesUpdatedEvent {
  workspaceId: string
  agentId?: string
  action?: string
  changeCount?: number
  paths?: string[]
}

export interface UnifiedSandboxUpdatedEvent {
  workspaceId: string
  entryCount: number
}

export interface SessionOutputEvent {
  sessionId: string
  data: string
}

export interface IdeTerminalOutputEvent {
  data: string
}

export interface SentinelApi {
  bootstrap: () => Promise<BootstrapPayload>
  pickProjectDirectory: () => Promise<string | null>
  selectProject: () => Promise<ProjectState>
  createWorkspace: (candidatePath: string, name?: string, mode?: WorkspaceMode) => Promise<WorkspaceContext>
  listWorkspaces: () => Promise<WorkspaceContext[]>
  switchWorkspace: (workspaceId: string) => Promise<WorkspaceContext>
  closeWorkspace: (workspaceId: string, closeSessions: boolean) => Promise<void>
  stopWorkspace: (workspaceId: string) => Promise<void>
  pauseWorkspace: (workspaceId: string) => Promise<void>
  getActiveWorkspace: () => Promise<WorkspaceContext | null>
  refreshProject: () => Promise<ProjectState>
  setDefaultSessionStrategy: (strategy: SessionWorkspaceStrategy) => Promise<WorkspacePreferences>
  createSession: (input?: CreateSessionInput) => Promise<SessionSummary>
  closeSession: (sessionId: string) => Promise<void>
  pauseSession: (sessionId: string) => Promise<void>
  resumeSession: (sessionId: string) => Promise<SessionSummary>
  deleteSession: (sessionId: string) => Promise<void>
  resizeSession: (sessionId: string, cols: number, rows: number) => Promise<void>
  sendInput: (sessionId: string, data: string) => Promise<void>
  ensureIdeTerminal: () => Promise<IdeTerminalState>
  resizeIdeTerminal: (cols: number, rows: number) => Promise<void>
  sendIdeTerminalInput: (data: string) => Promise<void>
  writeIdeFile: (relativePath: string, content: string) => Promise<void>
  applyIdeWorkspace: () => Promise<SessionApplyResult>
  discardIdeWorkspaceChanges: () => Promise<void>
  readFile: (filePath: string) => Promise<string>
  readFileDiff: (sessionId: string, filePath: string) => Promise<string>
  writeSessionFile: (sessionId: string, relativePath: string, content: string) => Promise<void>
  applySession: (sessionId: string) => Promise<SessionApplyResult>
  commitSession: (sessionId: string, message: string) => Promise<SessionCommitResult>
  discardSessionChanges: (sessionId: string) => Promise<void>
  revealInFileExplorer: (filePath: string) => Promise<void>
  openInSystemEditor: (filePath: string) => Promise<void>
  createStandaloneTerminal: (cwd: string | undefined, label: string | undefined, cols: number, rows: number) => Promise<TabSummary>
  closeTab: (tabId: string) => Promise<void>
  resizeTab: (tabId: string, cols: number, rows: number) => Promise<void>
  sendTabInput: (tabId: string, data: string) => Promise<void>
  searchCommandHistory: (workspaceId: string, query: string, limit?: number) => Promise<CommandHistoryEntry[]>
  getFileChangeTimeline: (workspaceId: string, filePath?: string, limit?: number) => Promise<FileChangeEntry[]>
  getWorkspaceAnalytics: (workspaceId: string) => Promise<WorkspaceAnalytics>
  exportAuditLog: (workspaceId: string, startTimestamp?: number, endTimestamp?: number, format?: 'json' | 'csv') => Promise<string>
  createWorkspaceSnapshot: (workspaceId: string, name: string, description?: string) => Promise<SnapshotSummary>
  restoreWorkspaceSnapshot: (snapshotId: string) => Promise<WorkspaceContext>
  listWorkspaceSnapshots: (workspaceId: string) => Promise<SnapshotSummary[]>
  getChangesManagerState: (workspaceId: string) => Promise<ChangesManagerState>
  scanAgentChanges: (workspaceId: string, agentId: string) => Promise<void>
  pushUnifiedSandbox: (workspaceId: string) => Promise<string[]>
  discardChanges: (workspaceId: string, agentId?: string) => Promise<void>
  resolveFileConflict: (workspaceId: string, filePath: string, winningAgentId: string) => Promise<void>
  onSessionOutput: (listener: (event: SessionOutputEvent) => void) => () => void
  onIdeTerminalOutput: (listener: (event: IdeTerminalOutputEvent) => void) => () => void
  onProjectState: (listener: (project: ProjectState) => void) => () => void
  onSessionState: (listener: (session: SessionSummary) => void) => () => void
  onIdeTerminalState: (listener: (state: IdeTerminalState) => void) => () => void
  onSessionMetrics: (listener: (payload: SessionMetricsUpdate) => void) => () => void
  onSessionHistory: (listener: (payload: SessionHistoryUpdate) => void) => () => void
  onSessionDiff: (listener: (payload: SessionDiffUpdate) => void) => () => void
  onWorkspaceState: (listener: (summary: WorkspaceSummary) => void) => () => void
  onWorkspaceCreated: (listener: (workspace: WorkspaceContext) => void) => () => void
  onWorkspaceUpdated: (listener: (workspace: WorkspaceContext) => void) => () => void
  onWorkspaceSwitched: (listener: (workspace: WorkspaceContext) => void) => () => void
  onWorkspaceRemoved: (listener: (payload: WorkspaceRemovedEvent) => void) => () => void
  onActivityLog: (listener: (entry: ActivityLogEntry) => void) => () => void
  onTabOutput: (listener: (event: TabOutputEvent) => void) => () => void
  onTabState: (listener: (payload: TabStateUpdate) => void) => () => void
  onTabMetrics: (listener: (payload: TabMetricsUpdate) => void) => () => void
  onChangesUpdated: (listener: (payload: ChangesUpdatedEvent) => void) => () => void
  onUnifiedSandboxUpdated: (listener: (payload: UnifiedSandboxUpdatedEvent) => void) => () => void
}

declare global {
  interface Window {
    sentinel: SentinelApi
  }
}

export { }
