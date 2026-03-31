import { useEffect, useRef, useState } from 'react'
import type { WheelEvent } from 'react'
import { FitAddon } from '@xterm/addon-fit'
import { Terminal } from '@xterm/xterm'

import type { IdeTerminalState } from '@shared/types'

import { getErrorMessage } from '../../error-utils'
import {
  createTerminalOptions,
  getTerminalRecoveryIntervalMs,
  installTerminalMaintenance,
  refreshTerminalSurface
} from '../../terminal-config'
import { attachIdeTerminalOutput, clearIdeTerminalOutput } from '../../terminal-stream'
import { createIdleState, describeState } from './helpers'

interface UseIdeTerminalRuntimeOptions {
  fitNonce: number
  projectPath?: string
  terminalState: IdeTerminalState
  windowsBuildNumber?: number
  isVisible: boolean
}

const IDE_TERMINAL_GEOMETRY_CACHE_KEY = 'ide-terminal'
const ideTerminalGeometryCache = new Map<string, { cols: number; rows: number }>()

export function useIdeTerminalRuntime({
  fitNonce,
  projectPath,
  terminalState: externalState,
  windowsBuildNumber,
  isVisible
}: UseIdeTerminalRuntimeOptions) {
  const terminalHostRef = useRef<HTMLDivElement | null>(null)
  const terminalRef = useRef<Terminal | null>(null)
  const fitAddonRef = useRef<FitAddon | null>(null)
  const writeQueueRef = useRef<string[]>([])
  const writeFrameRef = useRef<number | null>(null)
  const writeInFlightRef = useRef(false)
  const fitFrameRef = useRef<number | null>(null)
  const fitTimerRef = useRef<number | null>(null)
  const focusFrameRef = useRef<number | null>(null)
  const rebuildTimerRef = useRef<number | null>(null)
  const recoveryTimerRef = useRef<number | null>(null)
  const lastGeometryRef = useRef({ width: 0, height: 0, cols: 0, rows: 0 })
  const lastProjectPathRef = useRef<string | undefined>(projectPath)
  const hasWrittenExitRef = useRef(false)
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
    const cachedGeometry = ideTerminalGeometryCache.get(IDE_TERMINAL_GEOMETRY_CACHE_KEY)
    const backendGeometryChanged =
      cachedGeometry?.cols !== cols || cachedGeometry?.rows !== rows

    if (!hostChanged && !cellGeometryChanged) {
      return
    }

    lastGeometryRef.current = { width, height, cols, rows }
    ideTerminalGeometryCache.set(IDE_TERMINAL_GEOMETRY_CACHE_KEY, { cols, rows })
    if (cellGeometryChanged && backendGeometryChanged) {
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

  function focusTerminal(): void {
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

  function handleWheel(event: WheelEvent<HTMLDivElement>): void {
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
        focusTerminal()
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

    let primed = false
    const pendingOutput: string[] = []
    const { replayData, unsubscribe: outputCleanup } = attachIdeTerminalOutput((data) => {
      if (!primed) {
        pendingOutput.push(data)
        return
      }
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
      focusTerminal()
    })

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
      hasInitializedRef.current = false
    }
  }, [windowsBuildNumber])

  useEffect(() => {
    if (!isVisible || hasInitializedRef.current) {
      return
    }

    hasInitializedRef.current = true
    ensureTerminal().catch((error) => {
      console.error('Failed to ensure IDE terminal on first show:', error)
      setTerminalState((previous) => ({ ...previous, status: 'error', error: getErrorMessage(error) }))
    })
    requestAnimationFrame(() => {
      scheduleTerminalFit(0)
      focusTerminal()
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
        setTerminalState((previous) => ({ ...previous, status: 'error', error: getErrorMessage(error) }))
      })
    }
  }, [projectPath])

  useEffect(() => {
    scheduleTerminalFit(60)
    requestAnimationFrame(() => {
      if (terminalRef.current) {
        refreshTerminalSurface(terminalRef.current)
      }
    })
  }, [fitNonce, isVisible])

  useEffect(() => {
    if (recoveryTimerRef.current !== null) {
      window.clearInterval(recoveryTimerRef.current)
      recoveryTimerRef.current = null
    }

    if (terminalState.status !== 'ready') {
      return
    }

    const intervalMs = getTerminalRecoveryIntervalMs('ide-terminal')
    recoveryTimerRef.current = window.setInterval(() => {
      healTerminalDisplay()
    }, intervalMs)

    return () => {
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

  async function runWorkspaceOp(op: 'apply' | 'discard'): Promise<void> {
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

  async function recoverOrReconnect(): Promise<void> {
    if (terminalState.status === 'ready') {
      healTerminalDisplay()
      return
    }

    try {
      await ensureTerminal(true)
    } catch (error) {
      console.error('Failed to reconnect IDE terminal:', error)
      enqueueOutput(`\r\n\x1b[38;2;255;170;170mReconnection failed: ${getErrorMessage(error)}\x1b[0m\r\n`)
    }
  }

  return {
    connecting,
    focusTerminal,
    handleWheel,
    operationLoading,
    recoverOrReconnect,
    runWorkspaceOp,
    terminalHostRef,
    terminalState
  }
}
