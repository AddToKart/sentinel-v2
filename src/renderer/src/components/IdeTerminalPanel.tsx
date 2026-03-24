import { useEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { FitAddon } from '@xterm/addon-fit'
import { Terminal } from '@xterm/xterm'
import { CheckCheck, LoaderCircle, RefreshCw, RotateCcw, TerminalSquare } from 'lucide-react'

import type { IdeTerminalState } from '@shared/types'
import { getErrorMessage } from '../error-utils'
import {
  createTerminalOptions,
  getTerminalRecoveryIntervalMs,
  installTerminalMaintenance,
  refreshTerminalSurface
} from '../terminal-config'
import { clearIdeTerminalOutput, subscribeToIdeTerminalOutput } from '../terminal-stream'

interface IdeTerminalPanelProps {
  fitNonce: number
  projectPath?: string
  terminalState: IdeTerminalState
  windowsBuildNumber?: number
  onClose?: () => void
  actionsTarget?: HTMLDivElement | null
  /** When false/undefined the panel is CSS-hidden; defer init until first visible */
  isVisible?: boolean
}

function createIdleState(projectPath?: string): IdeTerminalState {
  return {
    status: 'idle',
    shell: 'powershell.exe',
    cwd: projectPath,
    modifiedPaths: []
  }
}

function describeState(state: IdeTerminalState): string {
  if (state.status === 'idle') return 'idle'
  if (state.status === 'starting') return 'starting'
  if (state.status === 'closing') return 'closing'
  if (state.status === 'error') return 'shell error'
  if (state.status === 'closed') return 'closed'
  return 'ready'
}

export function IdeTerminalPanel({
  fitNonce,
  projectPath,
  terminalState: externalState,
  windowsBuildNumber,
  onClose,
  actionsTarget,
  isVisible = false
}: IdeTerminalPanelProps): JSX.Element {
  const terminalHostRef = useRef<HTMLDivElement | null>(null)
  const terminalRef = useRef<Terminal | null>(null)
  const fitAddonRef = useRef<FitAddon | null>(null)
  const writeQueueRef = useRef<string[]>([])
  const writeFrameRef = useRef<number | null>(null)
  const writeInFlightRef = useRef(false)
  const fitFrameRef = useRef<number | null>(null)
  const fitTimerRef = useRef<number | null>(null)
  const focusFrameRef = useRef<number | null>(null)
  const recoveryTimerRef = useRef<number | null>(null)
  const lastGeometryRef = useRef({ width: 0, height: 0, cols: 0, rows: 0 })
  const lastProjectPathRef = useRef<string | undefined>(projectPath)
  const hasWrittenExitRef = useRef(false)
  // Track if we've done the initial terminal connection
  const hasInitializedRef = useRef(false)

  const [terminalState, setTerminalState] = useState<IdeTerminalState>(() => externalState)
  const [connecting, setConnecting] = useState(false)
  const [operationLoading, setOperationLoading] = useState<'apply' | 'discard' | null>(null)

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

    if (!hostChanged && !cellGeometryChanged) {
      return
    }

    lastGeometryRef.current = { width, height, cols, rows }
    if (cellGeometryChanged) {
      void window.sentinel.resizeIdeTerminal(cols, rows)
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
    lastGeometryRef.current = { width: 0, height: 0, cols: 0, rows: 0 }
    scheduleTerminalFit(0)
    requestAnimationFrame(() => {
      if (terminalRef.current) {
        refreshTerminalSurface(terminalRef.current)
      }
    })
  }

  async function ensureTerminal(resetOutput = false): Promise<void> {
    if (!projectPath) {
      setTerminalState(createIdleState())
      return
    }

    if (resetOutput) {
      clearIdeTerminalOutput()
      terminalRef.current?.clear()
      lastGeometryRef.current = { width: 0, height: 0, cols: 0, rows: 0 }
      hasWrittenExitRef.current = false
    }

    setConnecting(true)
    try {
      const state = await window.sentinel.ensureIdeTerminal()
      setTerminalState(state)
      requestAnimationFrame(() => {
        scheduleTerminalFit(0)
        scheduleTerminalFocus()
      })
    } finally {
      setConnecting(false)
    }
  }

  useEffect(() => {
    if (!terminalHostRef.current) {
      return
    }

    const terminal = new Terminal(createTerminalOptions(windowsBuildNumber))

    const fitAddon = new FitAddon()
    terminal.loadAddon(fitAddon)
    terminal.open(terminalHostRef.current)
    configureTerminalDom(terminal)

    const outputCleanup = subscribeToIdeTerminalOutput((data) => {
      enqueueOutput(data)
    })

    const inputDisposable = terminal.onData((data) => {
      void window.sentinel.sendIdeTerminalInput(data)
    })

    const observer = new ResizeObserver(() => {
      scheduleTerminalFit(140)
    })
    observer.observe(terminalHostRef.current)

    const disposeMaintenance = installTerminalMaintenance(terminal, () => true)

    terminalRef.current = terminal
    fitAddonRef.current = fitAddon
    // NOTE: DO NOT call ensureTerminal() here.
    // Initialization is deferred to the isVisible effect to prevent a zero-dimension
    // PTY resize crashing the Tauri backend when the panel is first mounted hidden.

    return () => {
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
      writeQueueRef.current = []
      writeInFlightRef.current = false
      lastGeometryRef.current = { width: 0, height: 0, cols: 0, rows: 0 }
      terminal.dispose()
      terminalRef.current = null
      fitAddonRef.current = null
      hasInitializedRef.current = false
    }
  }, [windowsBuildNumber])

  // Defer initial ensureTerminal() until the panel is actually visible.
  // This prevents a zero-dimension PTY resize crashing the Tauri backend.
  useEffect(() => {
    if (!isVisible || hasInitializedRef.current) return
    hasInitializedRef.current = true
    ensureTerminal().catch((error) => {
      console.error('Failed to ensure IDE terminal on first show:', error)
      setTerminalState((prev) => ({ ...prev, status: 'error', error: getErrorMessage(error) }))
    })
    requestAnimationFrame(() => {
      scheduleTerminalFit(0)
      scheduleTerminalFocus()
    })
  }, [isVisible])

  useEffect(() => {
    setTerminalState(externalState)
  }, [externalState])

  useEffect(() => {
    if (lastProjectPathRef.current !== projectPath) {
      lastProjectPathRef.current = projectPath
      ensureTerminal(true).catch((error) => {
        console.error('Failed to ensure IDE terminal on project change:', error)
        setTerminalState((prev) => ({ ...prev, status: 'error', error: getErrorMessage(error) }))
      })
    }
  }, [projectPath])

  useEffect(() => {
    scheduleTerminalFit(60)
    requestAnimationFrame(() => {
      if (terminalRef.current) refreshTerminalSurface(terminalRef.current)
    })
  }, [fitNonce])

  useEffect(() => {
    // Always clear existing interval first
    if (recoveryTimerRef.current !== null) {
      window.clearInterval(recoveryTimerRef.current)
      recoveryTimerRef.current = null
    }

    // Only set up new interval if status is ready
    if (terminalState.status !== 'ready') {
      return
    }

    const intervalMs = getTerminalRecoveryIntervalMs('ide-terminal')
    recoveryTimerRef.current = window.setInterval(() => {
      healTerminalDisplay()
    }, intervalMs)

    return () => {
      // Cleanup will run when effect reruns or component unmounts
      if (recoveryTimerRef.current !== null) {
        window.clearInterval(recoveryTimerRef.current)
        recoveryTimerRef.current = null
      }
    }
  }, [terminalState.status])

  useEffect(() => {
    if (terminalState.status !== 'closed' && terminalState.status !== 'error') {
      hasWrittenExitRef.current = false
      return
    }

    if (hasWrittenExitRef.current) {
      return
    }

    enqueueOutput(`\r\n\x1b[38;2;255;170;170mIDE terminal ${describeState(terminalState)}.\x1b[0m\r\n`)
    if (terminalState.error) {
      enqueueOutput(`\x1b[38;2;143;165;184m${terminalState.error}\x1b[0m\r\n`)
    }
    hasWrittenExitRef.current = true
  }, [terminalState])

  async function handleWorkspaceOp(op: 'apply' | 'discard'): Promise<void> {
    if (operationLoading) {
      return
    }

    setOperationLoading(op)
    try {
      if (op === 'apply') {
        const result = await window.sentinel.applyIdeWorkspace()
        if (result.conflicts.length > 0) {
          enqueueOutput(`\r\n\x1b[38;2;255;170;170mIDE apply completed with ${result.conflicts.length} conflict(s).\x1b[0m\r\n`)
        } else {
          enqueueOutput(`\r\n\x1b[38;2;140;245;221mApplied ${result.appliedPaths.length} IDE workspace file(s) back to the main project.\x1b[0m\r\n`)
        }
      } else {
        await window.sentinel.discardIdeWorkspaceChanges()
        enqueueOutput('\r\n\x1b[38;2;140;245;221mIDE workspace reset to the main project baseline.\x1b[0m\r\n')
      }
    } catch (error) {
      enqueueOutput(`\r\n\x1b[38;2;255;170;170mIDE workspace op failed: ${getErrorMessage(error)}\x1b[0m\r\n`)
    } finally {
      setOperationLoading(null)
    }
  }

  if (!projectPath) {
    return (
      <div className="flex h-full items-center justify-center border-t border-white/10 bg-[#060a0f] text-sm text-sentinel-mist">
        Open a project folder to start the IDE terminal.
      </div>
    )
  }

  const actions = (
    <>
      {terminalState.modifiedPaths.length > 0 && (
        <span className="text-[10px] uppercase tracking-[0.2em] text-amber-300/80 mr-1">
          {terminalState.modifiedPaths.length} changes
        </span>
      )}
      {(connecting || terminalState.status === 'starting') && (
        <LoaderCircle className="h-3 w-3 animate-spin text-amber-300" />
      )}
      <span className="text-[10px] uppercase tracking-[0.2em] text-sentinel-mist/70">
        {describeState(terminalState)}
      </span>
      <button
        className="inline-flex h-5 w-5 items-center justify-center text-sentinel-mist/60 transition hover:text-sentinel-glow disabled:opacity-30"
        disabled={terminalState.modifiedPaths.length === 0 || operationLoading !== null}
        onClick={() => void handleWorkspaceOp('apply')}
        title="Apply IDE workspace to main project"
        type="button"
      >
        <CheckCheck className="h-3.5 w-3.5" />
      </button>
      <button
        className="inline-flex h-5 w-5 items-center justify-center text-sentinel-mist/60 transition hover:text-rose-300 disabled:opacity-30"
        disabled={terminalState.modifiedPaths.length === 0 || operationLoading !== null}
        onClick={() => void handleWorkspaceOp('discard')}
        title="Reset IDE workspace"
        type="button"
      >
        <RotateCcw className="h-3.5 w-3.5" />
      </button>
      <button
        className="inline-flex h-5 w-5 items-center justify-center text-sentinel-mist/60 transition hover:text-white"
        onClick={() => {
          if (terminalState.status === 'ready') {
            healTerminalDisplay()
            return
          }

          ensureTerminal(true).catch((error) => {
            console.error('Failed to reconnect IDE terminal:', error)
            enqueueOutput(`\r\n\x1b[38;2;255;170;170mReconnection failed: ${getErrorMessage(error)}\x1b[0m\r\n`)
          })
        }}
        title={terminalState.status === 'ready' ? 'Recover display' : 'Reconnect shell'}
        type="button"
      >
        <RefreshCw className="h-3.5 w-3.5" />
      </button>
      {onClose && !actionsTarget && (
        <button
          className="ml-2 inline-flex h-5 w-5 items-center justify-center text-sentinel-mist/60 transition hover:text-rose-400"
          onClick={onClose}
          title="Close IDE terminal"
          type="button"
        >
          <svg width="12" height="12" viewBox="0 0 15 15" fill="none" xmlns="http://www.w3.org/2000/svg" className="h-3.5 w-3.5">
            <path d="M11.7816 4.03157C12.0062 3.80702 12.0062 3.44295 11.7816 3.2184C11.5571 2.99385 11.193 2.99385 10.9685 3.2184L7.50005 6.68682L4.03164 3.2184C3.80708 2.99385 3.44301 2.99385 3.21846 3.2184C2.99391 3.44295 2.99391 3.80702 3.21846 4.03157L6.68688 7.49999L3.21846 10.9684C2.99391 11.193 2.99391 11.557 3.21846 11.7816C3.44301 12.0061 3.80708 12.0061 4.03164 11.7816L7.50005 8.31316L10.9685 11.7816C11.193 12.0061 11.5571 12.0061 11.7816 11.7816C12.0062 11.557 12.0062 11.193 11.7816 10.9684L8.31322 7.49999L11.7816 4.03157Z" fill="currentColor" fillRule="evenodd" clipRule="evenodd"></path>
          </svg>
        </button>
      )}
    </>
  )

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-col overflow-hidden border-t border-white/10 bg-[#060a0f]">
      {actionsTarget ? (
        createPortal(actions, actionsTarget)
      ) : (
        <div className="flex h-[24px] shrink-0 items-center justify-between border-b border-white/10 bg-black/40 px-2">
          <div className="flex min-w-0 items-center gap-2">
            <TerminalSquare className="h-3 w-3 text-sentinel-accent" />
            <span className="text-[10px] font-bold uppercase tracking-[0.2em] text-white/80">IDE Workspace</span>
            <span className="truncate text-[10px] text-sentinel-mist/70">{terminalState.workspacePath || terminalState.cwd || projectPath}</span>
          </div>
          <div className="flex items-center gap-2">
            {actions}
          </div>
        </div>
      )}

      <div className="terminal-host h-full min-h-0 w-full overflow-hidden" onMouseDown={() => scheduleTerminalFocus()} onWheel={handleWheel} ref={terminalHostRef} />
    </div>
  )
}
