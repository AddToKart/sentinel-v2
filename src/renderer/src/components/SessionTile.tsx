import { useEffect, useRef, useState } from 'react'
import { FitAddon } from '@xterm/addon-fit'
import { Terminal } from '@xterm/xterm'
import { DiffEditor } from '@monaco-editor/react'
import {
  CopyCheck,
  Code2,
  Cpu,
  GitCommit,
  GitMerge,
  History,
  LoaderCircle,
  Maximize2,
  MemoryStick,
  Minimize2,
  Pause,
  Play,
  RefreshCw,
  Search,
  Square,
  Sparkles,
  TerminalSquare,
  Trash2,
} from 'lucide-react'

import type {
  SessionApplyResult,
  SessionCommandEntry,
  SessionCommitResult,
  SessionSummary
} from '@shared/types'
import { getErrorMessage } from '../error-utils'
import {
  createTerminalOptions,
  getTerminalRecoveryIntervalMs,
  installTerminalMaintenance,
  refreshTerminalSurface
} from '../terminal-config'
import { attachSessionOutput } from '../terminal-stream'

interface SessionTileProps {
  session: SessionSummary
  historyEntries: SessionCommandEntry[]
  modifiedPaths: string[]
  onClose: (sessionId: string) => Promise<void>
  onPause: (sessionId: string) => Promise<void>
  onResume: (sessionId: string) => Promise<void>
  onDelete: (sessionId: string) => Promise<void>
  onToggleMaximize: (sessionId: string) => void
  applySession: () => Promise<SessionApplyResult>
  commitSession: (message: string) => Promise<SessionCommitResult>
  discardSessionChanges: () => Promise<void>
  isMaximized: boolean
  fitNonce: number
  windowsBuildNumber?: number
}

const sessionTerminalGeometryCache = new Map<string, { cols: number; rows: number }>()

function statusColor(status: SessionSummary['status']): string {
  if (status === 'ready') return 'bg-emerald-400'
  if (status === 'starting') return 'bg-amber-400 animate-pulse'
  if (status === 'closing') return 'bg-sky-400 animate-pulse'
  if (status === 'paused') return 'bg-amber-300'
  if (status === 'error') return 'bg-rose-400'
  return 'bg-white/20'
}

function cleanupLabel(session: SessionSummary): string {
  if (session.status === 'closing') return 'closing'
  if (session.status === 'paused') return 'paused'
  if (session.cleanupState === 'removed') return 'cleaned'
  if (session.cleanupState === 'preserved') return 'preserved'
  if (session.cleanupState === 'failed') return 'cleanup failed'
  return session.status
}

