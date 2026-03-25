import { useEffect } from 'react'
import type { Dispatch, MutableRefObject, SetStateAction } from 'react'

import type {
  ActivityLogEntry,
  IdeTerminalState,
  ProjectState,
  SessionCommandEntry,
  SessionSummary,
  SessionWorkspaceStrategy,
  TabSummary,
  WorkspaceContext,
  WorkspaceSummary
} from '@shared/types'

import { getErrorMessage } from '../../error-utils'
import type { SelectedFileEntry } from '../../workspace-overlay'
import {
  getSentinelBridge,
  missingBridgeMessage,
  sortWorkspaces,
  upsertWorkspace
} from '../support'

interface UseSentinelBootstrapOptions {
  workspacesRef: MutableRefObject<WorkspaceContext[]>
  setActivityLog: Dispatch<SetStateAction<ActivityLogEntry[]>>
  setActiveWorkspaceId: Dispatch<SetStateAction<string | null>>
  setDefaultSessionStrategy: Dispatch<SetStateAction<SessionWorkspaceStrategy>>
  setErrorMessage: Dispatch<SetStateAction<string | null>>
  setIdeTerminalState: Dispatch<SetStateAction<IdeTerminalState>>
  setMaximizedSessionId: Dispatch<SetStateAction<string | null>>
  setProject: Dispatch<SetStateAction<ProjectState>>
  setSelectedFile: Dispatch<SetStateAction<SelectedFileEntry | null>>
  setSessionDiffs: Dispatch<SetStateAction<Record<string, string[]>>>
  setSessionHistories: Dispatch<SetStateAction<Record<string, SessionCommandEntry[]>>>
  setSessions: Dispatch<SetStateAction<SessionSummary[]>>
  setTabs: Dispatch<SetStateAction<TabSummary[]>>
  setWindowsBuildNumber: Dispatch<SetStateAction<number | undefined>>
  setWorkspaceSummary: Dispatch<SetStateAction<WorkspaceSummary>>
  setWorkspaces: Dispatch<SetStateAction<WorkspaceContext[]>>
}

