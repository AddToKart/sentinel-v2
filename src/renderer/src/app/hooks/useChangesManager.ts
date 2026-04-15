import { useCallback, useEffect, useRef, useState } from 'react'
import {
  AgentFileChange,
  ChangesManagerState,
  ChangesUpdatedEvent,
  UnifiedSandboxEntry,
  UnifiedSandboxUpdatedEvent
} from '@shared/types'
import { getSentinelBridge } from '../support'
import { getErrorMessage } from '../../error-utils'

interface UseChangesManagerReturn {
  isOpen: boolean
  isLoading: boolean
  changesState: ChangesManagerState | null
  selectedFile: { agentId: string; filePath: string } | null
  error: string | null
  togglePanel: () => void
  openPanel: () => void
  closePanel: () => void
  selectFile: (agentId: string, filePath: string) => void
  clearSelectedFile: () => void
  pushAll: () => Promise<void>
  pushAgentChanges: (agentId: string) => Promise<void>
  discardAll: () => Promise<void>
  discardAgentChanges: (agentId: string) => Promise<void>
  discardFileChange: (changeId: string) => Promise<void>
  resolveConflict: (filePath: string, winningAgentId: string) => Promise<void>
  getAgentChanges: (agentId: string) => AgentFileChange[]
  getUnifiedConflicts: () => UnifiedSandboxEntry[]
  refreshChanges: () => Promise<void>
}

export function useChangesManager(workspaceId: string | null): UseChangesManagerReturn {
  const [isOpen, setIsOpen] = useState(false)
  const [isLoading, setIsLoading] = useState(false)
  const [changesState, setChangesState] = useState<ChangesManagerState | null>(null)
  const [selectedFile, setSelectedFile] = useState<{ agentId: string; filePath: string } | null>(null)
  const [error, setError] = useState<string | null>(null)
  const disposedRef = useRef(false)

  const loadChanges = useCallback(async () => {
    if (!workspaceId) return
    const sentinel = getSentinelBridge()
    if (!sentinel) return

    try {
      setIsLoading(true)
      const state = await sentinel.getChangesManagerState(workspaceId)
      if (!disposedRef.current) {
        setChangesState(state)
      }
    } catch (err) {
      if (!disposedRef.current) {
        setError(getErrorMessage(err))
      }
    } finally {
      if (!disposedRef.current) {
        setIsLoading(false)
      }
    }
  }, [workspaceId])

  useEffect(() => {
    disposedRef.current = false
    if (workspaceId) {
      void loadChanges()
    }
    return () => {
      disposedRef.current = true
    }
  }, [workspaceId, loadChanges])

  useEffect(() => {
    if (!workspaceId) return
    const sentinel = getSentinelBridge()
    if (!sentinel) return

    const unsubChanges = sentinel.onChangesUpdated((_payload: ChangesUpdatedEvent) => {
      void loadChanges()
    })

    const unsubUnified = sentinel.onUnifiedSandboxUpdated((_payload: UnifiedSandboxUpdatedEvent) => {
      void loadChanges()
    })

    return () => {
      unsubChanges()
      unsubUnified()
    }
  }, [workspaceId, loadChanges])

  const togglePanel = useCallback(() => {
    setIsOpen((prev) => !prev)
  }, [])

  const openPanel = useCallback(() => {
    setIsOpen(true)
  }, [])

  const closePanel = useCallback(() => {
    setIsOpen(false)
    setSelectedFile(null)
  }, [])

  const selectFile = useCallback((agentId: string, filePath: string) => {
    setSelectedFile({ agentId, filePath })
  }, [])

  const clearSelectedFile = useCallback(() => {
    setSelectedFile(null)
  }, [])

  const pushAll = useCallback(async () => {
    if (!workspaceId) return
    const sentinel = getSentinelBridge()
    if (!sentinel) return

    try {
      setIsLoading(true)
      await sentinel.pushUnifiedSandbox(workspaceId)
      await loadChanges()
    } catch (err) {
      setError(getErrorMessage(err))
    } finally {
      setIsLoading(false)
    }
  }, [workspaceId, loadChanges])

  const pushAgentChanges = useCallback(async (_agentId: string) => {
    if (!workspaceId) return
    const sentinel = getSentinelBridge()
    if (!sentinel) return

    try {
      setIsLoading(true)
      await sentinel.pushUnifiedSandbox(workspaceId)
      await loadChanges()
    } catch (err) {
      setError(getErrorMessage(err))
    } finally {
      setIsLoading(false)
    }
  }, [workspaceId, loadChanges])

  const discardAll = useCallback(async () => {
    if (!workspaceId) return
    const sentinel = getSentinelBridge()
    if (!sentinel) return

    try {
      setIsLoading(true)
      await sentinel.discardChanges(workspaceId)
      await loadChanges()
    } catch (err) {
      setError(getErrorMessage(err))
    } finally {
      setIsLoading(false)
    }
  }, [workspaceId, loadChanges])

  const discardAgentChanges = useCallback(async (agentId: string) => {
    if (!workspaceId) return
    const sentinel = getSentinelBridge()
    if (!sentinel) return

    try {
      setIsLoading(true)
      await sentinel.discardChanges(workspaceId, agentId)
      await loadChanges()
    } catch (err) {
      setError(getErrorMessage(err))
    } finally {
      setIsLoading(false)
    }
  }, [workspaceId, loadChanges])

  const discardFileChange = useCallback(async (_changeId: string) => {
    if (!workspaceId) return
    const sentinel = getSentinelBridge()
    if (!sentinel) return

    try {
      setIsLoading(true)
      await sentinel.discardChanges(workspaceId)
      await loadChanges()
    } catch (err) {
      setError(getErrorMessage(err))
    } finally {
      setIsLoading(false)
    }
  }, [workspaceId, loadChanges])

  const resolveConflict = useCallback(async (filePath: string, winningAgentId: string) => {
    if (!workspaceId) return
    const sentinel = getSentinelBridge()
    if (!sentinel) return

    try {
      setIsLoading(true)
      await sentinel.resolveFileConflict(workspaceId, filePath, winningAgentId)
      await loadChanges()
    } catch (err) {
      setError(getErrorMessage(err))
    } finally {
      setIsLoading(false)
    }
  }, [workspaceId, loadChanges])

  const getAgentChanges = useCallback((agentId: string): AgentFileChange[] => {
    if (!changesState) return []
    return changesState.agentChanges.filter(
      (change) => change.agentId === agentId && change.unifiedStatus === 'pending'
    )
  }, [changesState])

  const getUnifiedConflicts = useCallback((): UnifiedSandboxEntry[] => {
    if (!changesState) return []
    return changesState.unifiedEntries.filter((entry) => entry.status === 'conflicted')
  }, [changesState])

  const refreshChanges = useCallback(async () => {
    await loadChanges()
  }, [loadChanges])

  return {
    isOpen,
    isLoading,
    changesState,
    selectedFile,
    error,
    togglePanel,
    openPanel,
    closePanel,
    selectFile,
    clearSelectedFile,
    pushAll,
    pushAgentChanges,
    discardAll,
    discardAgentChanges,
    discardFileChange,
    resolveConflict,
    getAgentChanges,
    getUnifiedConflicts,
    refreshChanges
  }
}
