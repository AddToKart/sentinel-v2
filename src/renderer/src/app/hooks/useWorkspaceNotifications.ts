import { useEffect, useRef, useState } from 'react'

import type { ActivityLogEntry, SessionStatus, SessionSummary, WorkspaceContext } from '@shared/types'

const MAX_NOTIFICATIONS = 24
const BELL_RING_MS = 1200
const MAX_OUTPUT_BUFFER_LENGTH = 4_000
const OUTPUT_NOTIFICATION_COOLDOWN_MS = 8_000
const SESSION_OUTPUT_SETTLE_MS = 1_250
const PREVIEW_VISIBILITY_MS = 3600
const OUTPUT_NOISE_PATTERNS = [
  /^>\s*$/,
  /^ps\s.+>$/i,
  /^type your message/i,
  /^\?\s+for shortcuts/i,
  /^esc to cancel\)?$/i,
  /^used$/i,
  /^[0-9]+(?:\.[0-9]+)?%$/,
  /^summoning the cloud of wisdom/i
]

export interface WorkspaceNotification {
  id: string
  workspaceId: string
  workspaceName: string
  title: string
  preview: string
  tone: 'success' | 'error'
  createdAt: number
  unread: boolean
  source: 'activity' | 'session' | 'output'
}

interface UseWorkspaceNotificationsOptions {
  activeWorkspaceId: string | null
  activityLog: ActivityLogEntry[]
  bootstrapped: boolean
  sessions: SessionSummary[]
  workspaces: WorkspaceContext[]
}

function isRunningSession(status: SessionStatus): boolean {
  return status === 'starting' || status === 'ready' || status === 'closing'
}

function workspaceNameFor(workspaces: WorkspaceContext[], workspaceId: string): string {
  return workspaces.find((workspace) => workspace.id === workspaceId)?.name ?? 'Workspace'
}

function trimPreview(value: string | undefined, fallback: string): string {
  const text = (value ?? fallback).replace(/\s+/g, ' ').trim()
  if (!text) {
    return fallback
  }

  return text.length > 120 ? `${text.slice(0, 117)}...` : text
}

function stripAnsi(value: string): string {
  return value
    .replace(/\u001b\][^\u0007]*(?:\u0007|\u001b\\)/g, '')
    .replace(/\u001b\[[0-9;?]*[ -/]*[@-~]/g, '')
}

function extractOutputPreview(value: string): string | null {
  const normalized = stripAnsi(value)
    .replace(/\r/g, '\n')
    .replace(/\u0000/g, '')
    .replace(/[ \t]+/g, ' ')

  const lines = normalized
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)

  for (let index = lines.length - 1; index >= 0; index -= 1) {
    const line = lines[index]
    if (!/[A-Za-z0-9]/.test(line)) {
      continue
    }
    if (OUTPUT_NOISE_PATTERNS.some((pattern) => pattern.test(line))) {
      continue
    }

    return trimPreview(line, 'New output is ready.')
  }

  const fallback = normalized.replace(/\s+/g, ' ').trim()
  if (!fallback || !/[A-Za-z0-9]/.test(fallback)) {
    return null
  }

  return trimPreview(fallback, 'New output is ready.')
}