function formatTime(timestamp: number): string {
  return new Date(timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' })
}

function writeConflictSummary(
  conflicts: SessionApplyResult['conflicts'],
  enqueueOutput: (data: string) => void,
  prefix: string
): void {
  if (conflicts.length === 0) {
    return
  }

  enqueueOutput(`\r\n\x1b[38;2;255;170;170m${prefix}: ${conflicts.length} conflict(s).\x1b[0m\r\n`)
  for (const conflict of conflicts.slice(0, 8)) {
    enqueueOutput(`\x1b[38;2;143;165;184mconflict:\x1b[0m ${conflict.path}\r\n`)
  }
  if (conflicts.length > 8) {
    enqueueOutput(`\x1b[38;2;143;165;184m...and ${conflicts.length - 8} more\x1b[0m\r\n`)
  }
}

export function SessionTile({
  session,
  historyEntries,
  modifiedPaths,
  onClose,
  onPause,
  onResume,
  onDelete,
  onToggleMaximize,
  applySession,
  commitSession,
  discardSessionChanges,
  isMaximized,
  fitNonce,
  windowsBuildNumber
}: SessionTileProps): JSX.Element {
  const isPaused = session.status === 'paused'
  const isArchived = session.status === 'closed' || session.status === 'error'
  const isTerminalLive =
    session.status === 'starting' || session.status === 'ready' || session.status === 'closing'
  const terminalHostRef = useRef<HTMLDivElement | null>(null)
  const terminalRef = useRef<Terminal | null>(null)
  const fitAddonRef = useRef<FitAddon | null>(null)
  const hasWrittenExitRef = useRef(false)
  const sessionStatusRef = useRef(session.status)
  const viewModeRef = useRef<'terminal' | 'history' | 'review'>('terminal')
  const writeQueueRef = useRef<string[]>([])
  const writeFrameRef = useRef<number | null>(null)
  const writeInFlightRef = useRef(false)
  const fitFrameRef = useRef<number | null>(null)
  const fitTimerRef = useRef<number | null>(null)
  const focusFrameRef = useRef<number | null>(null)
  const rebuildTimerRef = useRef<number | null>(null)
  const recoveryTimerRef = useRef<number | null>(null)
  const lastRebuildAtRef = useRef(0)
  const lastGeometryRef = useRef({ width: 0, height: 0, cols: 0, rows: 0 })

  const [viewMode, setViewMode] = useState<'terminal' | 'history' | 'review'>('terminal')
  const [historyQuery, setHistoryQuery] = useState('')
  const [opLoading, setOpLoading] = useState<string | null>(null)
  const [reviewFile, setReviewFile] = useState<string>(modifiedPaths[0] || '')
  const [originalContent, setOriginalContent] = useState('')
  const [modifiedContent, setModifiedContent] = useState('')
  const [terminalEpoch, setTerminalEpoch] = useState(0)

  useEffect(() => {
    sessionStatusRef.current = session.status
  }, [session.status])

  useEffect(() => {
    viewModeRef.current = viewMode
  }, [viewMode])

  function scheduleWriteFlush(): void {
    if (writeFrameRef.current !== null) {
      return
    }

    writeFrameRef.current = requestAnimationFrame(() => {
      writeFrameRef.current = null

      const terminal = terminalRef.current
      if (!terminal || writeInFlightRef.current || writeQueueRef.current.length === 0) {
        return
      }

      const chunk = writeQueueRef.current.join('')
      writeQueueRef.current = []
      writeInFlightRef.current = true

      terminal.write(chunk, () => {
        writeInFlightRef.current = false
        if (writeQueueRef.current.length > 0) {
          scheduleWriteFlush()
        }
      })
    })
  }

  function enqueueOutput(data: string): void {
    if (!data) {
      return
    }

    writeQueueRef.current.push(data)
    scheduleWriteFlush()
  }

  function performTerminalFit(): void {
    if (viewModeRef.current !== 'terminal') {
      return
    }

    const host = terminalHostRef.current
    const terminal = terminalRef.current
    const fitAddon = fitAddonRef.current

    if (!host || !terminal || !fitAddon) {
      return
    }

    const width = host.clientWidth
    const height = host.clientHeight
    if (width < 8 || height < 8) {
      return
    }

    fitAddon.fit()
    refreshTerminalSurface(terminal)

    const cols = terminal.cols
    const rows = terminal.rows
    if (cols <= 0 || rows <= 0) {
      return
    }

    const lastGeometry = lastGeometryRef.current
    const hostChanged = lastGeometry.width !== width || lastGeometry.height !== height
    const cellGeometryChanged = lastGeometry.cols !== cols || lastGeometry.rows !== rows
    const cachedGeometry = sessionTerminalGeometryCache.get(session.id)
    const backendGeometryChanged =
      cachedGeometry?.cols !== cols || cachedGeometry?.rows !== rows

    if (!hostChanged && !cellGeometryChanged) {
      return
    }

    lastGeometryRef.current = { width, height, cols, rows }
    sessionTerminalGeometryCache.set(session.id, { cols, rows })
    if (cellGeometryChanged && backendGeometryChanged) {
      void window.sentinel.resizeSession(session.id, cols, rows)
    }
  }

  function scheduleTerminalFit(delay = 120): void {
    if (fitTimerRef.current !== null) {
      window.clearTimeout(fitTimerRef.current)
    }

    fitTimerRef.current = window.setTimeout(() => {
      fitTimerRef.current = null

      if (fitFrameRef.current !== null) {
        cancelAnimationFrame(fitFrameRef.current)
      }

      fitFrameRef.current = requestAnimationFrame(() => {
        fitFrameRef.current = null
        performTerminalFit()
      })
    }, delay)
  }

  function scheduleTerminalFocus(): void {
    if (focusFrameRef.current !== null) {
      return
    }

    focusFrameRef.current = requestAnimationFrame(() => {
      focusFrameRef.current = null
      if (viewModeRef.current !== 'terminal') {
        return
      }

      terminalRef.current?.focus()
    })
  }

  function configureTerminalDom(terminal: Terminal): void {
    const textarea = terminal.textarea
    if (!textarea) {
      return
    }

    textarea.spellcheck = false
    textarea.autocapitalize = 'off'
    textarea.autocomplete = 'off'
    textarea.setAttribute('autocorrect', 'off')
    textarea.setAttribute('aria-hidden', 'true')
    textarea.style.pointerEvents = 'none'
    textarea.style.position = 'fixed'
    textarea.style.left = '-99999px'
    textarea.style.top = '0'
    textarea.style.width = '1px'
    textarea.style.height = '1px'
    textarea.style.opacity = '0'
  }

  function handleWheel(event: React.WheelEvent<HTMLDivElement>): void {
    const terminal = terminalRef.current
    if (!terminal || event.deltaY === 0) {
      return
    }

    event.preventDefault()
    const lines = Math.sign(event.deltaY) * Math.max(1, Math.ceil(Math.abs(event.deltaY) / 48))
    terminal.scrollLines(lines)
  }

  function healTerminalDisplay(): void {
    if (!isTerminalLive) {
      return
    }
    lastGeometryRef.current = { width: 0, height: 0, cols: 0, rows: 0 }
    scheduleTerminalFit(0)
    requestAnimationFrame(() => {
      if (terminalRef.current) {
        refreshTerminalSurface(terminalRef.current)
      }
    })
  }

  function requestTerminalRebuild(delay = 180): void {
    if (!isTerminalLive || viewModeRef.current !== 'terminal') {
      return
    }

    const now = Date.now()
    if (now - lastRebuildAtRef.current < 1500) {
      return
    }

    if (rebuildTimerRef.current !== null) {
      window.clearTimeout(rebuildTimerRef.current)
    }

    rebuildTimerRef.current = window.setTimeout(() => {
      rebuildTimerRef.current = null
      lastRebuildAtRef.current = Date.now()
      setTerminalEpoch((value) => value + 1)
    }, delay)
  }

  // Terminal initialization
  useEffect(() => {
    if (!terminalHostRef.current || !isTerminalLive) return

    let cancelled = false
    let disposeTerminal = () => {}
    const supportsIdleCallback = typeof (window as any).requestIdleCallback === 'function'

    const initializeTerminal = () => {
      if (cancelled || !terminalHostRef.current) {
        return
      }

      const terminal = new Terminal(createTerminalOptions(windowsBuildNumber))

      const fitAddon = new FitAddon()
      terminal.loadAddon(fitAddon)
      terminal.open(terminalHostRef.current)
      configureTerminalDom(terminal)

      let primed = false
      const pendingOutput: string[] = []
      const { replayData, unsubscribe: outputCleanup } = attachSessionOutput(session.id, (data) => {
        if (!primed) {
          pendingOutput.push(data)
          return
        }
        enqueueOutput(data)
      })

      const inputDisposable = terminal.onData((data) => {
        if (sessionStatusRef.current === 'ready' || sessionStatusRef.current === 'starting') {
          void window.sentinel.sendInput(session.id, data)
        }
      })

      const observer = new ResizeObserver(() => {
        scheduleTerminalFit(140)
      })
      observer.observe(terminalHostRef.current)

      const disposeMaintenance = installTerminalMaintenance(
        terminal,
        () => viewModeRef.current === 'terminal'
      )

      terminalRef.current = terminal
      fitAddonRef.current = fitAddon

      const flushBufferedOutput = () => {
        const chunks: string[] = []
        if (replayData) {
          chunks.push(replayData)
        }
        if (pendingOutput.length > 0) {
          chunks.push(pendingOutput.join(''))
          pendingOutput.length = 0
        }
        primed = true
        if (chunks.length > 0) {
          enqueueOutput(chunks.join(''))
        }
      }

      performTerminalFit()

      requestAnimationFrame(() => {
        performTerminalFit()
        flushBufferedOutput()
        scheduleTerminalFit(80)
        scheduleTerminalFocus()
      })

      disposeTerminal = () => {
        observer.disconnect()
        disposeMaintenance()
        outputCleanup()
        inputDisposable.dispose()
        if (writeFrameRef.current !== null) {
          cancelAnimationFrame(writeFrameRef.current)
          writeFrameRef.current = null
        }
        if (fitFrameRef.current !== null) {
          cancelAnimationFrame(fitFrameRef.current)
          fitFrameRef.current = null
        }
        if (fitTimerRef.current !== null) {
          window.clearTimeout(fitTimerRef.current)
          fitTimerRef.current = null
        }
        if (focusFrameRef.current !== null) {
          cancelAnimationFrame(focusFrameRef.current)
          focusFrameRef.current = null
        }
        if (rebuildTimerRef.current !== null) {
          window.clearTimeout(rebuildTimerRef.current)
          rebuildTimerRef.current = null
        }
        writeQueueRef.current = []
        writeInFlightRef.current = false
        lastGeometryRef.current = { width: 0, height: 0, cols: 0, rows: 0 }
        terminal.dispose()
        terminalRef.current = null
        fitAddonRef.current = null
      }
    }

    const idleHandle = supportsIdleCallback
      ? ((window as any).requestIdleCallback(initializeTerminal, { timeout: 120 }) as number)
      : window.setTimeout(initializeTerminal, 16)

    return () => {
      cancelled = true
      if (supportsIdleCallback) {
        ;(window as any).cancelIdleCallback?.(idleHandle)
      } else {
        window.clearTimeout(idleHandle)
      }
      disposeTerminal()
    }
  }, [isTerminalLive, session.id, terminalEpoch, windowsBuildNumber])

  useEffect(() => {
    if (!isTerminalLive || viewMode !== 'terminal' || session.status !== 'ready') {
      if (recoveryTimerRef.current !== null) {
        window.clearInterval(recoveryTimerRef.current)
        recoveryTimerRef.current = null
      }
      return
    }

    const intervalMs = getTerminalRecoveryIntervalMs(session.id)
    recoveryTimerRef.current = window.setInterval(() => {
      healTerminalDisplay()
    }, intervalMs)

    return () => {
      if (recoveryTimerRef.current !== null) {
        window.clearInterval(recoveryTimerRef.current)
        recoveryTimerRef.current = null
      }
    }
  }, [isTerminalLive, session.id, session.status, viewMode])

  // Re-fit on nonce/mode change
  useEffect(() => {
    if (viewMode !== 'terminal' || !terminalRef.current || !fitAddonRef.current) return
    scheduleTerminalFit(60)
    requestAnimationFrame(() => {
      if (terminalRef.current) refreshTerminalSurface(terminalRef.current)
    })
  }, [session.id, fitNonce, viewMode])

  useEffect(() => {
    if (viewMode !== 'terminal') {
      return
    }

    requestAnimationFrame(() => {
      scheduleTerminalFocus()
      if (terminalRef.current) refreshTerminalSurface(terminalRef.current)
    })
  }, [viewMode])

  // Session exit message
  useEffect(() => {
    if (session.status !== 'closed' && session.status !== 'error') {
      hasWrittenExitRef.current = false
      return
    }
    if (!terminalRef.current || hasWrittenExitRef.current) return
    enqueueOutput(`\r\n\x1b[38;2;255;170;170mSession exited (code ${session.exitCode ?? 0} · ${cleanupLabel(session)})\x1b[0m\r\n`)
    if (session.error) enqueueOutput(`\x1b[38;2;143;165;184m${session.error}\x1b[0m\r\n`)
    hasWrittenExitRef.current = true
  }, [session])

  // Diff viewer content
  useEffect(() => {
    if (viewMode !== 'review' || !reviewFile) return
    let active = true
    async function load() {
      try {
        const sep = session.projectRoot.includes('/') ? '/' : '\\'
        const root = await window.sentinel.readFile(`${session.projectRoot}${sep}${reviewFile}`)
        const wt = await window.sentinel.readFile(`${session.workspacePath}${sep}${reviewFile}`)
        if (!active) return
        setOriginalContent(root)
        setModifiedContent(wt)
      } catch {
        if (!active) return
        setOriginalContent('// Unable to load')
        setModifiedContent('// Unable to load')
      }
    }
    load()
    return () => { active = false }
  }, [viewMode, reviewFile, session.projectRoot, session.workspacePath])

  useEffect(() => {
    if (modifiedPaths.length > 0 && !modifiedPaths.includes(reviewFile)) {
      setReviewFile(modifiedPaths[0])
    }
  }, [modifiedPaths])

  async function handleOp(op: 'apply' | 'commit' | 'discard') {
    if (opLoading) return
    setOpLoading(op)
    try {
      if (op === 'apply') {
        const result = await applySession()
        if (session.workspaceStrategy === 'git-worktree') {
          enqueueOutput('\r\n\x1b[38;2;140;245;221mMerged the worktree branch into the main project.\x1b[0m\r\n')
        } else if (result.appliedPaths.length > 0) {
          enqueueOutput(`\r\n\x1b[38;2;140;245;221mSynced ${result.appliedPaths.length} file(s) into the main project files.\x1b[0m\r\n`)
        } else {
          enqueueOutput('\r\n\x1b[38;2;143;165;184mNo sandbox changes were ready to sync into the main project.\x1b[0m\r\n')
        }
        if (result.remainingPaths.length > 0) {
          enqueueOutput(`\x1b[38;2;143;165;184m${result.remainingPaths.length} change(s) still remain in the workspace.\x1b[0m\r\n`)
        }
        writeConflictSummary(result.conflicts, enqueueOutput, 'Sync completed')
      }
      if (op === 'commit') {
        const defaultMessage = session.workspaceStrategy === 'sandbox-copy'
          ? 'Sentinel sandbox update'
          : 'Agent update'
        const messagePrompt = session.workspaceStrategy === 'sandbox-copy'
          ? 'Commit message for the main project:'
          : 'Commit message:'
        const response = prompt(messagePrompt, defaultMessage)
        if (response === null) {
          return
        }

        const result = await commitSession(response.trim() || defaultMessage)
        if (result.createdCommit) {
          const target = session.workspaceStrategy === 'sandbox-copy'
            ? 'the main project'
            : 'the worktree branch'
          const hashSuffix = result.commitHash ? ` as ${result.commitHash}` : ''
          enqueueOutput(`\r\n\x1b[38;2;140;245;221mCommitted ${result.committedPaths.length} file(s) to ${target}${hashSuffix}.\x1b[0m\r\n`)
        } else if (result.conflicts.length > 0) {
          enqueueOutput('\r\n\x1b[38;2;255;170;170mNo commit was created because every sandbox change is still blocked by conflicts.\x1b[0m\r\n')
        } else {
          enqueueOutput('\r\n\x1b[38;2;143;165;184mNo commit was created because there were no commit-ready changes.\x1b[0m\r\n')
        }

        if (result.appliedPaths.length > 0 && session.workspaceStrategy === 'sandbox-copy') {
          enqueueOutput(`\x1b[38;2;143;165;184m${result.appliedPaths.length} sandbox file(s) were synced into the main project.\x1b[0m\r\n`)
        }
        if (result.remainingPaths.length > 0) {
          enqueueOutput(`\x1b[38;2;143;165;184m${result.remainingPaths.length} change(s) still remain in the workspace.\x1b[0m\r\n`)
        }
        writeConflictSummary(result.conflicts, enqueueOutput, 'Commit completed')
      }
      if (op === 'discard') {
        const confirmed = confirm(
          session.workspaceStrategy === 'sandbox-copy'
            ? 'Discard all changes in this sandbox workspace and resync it with the main project?'
            : 'Discard all uncommitted changes in this worktree?'
        )
        if (confirmed) await discardSessionChanges()
      }
    } catch (error) {
      enqueueOutput(`\r\n\x1b[38;2;255;170;170mOp failed: ${getErrorMessage(error)}\x1b[0m\r\n`)
    } finally {
      setOpLoading(null)
    }
  }

  async function handleDelete(): Promise<void> {
    if (session.status === 'starting' || session.status === 'ready' || session.status === 'closing') {
      return
    }

    const confirmed = confirm(
      'Delete this saved session from Sentinel? This also removes the preserved workspace if it still exists.'
    )
    if (!confirmed) {
      return
    }

    await onDelete(session.id)
  }

  const isClosing = session.status === 'closing' || isArchived
  const canPause = session.status === 'starting' || session.status === 'ready'
  const canResume = session.status === 'paused'
  const canStop =
    session.status === 'starting' ||
    session.status === 'ready' ||
    session.status === 'paused' ||
    ((session.status === 'closed' || session.status === 'error') &&
      session.cleanupState !== 'removed')
  const canDelete = session.status === 'paused' || session.status === 'closed' || session.status === 'error'
  const canCommitSession = session.workspaceStrategy === 'git-worktree'
  const commitTitle = 'Commit Worktree Changes'
  const filteredHistory = historyQuery.trim()
    ? historyEntries.filter((e) => e.command.toLowerCase().includes(historyQuery.toLowerCase()))
    : historyEntries

  return (
    <article
      className="flex h-full min-h-0 min-w-0 flex-col overflow-hidden bg-[#060a0f] rounded-none border border-white/10"
      onMouseDown={() => { if (viewMode === 'terminal') scheduleTerminalFocus() }}
    >
      {/* ── Permanent utility strip (20px) ─────────────────────── */}
      <div className="shrink-0 flex items-center justify-between border-b border-white/10 bg-black/40 px-2 h-[20px]">
        <div className="flex items-center gap-2">
          <span className={`h-1.5 w-1.5 rounded-full ${statusColor(session.status)}`} />
          <span className="text-[10px] font-bold uppercase tracking-[0.2em] text-white/80">{session.label}</span>
          <span className="text-[9px] uppercase tracking-[0.2em] text-sentinel-mist/70">
            {session.workspaceStrategy === 'sandbox-copy' ? 'local sandbox' : 'git worktree'}
          </span>
          {modifiedPaths.length > 0 && (
            <span className="text-[9px] text-amber-400/80 tracking-widest">{modifiedPaths.length} changes</span>
          )}
        </div>
        <div className="flex items-center gap-0.5" style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}>
          {/* View toggles */}
          <button className={`px-1.5 py-0.5 text-[9px] uppercase tracking-widest transition ${viewMode === 'terminal' ? 'text-white' : 'text-white/30 hover:text-white/70'}`} onClick={() => setViewMode('terminal')} title="Terminal"><TerminalSquare className="h-2.5 w-2.5" /></button>
          <button className={`px-1.5 py-0.5 text-[9px] uppercase tracking-widest transition ${viewMode === 'review' ? 'text-emerald-400' : 'text-white/30 hover:text-white/70'}`} onClick={() => setViewMode('review')} title="Diff"><Code2 className="h-2.5 w-2.5" /></button>
          <button className={`px-1.5 py-0.5 text-[9px] uppercase tracking-widest transition ${viewMode === 'history' ? 'text-sentinel-accent' : 'text-white/30 hover:text-white/70'}`} onClick={() => setViewMode('history')} title="History"><History className="h-2.5 w-2.5" /></button>
          <div className="mx-1 h-3 w-px bg-white/10" />
          {canCommitSession && (
            <button className="px-1 text-white/30 hover:text-emerald-300 transition disabled:opacity-20" disabled={isClosing || opLoading !== null || modifiedPaths.length === 0} onClick={() => handleOp('commit')} title={commitTitle}><GitCommit className="h-2.5 w-2.5" /></button>
          )}
          <button className="px-1 text-white/30 hover:text-rose-300 transition disabled:opacity-20" disabled={isClosing || opLoading !== null || modifiedPaths.length === 0} onClick={() => handleOp('discard')} title="Discard"><Trash2 className="h-2.5 w-2.5" /></button>
          <button className="px-1 text-white/30 hover:text-sentinel-glow transition disabled:opacity-20" disabled={isClosing || opLoading !== null || modifiedPaths.length === 0} onClick={() => handleOp('apply')} title={session.workspaceStrategy === 'sandbox-copy' ? 'Sync to Main Project Files' : 'Merge to Main'}>{session.workspaceStrategy === 'sandbox-copy' ? <CopyCheck className="h-2.5 w-2.5" /> : <GitMerge className="h-2.5 w-2.5" />}</button>
          <div className="mx-1 h-3 w-px bg-white/10" />
          <button
            className="px-1 text-white/30 hover:text-emerald-300 transition disabled:opacity-20"
            disabled={!canResume}
            onClick={() => void onResume(session.id)}
            title="Resume"
          >
            <Play className="h-2.5 w-2.5" />
          </button>
          <button
            className="px-1 text-white/30 hover:text-amber-300 transition disabled:opacity-20"
            disabled={!canPause}
            onClick={() => void onPause(session.id)}
            title="Pause"
          >
            <Pause className="h-2.5 w-2.5" />
          </button>
          <button
            className="px-1 text-white/30 hover:text-rose-300 transition disabled:opacity-20"
            disabled={!canStop}
            onClick={() => void onClose(session.id)}
            title="Stop"
          >
            <Square className="h-2.5 w-2.5" />
          </button>
          <button
            className="px-1 text-white/30 hover:text-rose-300 transition disabled:opacity-20"
            disabled={!canDelete}
            onClick={() => void handleDelete()}
            title="Delete"
          >
            <Trash2 className="h-2.5 w-2.5" />
          </button>
          <div className="mx-1 h-3 w-px bg-white/10" />
          <button className="px-1 text-white/30 hover:text-sentinel-accent transition disabled:opacity-20" disabled={isClosing} onClick={() => requestTerminalRebuild(0)} title="Recover display">
            <RefreshCw className="h-2.5 w-2.5" />
          </button>
          <button className="px-1 text-white/30 hover:text-white transition" onClick={() => onToggleMaximize(session.id)} title={isMaximized ? 'Restore' : 'Maximize'}>
            {isMaximized ? <Minimize2 className="h-2.5 w-2.5" /> : <Maximize2 className="h-2.5 w-2.5" />}
          </button>
        </div>
      </div>

      {/* ── Content area ─────────────────────────────────────────── */}
      <div className="relative flex-1 min-h-0 overflow-hidden">
        {/* Terminal */}
        <div className={`absolute inset-0 ${viewMode === 'terminal' ? 'z-10' : 'z-0 pointer-events-none'}`}>
          {isTerminalLive ? (
            <div className="terminal-host h-full w-full overflow-hidden" onMouseDown={() => scheduleTerminalFocus()} onWheel={handleWheel} ref={terminalHostRef} />
          ) : (
            <div className="flex h-full items-center justify-center bg-[#04070b] p-6 text-center">
              <div className="max-w-sm space-y-3">
                <div className="mx-auto flex h-10 w-10 items-center justify-center border border-white/10 bg-white/[0.03] text-sentinel-mist">
                  {isPaused ? <Pause className="h-4 w-4 text-amber-300" /> : <Square className="h-4 w-4 text-white/70" />}
                </div>
                <div className="space-y-1">
                  <p className="text-sm font-semibold uppercase tracking-[0.18em] text-white/85">
                    {isPaused ? 'Session Paused' : session.status === 'closed' ? 'Session Stopped' : 'Session Unavailable'}
                  </p>
                  <p className="text-xs leading-6 text-sentinel-mist">
                    {isPaused
                      ? 'The agent process has been paused and the workspace was preserved. Resume the session to keep working, or stop/delete it when you are done.'
                      : session.cleanupState === 'removed'
                        ? 'This session is no longer running and its workspace has already been cleaned up.'
                        : 'This session is no longer running. Stop it to finish cleanup, or delete it if you no longer need the preserved workspace.'}
                  </p>
                  {session.error && (
                    <p className="text-[11px] leading-5 text-rose-300/90">{session.error}</p>
                  )}
                </div>
              </div>
            </div>
          )}
        </div>

        {/* Review / Diff */}
        <div className={`absolute inset-0 flex flex-col ${viewMode === 'review' ? 'z-10' : 'z-0 pointer-events-none opacity-0'}`}>
          <div className="shrink-0 bg-black/20 px-2 py-1 border-b border-white/10">
            <select
              className="w-full bg-black/60 border border-white/10 text-[11px] text-sentinel-mist p-0.5 outline-none"
              value={reviewFile}
              onChange={(e) => setReviewFile(e.target.value)}
            >
              {modifiedPaths.length === 0 && <option value="">No modified files</option>}
              {modifiedPaths.map((p) => <option key={p} value={p}>{p}</option>)}
            </select>
          </div>
          <div className="flex-1 bg-[#1e1e1e] min-h-0">
            {reviewFile ? (
              <DiffEditor
                height="100%"
                language="typescript"
                theme="vs-dark"
                original={originalContent}
                modified={modifiedContent}
                options={{ readOnly: true, minimap: { enabled: false }, fontSize: 12 }}
              />
            ) : (
              <div className="flex h-full items-center justify-center text-xs text-sentinel-mist">
                Make edits in the terminal to see diff here.
              </div>
            )}
          </div>
        </div>

        {/* History */}
        <div className={`absolute inset-0 flex flex-col ${viewMode === 'history' ? 'z-10' : 'z-0 pointer-events-none opacity-0'}`}>
          <div className="shrink-0 bg-black/20 p-2 border-b border-white/10">
            <label className="flex items-center gap-2 border border-white/10 bg-white/[0.03] px-2 py-1 text-xs text-sentinel-mist">
              <Search className="h-3 w-3 text-sentinel-accent" />
              <input
                className="min-w-0 flex-1 bg-transparent text-white outline-none text-[11px]"
                onChange={(e) => setHistoryQuery(e.target.value)}
                placeholder="Filter commands..."
                value={historyQuery}
              />
            </label>
          </div>
          <div className="flex-1 min-h-0 overflow-auto p-1.5 space-y-0.5">
            {filteredHistory.map((entry) => (
              <div key={entry.id} className="flex gap-2 border border-white/10 bg-white/[0.02] px-2 py-1 text-[10px]">
                <span className="text-sentinel-mist/50 shrink-0">{formatTime(entry.timestamp)}</span>
                <span className="font-mono text-white break-all">{entry.command}</span>
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* ── Permanent Telemetry Ribbon (18px) ───────────────────── */}
      <div className="shrink-0 flex items-center justify-between border-t border-white/[0.06] bg-black/60 px-2 h-[18px]">
        <div className="flex items-center gap-3">
          <span className="flex items-center gap-1 text-[9px] font-mono text-sentinel-mist/70">
            <Cpu className="h-2.5 w-2.5 text-sentinel-ice/60" />
            {session.metrics.cpuPercent.toFixed(1)}%
          </span>
          <span className="flex items-center gap-1 text-[9px] font-mono text-sentinel-mist/70">
            <MemoryStick className="h-2.5 w-2.5 text-sentinel-accent/60" />
            {session.metrics.memoryMb.toFixed(0)} MB
          </span>
        </div>
        <div className="flex items-center gap-3 text-[9px] font-mono text-sentinel-mist/40">
          {session.metrics.processCount > 0 && <span>{session.metrics.processCount} proc</span>}
          {opLoading && (
            <span className="flex items-center gap-1 text-amber-400/70">
              <LoaderCircle className="h-2.5 w-2.5 animate-spin" />
              {opLoading}…
            </span>
          )}
          {session.status !== 'ready' && (
            <span className="flex items-center gap-1 text-sentinel-mist/50">
              <Sparkles className="h-2.5 w-2.5" />
              {cleanupLabel(session)}
            </span>
          )}
        </div>
      </div>
    </article>
  )
}