export function useSentinelBootstrap({
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
}: UseSentinelBootstrapOptions): void {
  useEffect(() => {
    let disposed = false
    const sentinel = getSentinelBridge()

    if (!sentinel) {
      setErrorMessage(missingBridgeMessage())
      return
    }
    const sentinelBridge = sentinel

    const unsubs = [
      sentinelBridge.onActivityLog((entry) => {
        setActivityLog((current) => {
          const index = current.findIndex((existing) => existing.id === entry.id)
          if (index >= 0) {
            const next = [...current]
            next[index] = entry
            return next
          }

          return [entry, ...current].slice(0, 100)
        })
      }),
      sentinelBridge.onProjectState(setProject),
      sentinelBridge.onWorkspaceState(setWorkspaceSummary),
      sentinelBridge.onWorkspaceCreated((workspace) => {
        setWorkspaces((current) => upsertWorkspace(current, workspace))
        setActiveWorkspaceId(workspace.id)
        setDefaultSessionStrategy(workspace.defaultSessionStrategy)
      }),
      sentinelBridge.onWorkspaceUpdated((workspace) => {
        setWorkspaces((current) => upsertWorkspace(current, workspace))
      }),
      sentinelBridge.onWorkspaceSwitched((workspace) => {
        setWorkspaces((current) => upsertWorkspace(current, workspace))
        setActiveWorkspaceId(workspace.id)
        setProject(workspace.project)
        setDefaultSessionStrategy(workspace.defaultSessionStrategy)
        setSelectedFile(null)
        setMaximizedSessionId(null)
      }),
      sentinelBridge.onWorkspaceRemoved((payload) => {
        setActiveWorkspaceId((current) =>
          current === payload.workspaceId ? null : current
        )
        setWorkspaces((current) =>
          current.filter((workspace) => workspace.id !== payload.workspaceId)
        )
      }),
      sentinelBridge.onSessionState((session) => {
        setSessions((current) => {
          const workspaceExists = workspacesRef.current.some(
            (workspace) => workspace.id === session.workspaceId
          )
          const index = current.findIndex((existing) => existing.id === session.id)

          if (index >= 0) {
            const next = [...current]
            next[index] = session
            return next
          }

          if (!workspaceExists) {
            return current
          }

          return [...current, session]
        })
      }),
      sentinelBridge.onSessionDiff((update) => {
        setSessionDiffs((current) => ({ ...current, [update.sessionId]: update.modifiedPaths }))
      }),
      sentinelBridge.onSessionHistory((update) => {
        setSessionHistories((current) => ({ ...current, [update.sessionId]: update.entries }))
      }),
      sentinelBridge.onSessionMetrics((update) => {
        setSessions((current) => {
          const index = current.findIndex((session) => session.id === update.sessionId)
          if (index >= 0 && current[index].status !== 'closed') {
            const next = [...current]
            next[index] = { ...next[index], metrics: update.metrics, pid: update.pid ?? next[index].pid }
            return next
          }

          return current
        })
      }),
      sentinelBridge.onIdeTerminalState(setIdeTerminalState),
      sentinelBridge.onTabState((update) => {
        setTabs((current) => {
          const workspaceExists = workspacesRef.current.some(
            (workspace) => workspace.id === update.workspaceId
          )
          const existing = current.find((tab) => tab.id === update.tabId)

          if (existing) {
            if (update.status === 'closed') {
              return current.filter((tab) => tab.id !== update.tabId)
            }

            return current.map((tab) =>
              tab.id === update.tabId
                ? {
                    ...tab,
                    status: update.status,
                    pid: update.pid ?? tab.pid,
                    exitCode: update.exitCode ?? tab.exitCode,
                    error: update.error ?? tab.error
                  }
                : tab
            )
          }

          if (!workspaceExists || update.status === 'closed') {
            return current
          }

          return current
        })
      }),
      sentinelBridge.onTabMetrics((update) => {
        setTabs((current) => {
          const index = current.findIndex((tab) => tab.id === update.tabId)
          if (index >= 0 && current[index].status !== 'closed') {
            const next = [...current]
            next[index] = { ...next[index], metrics: update.metrics, pid: update.pid ?? next[index].pid }
            return next
          }

          return current
        })
      })
    ]

    async function init(): Promise<void> {
      try {
        const payload = await sentinelBridge.bootstrap()
        if (disposed) {
          return
        }

        setProject(payload.project)
        setWorkspaces(sortWorkspaces(payload.workspaces))
        setActiveWorkspaceId(payload.activeWorkspaceId ?? null)
        setSessions(payload.sessions)
        setWorkspaceSummary(payload.summary)
        setActivityLog(payload.activityLog)
        setDefaultSessionStrategy(payload.preferences.defaultSessionStrategy)
        setIdeTerminalState(payload.ideTerminal)
        setWindowsBuildNumber(payload.windowsBuildNumber)

        const histories: Record<string, SessionCommandEntry[]> = {}
        for (const update of payload.histories) {
          histories[update.sessionId] = update.entries
        }
        setSessionHistories(histories)

        const diffs: Record<string, string[]> = {}
        for (const update of payload.diffs) {
          diffs[update.sessionId] = update.modifiedPaths
        }
        setSessionDiffs(diffs)

        const tabsWithMetrics = payload.tabs.map((tab) => {
          const metrics = payload.tabMetrics.find((tabMetric) => tabMetric.tabId === tab.id)
          if (metrics) {
            return { ...tab, metrics: metrics.metrics, pid: metrics.pid ?? tab.pid }
          }

          return tab
        })
        setTabs(tabsWithMetrics)
      } catch (error) {
        if (!disposed) {
          setErrorMessage(`Failed to initialize Sentinel: ${getErrorMessage(error)}`)
        }
      }
    }

    void init()

    return () => {
      disposed = true
      unsubs.forEach((unsubscribe) => {
        try {
          unsubscribe()
        } catch (error) {
          console.error('[sentinel] Failed to unsubscribe from event', { error })
        }
      })
    }
  }, [
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
    setWorkspaces,
    workspacesRef
  ])
}