export function useWorkspaceNotifications({
  activeWorkspaceId,
  activityLog,
  bootstrapped,
  sessions,
  workspaces
}: UseWorkspaceNotificationsOptions) {
  const [notifications, setNotifications] = useState<WorkspaceNotification[]>([])
  const [bellRinging, setBellRinging] = useState(false)
  const [previewNotification, setPreviewNotification] = useState<WorkspaceNotification | null>(null)

  const activityBootstrapRef = useRef(false)
  const activeWorkspaceIdRef = useRef<string | null>(activeWorkspaceId)
  const bootstrappedRef = useRef(bootstrapped)
  const sessionBootstrapRef = useRef(false)
  const seenActivityIdsRef = useRef<Set<string>>(new Set())
  const sessionOutputBuffersRef = useRef<Map<string, string>>(new Map())
  const sessionOutputTimersRef = useRef<Map<string, number>>(new Map())
  const sessionsByIdRef = useRef<Map<string, SessionSummary>>(new Map())
  const previousSessionStatusesRef = useRef<Record<string, SessionStatus>>({})
  const lastOutputNotificationAtRef = useRef<Map<string, number>>(new Map())
  const lastOutputPreviewRef = useRef<Map<string, string>>(new Map())
  const bellTimeoutRef = useRef<number | null>(null)
  const previewTimeoutRef = useRef<number | null>(null)
  const pushNotificationRef = useRef<(notification: WorkspaceNotification) => void>(() => {})

  activeWorkspaceIdRef.current = activeWorkspaceId
  bootstrappedRef.current = bootstrapped
  pushNotificationRef.current = pushNotification

  useEffect(() => {
    sessionsByIdRef.current = new Map(sessions.map((session) => [session.id, session]))
  }, [sessions])

  function triggerBell(notification: WorkspaceNotification): void {
    setBellRinging(true)
    setPreviewNotification(notification)

    if (bellTimeoutRef.current) {
      window.clearTimeout(bellTimeoutRef.current)
    }
    if (previewTimeoutRef.current) {
      window.clearTimeout(previewTimeoutRef.current)
    }

    bellTimeoutRef.current = window.setTimeout(() => {
      bellTimeoutRef.current = null
      setBellRinging(false)
    }, BELL_RING_MS)
    previewTimeoutRef.current = window.setTimeout(() => {
      previewTimeoutRef.current = null
      setPreviewNotification((current) =>
        current?.id === notification.id ? null : current
      )
    }, PREVIEW_VISIBILITY_MS)
  }

  function pushNotification(notification: WorkspaceNotification): void {
    setNotifications((current) => {
      const next = [
        notification,
        ...current.filter((existing) => existing.id !== notification.id)
      ]
      return next.slice(0, MAX_NOTIFICATIONS)
    })
    triggerBell(notification)
  }

  useEffect(() => {
    return () => {
      if (bellTimeoutRef.current) {
        window.clearTimeout(bellTimeoutRef.current)
      }
      if (previewTimeoutRef.current) {
        window.clearTimeout(previewTimeoutRef.current)
      }
      for (const timeoutId of sessionOutputTimersRef.current.values()) {
        window.clearTimeout(timeoutId)
      }
      sessionOutputTimersRef.current.clear()
    }
  }, [])

  useEffect(() => {
    if (typeof window === 'undefined' || typeof window.sentinel === 'undefined') {
      return
    }

    const flushSessionOutputNotification = (sessionId: string) => {
      const session = sessionsByIdRef.current.get(sessionId)
      const buffered = sessionOutputBuffersRef.current.get(sessionId) ?? ''

      sessionOutputTimersRef.current.delete(sessionId)
      sessionOutputBuffersRef.current.delete(sessionId)

      if (!bootstrappedRef.current || !session) {
        return
      }
      if (session.workspaceId === activeWorkspaceIdRef.current) {
        return
      }
      if (session.status !== 'starting' && session.status !== 'ready' && session.status !== 'closing') {
        return
      }

      const preview = extractOutputPreview(buffered)
      if (!preview) {
        return
      }

      const now = Date.now()
      const lastPreview = lastOutputPreviewRef.current.get(sessionId)
      const lastNotifiedAt = lastOutputNotificationAtRef.current.get(sessionId) ?? 0
      if (lastPreview === preview && now - lastNotifiedAt < OUTPUT_NOTIFICATION_COOLDOWN_MS) {
        return
      }

      lastOutputPreviewRef.current.set(sessionId, preview)
      lastOutputNotificationAtRef.current.set(sessionId, now)

      const workspaceName = workspaceNameFor(workspaces, session.workspaceId)
      pushNotificationRef.current({
        id: `output:${sessionId}:${now}`,
        workspaceId: session.workspaceId,
        workspaceName,
        title: `${session.label} has new output in ${workspaceName}`,
        preview,
        tone: 'success',
        createdAt: now,
        unread: true,
        source: 'output'
      })
    }

    const unsubscribe = window.sentinel.onSessionOutput((event) => {
      if (!bootstrappedRef.current) {
        return
      }

      const session = sessionsByIdRef.current.get(event.sessionId)
      if (!session || session.workspaceId === activeWorkspaceIdRef.current) {
        return
      }

      const nextBuffered = `${sessionOutputBuffersRef.current.get(event.sessionId) ?? ''}${event.data}`
      sessionOutputBuffersRef.current.set(
        event.sessionId,
        nextBuffered.slice(-MAX_OUTPUT_BUFFER_LENGTH)
      )

      const existingTimer = sessionOutputTimersRef.current.get(event.sessionId)
      if (existingTimer !== undefined) {
        window.clearTimeout(existingTimer)
      }

      const timeoutId = window.setTimeout(() => {
        flushSessionOutputNotification(event.sessionId)
      }, SESSION_OUTPUT_SETTLE_MS)
      sessionOutputTimersRef.current.set(event.sessionId, timeoutId)
    })

    return () => {
      unsubscribe()
    }
  }, [workspaces])

  useEffect(() => {
    if (!bootstrapped) {
      return
    }

    if (!activityBootstrapRef.current) {
      seenActivityIdsRef.current = new Set(activityLog.map((entry) => entry.id))
      activityBootstrapRef.current = true
      return
    }

    const nextEntries = activityLog
      .filter((entry) => !seenActivityIdsRef.current.has(entry.id))
      .sort((left, right) => left.timestamp - right.timestamp)

    for (const entry of nextEntries) {
      seenActivityIdsRef.current.add(entry.id)

      if (!entry.workspaceId || entry.workspaceId === activeWorkspaceId) {
        continue
      }
      if (entry.status !== 'completed' && entry.status !== 'failed') {
        continue
      }

      const workspaceName = workspaceNameFor(workspaces, entry.workspaceId)
      pushNotification({
        id: `activity:${entry.id}`,
        workspaceId: entry.workspaceId,
        workspaceName,
        title: entry.status === 'completed'
          ? `${workspaceName} completed work`
          : `${workspaceName} hit a failure`,
        preview: trimPreview(entry.detail, entry.command),
        tone: entry.status === 'completed' ? 'success' : 'error',
        createdAt: entry.timestamp,
        unread: true,
        source: 'activity'
      })
    }
  }, [activityLog, activeWorkspaceId, bootstrapped, workspaces])

  useEffect(() => {
    if (!bootstrapped) {
      return
    }

    const nextStatuses: Record<string, SessionStatus> = {}

    if (!sessionBootstrapRef.current) {
      for (const session of sessions) {
        nextStatuses[session.id] = session.status
      }
      previousSessionStatusesRef.current = nextStatuses
      sessionBootstrapRef.current = true
      return
    }

    for (const session of sessions) {
      nextStatuses[session.id] = session.status
      const previousStatus = previousSessionStatusesRef.current[session.id]

      if (!previousStatus || previousStatus === session.status) {
        continue
      }
      if (session.workspaceId === activeWorkspaceId) {
        continue
      }

      const workspaceName = workspaceNameFor(workspaces, session.workspaceId)
      if (session.status === 'closed') {
        pushNotification({
          id: `session:${session.id}:closed:${session.createdAt}`,
          workspaceId: session.workspaceId,
          workspaceName,
          title: `${session.label} finished in ${workspaceName}`,
          preview: trimPreview(session.startupCommand, 'The agent session exited cleanly.'),
          tone: 'success',
          createdAt: Date.now(),
          unread: true,
          source: 'session'
        })
      } else if (session.status === 'error') {
        pushNotification({
          id: `session:${session.id}:error:${session.createdAt}`,
          workspaceId: session.workspaceId,
          workspaceName,
          title: `${session.label} failed in ${workspaceName}`,
          preview: trimPreview(session.error, 'The agent session exited with an error.'),
          tone: 'error',
          createdAt: Date.now(),
          unread: true,
          source: 'session'
        })
      }
    }

    previousSessionStatusesRef.current = nextStatuses
  }, [activeWorkspaceId, bootstrapped, sessions, workspaces])

  useEffect(() => {
    if (!activeWorkspaceId) {
      return
    }

    sessionOutputBuffersRef.current.clear()
    for (const [sessionId, timeoutId] of sessionOutputTimersRef.current.entries()) {
      const session = sessionsByIdRef.current.get(sessionId)
      if (session?.workspaceId === activeWorkspaceId) {
        window.clearTimeout(timeoutId)
        sessionOutputTimersRef.current.delete(sessionId)
      }
    }

    setNotifications((current) => {
      let changed = false
      const next = current.map((notification) => {
        if (notification.workspaceId === activeWorkspaceId && notification.unread) {
          changed = true
          return { ...notification, unread: false }
        }

        return notification
      })
      return changed ? next : current
    })
  }, [activeWorkspaceId])

  const unreadCount = notifications.filter((notification) => notification.unread).length
  const unreadCountsByWorkspace: Record<string, number> = {}
  for (const notification of notifications) {
    if (!notification.unread) {
      continue
    }

    unreadCountsByWorkspace[notification.workspaceId] =
      (unreadCountsByWorkspace[notification.workspaceId] ?? 0) + 1
  }

  const runningSessionCountsByWorkspace: Record<string, number> = {}
  for (const session of sessions) {
    if (!isRunningSession(session.status)) {
      continue
    }

    runningSessionCountsByWorkspace[session.workspaceId] =
      (runningSessionCountsByWorkspace[session.workspaceId] ?? 0) + 1
  }

  function dismissNotification(notificationId: string): void {
    setNotifications((current) =>
      current.filter((notification) => notification.id !== notificationId)
    )
    setPreviewNotification((current) =>
      current?.id === notificationId ? null : current
    )
  }

  function markAllRead(): void {
    setNotifications((current) => current.map((notification) =>
      notification.unread
        ? { ...notification, unread: false }
        : notification
    ))
  }

  function clearNotifications(): void {
    setNotifications([])
    setPreviewNotification(null)
  }

  function clearPreviewNotification(): void {
    setPreviewNotification(null)
  }

  return {
    bellRinging,
    clearNotifications,
    clearPreviewNotification,
    dismissNotification,
    markAllRead,
    notifications,
    previewNotification,
    runningSessionCountsByWorkspace,
    unreadCount,
    unreadCountsByWorkspace
  }
}
