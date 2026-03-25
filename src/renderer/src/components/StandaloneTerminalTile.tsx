import { FitAddon } from '@xterm/addon-fit'
import { Terminal } from '@xterm/xterm'
import { useEffect, useRef, useState } from 'react'
import { X, Maximize2, Minimize2 } from 'lucide-react'

import {
  createTerminalOptions,
  refreshTerminalSurface,
} from '../terminal-config'
import { subscribeToTabOutput } from '../tab-stream'

import type { TabSummary } from '@shared/types'

interface StandaloneTerminalTileProps {
  tab: TabSummary
  fitNonce: number
  onClose: () => void
  windowsBuildNumber?: number
  onFocus?: () => void
  hideMaximize?: boolean
}

export function StandaloneTerminalTile({
  tab,
  fitNonce,
  onClose,
  windowsBuildNumber,
  hideMaximize
}: StandaloneTerminalTileProps): JSX.Element {
  const terminalHostRef = useRef<HTMLDivElement | null>(null)
  const terminalRef = useRef<Terminal | null>(null)
  const fitAddonRef = useRef<FitAddon | null>(null)
  const inputDisposableRef = useRef<{ dispose: () => void } | null>(null)
  const writeQueueRef = useRef<string[]>([])
  const writeFrameRef = useRef<number | null>(null)
  const writeInFlightRef = useRef(false)
  const isDisposedRef = useRef(false)
  const rebuildTimerRef = useRef<number | null>(null)
  const lastRebuildAtRef = useRef(0)
  const [isMaximized, setIsMaximized] = useState(false)
  const [terminalEpoch, setTerminalEpoch] = useState(0)

  // Process write queue — batch all pending chunks into a single write to avoid
  // per-chunk callback recursion, which can fire after disposal and crash xterm.
  const scheduleWriteFlush = () => {
    if (writeFrameRef.current !== null) return
    writeFrameRef.current = requestAnimationFrame(() => {
      writeFrameRef.current = null
      if (isDisposedRef.current) {
        writeQueueRef.current = []
        return
      }
      const terminal = terminalRef.current
      if (!terminal || writeInFlightRef.current || writeQueueRef.current.length === 0) return
      const chunk = writeQueueRef.current.join('')
      writeQueueRef.current = []
      writeInFlightRef.current = true

      // Check disposed flag again immediately before write to prevent race conditions
      if (isDisposedRef.current) {
        writeInFlightRef.current = false
        return
      }

      terminal.write(chunk, () => {
        writeInFlightRef.current = false
        // Additional check before recursing in callback
        if (!isDisposedRef.current && writeQueueRef.current.length > 0) {
          scheduleWriteFlush()
        }
      })
    })
  }

  // Fit terminal to container
  const fitTerminal = () => {
    // Guard against fitting after disposal
    if (isDisposedRef.current) return

    const host = terminalHostRef.current
    const terminal = terminalRef.current
    const fitAddon = fitAddonRef.current

    if (!host || !terminal || !fitAddon) return

    try {
      fitAddon.fit()
      const dims = fitAddon.proposeDimensions()
      if (dims && dims.cols > 0 && dims.rows > 0) {
        if (terminal.cols !== dims.cols || terminal.rows !== dims.rows) {
          window.sentinel.resizeTab(tab.id, dims.cols, dims.rows)
        }
      }
      refreshTerminalSurface(terminal)
    } catch {
      // Silently fail if fit fails
    }
  }

  // Focus terminal
  const focusTerminal = () => {
    if (isDisposedRef.current) return
    setTimeout(() => {
      if (!isDisposedRef.current) {
        terminalRef.current?.focus()
      }
    }, 0)
  }

  const requestTerminalRebuild = (delay = 180) => {
    if (isDisposedRef.current) return

    const now = Date.now()
    if (now - lastRebuildAtRef.current < 1500) return

    if (rebuildTimerRef.current !== null) {
      window.clearTimeout(rebuildTimerRef.current)
    }

    rebuildTimerRef.current = window.setTimeout(() => {
      rebuildTimerRef.current = null
      lastRebuildAtRef.current = Date.now()
      setTerminalEpoch((value) => value + 1)
    }, delay)
  }

  // Handle resize
  useEffect(() => {
    fitTerminal()
    focusTerminal()
    requestTerminalRebuild(220)
  }, [fitNonce])

  // Initialize terminal
  useEffect(() => {
    if (!terminalHostRef.current || terminalRef.current) return

    // Reset disposal flag
    isDisposedRef.current = false

    const fitAddon = new FitAddon()
    fitAddonRef.current = fitAddon

    const terminal = new Terminal(createTerminalOptions(windowsBuildNumber))
    terminal.loadAddon(fitAddon)
    terminal.open(terminalHostRef.current)

    // Configure terminal DOM
    const textarea = terminal.textarea
    if (textarea) {
      textarea.setAttribute('aria-label', `Terminal ${tab.label}`)
      textarea.setAttribute('spellcheck', 'false')
      textarea.style.position = 'absolute'
      textarea.style.opacity = '0'
    }

    // Subscribe to output
    const unsubscribe = subscribeToTabOutput(tab.id, (data: string) => {
      if (isDisposedRef.current) return
      writeQueueRef.current.push(data)
      scheduleWriteFlush()
    })

    // Handle input
    inputDisposableRef.current = terminal.onData((data) => {
      if (isDisposedRef.current) return
      window.sentinel.sendTabInput(tab.id, data)
    })

    terminalRef.current = terminal

    // Initial fit
    setTimeout(() => {
      fitTerminal()
      focusTerminal()
    }, 50)

    return () => {
      // Set disposal flag FIRST to stop any incoming data
      isDisposedRef.current = true

      // Null the ref immediately so a concurrent remount never sees a stale instance
      terminalRef.current = null

      // Cancel any pending animation frame writes
      if (writeFrameRef.current !== null) {
        cancelAnimationFrame(writeFrameRef.current)
        writeFrameRef.current = null
      }
      if (rebuildTimerRef.current !== null) {
        window.clearTimeout(rebuildTimerRef.current)
        rebuildTimerRef.current = null
      }

      // Drain the write queue and reset in-flight flag before dispose
      writeQueueRef.current = []
      writeInFlightRef.current = false

      // Clean up subscriptions
      unsubscribe()
      inputDisposableRef.current?.dispose()
      inputDisposableRef.current = null

      // Dispose fit addon first
      try {
        fitAddon.dispose()
      } catch {
        // Ignore errors during disposal
      }
      fitAddonRef.current = null

      // Dispose terminal last (write queue is already empty, no callbacks can fire)
      try {
        terminal.dispose()
      } catch {
        // Ignore errors during disposal
      }
    }
  }, [tab.id, terminalEpoch])

  const handleWheel = (event: React.WheelEvent) => {
    if (isDisposedRef.current) return
    const terminal = terminalRef.current
    if (!terminal || event.deltaY === 0) return

    const lines = Math.round(event.deltaY / 40)
    terminal.scrollLines(lines)
  }

  return (
    <div className="flex flex-col h-full w-full bg-[#0b1219] border border-white/10">
      {/* Header */}
      <div className="flex items-center justify-between px-2 h-7 border-b border-white/10 bg-black/20">
        <div className="flex items-center gap-2">
          <span className="text-[10px] font-medium text-white/90">{tab.label}</span>
          {tab.status === 'starting' && (
            <span className="h-1.5 w-1.5 rounded-full bg-sentinel-accent animate-pulse" />
          )}
        </div>
        <div className="flex items-center gap-1">
          {!hideMaximize && (
            <button
              onClick={() => setIsMaximized(!isMaximized)}
              className="p-1 text-sentinel-mist hover:text-white hover:bg-white/10 transition-colors"
              title={isMaximized ? 'Minimize' : 'Maximize'}
            >
              {isMaximized ? (
                <Minimize2 className="h-3 w-3" />
              ) : (
                <Maximize2 className="h-3 w-3" />
              )}
            </button>
          )}
          <button
            onClick={onClose}
            className="p-1 text-sentinel-mist hover:text-white hover:bg-red-500/20 transition-colors"
            title="Close terminal"
          >
            <X className="h-3 w-3" />
          </button>
        </div>
      </div>

      {/* Terminal Content */}
      <div className="flex-1 min-h-0 overflow-hidden p-1">
        <div
          ref={terminalHostRef}
          className="h-full w-full terminal-host"
          onWheel={handleWheel}
        />
      </div>

      {/* Footer with info */}
      <div className="flex items-center justify-between px-2 h-5 border-t border-white/10 bg-black/20 text-[9px] text-sentinel-mist">
        <div className="flex items-center gap-2">
          <span>{tab.cwd}</span>
          {tab.pid && <span className="text-white/50">PID: {tab.pid}</span>}
        </div>
        <div className="flex items-center gap-2">
          {tab.metrics.cpuPercent > 0 && (
            <span className="text-sentinel-ice">{tab.metrics.cpuPercent.toFixed(1)}% CPU</span>
          )}
          {tab.metrics.memoryMb > 0 && (
            <span className="text-sentinel-accent">{tab.metrics.memoryMb.toFixed(1)} MB</span>
          )}
        </div>
      </div>
    </div>
  )
}
