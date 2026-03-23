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
}

export function StandaloneTerminalTile({
  tab,
  fitNonce,
  onClose,
  windowsBuildNumber
}: StandaloneTerminalTileProps): JSX.Element {
  const terminalHostRef = useRef<HTMLDivElement | null>(null)
  const terminalRef = useRef<Terminal | null>(null)
  const fitAddonRef = useRef<FitAddon | null>(null)
  const inputDisposableRef = useRef<{ dispose: () => void } | null>(null)
  const writeQueueRef = useRef<string[]>([])
  const writeInFlightRef = useRef(false)
  const isDisposedRef = useRef(false)
  const [isMaximized, setIsMaximized] = useState(false)

  // Process write queue
  const processWriteQueue = () => {
    // Guard against processing after disposal
    if (isDisposedRef.current) {
      writeQueueRef.current = []
      return
    }

    const terminal = terminalRef.current
    if (!terminal || writeInFlightRef.current || writeQueueRef.current.length === 0) {
      return
    }

    const chunk = writeQueueRef.current.shift()
    if (!chunk) return

    writeInFlightRef.current = true
    terminal.write(chunk, () => {
      writeInFlightRef.current = false
      // Check disposal flag before recursing
      if (!isDisposedRef.current) {
        processWriteQueue()
      }
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

  // Handle resize
  useEffect(() => {
    fitTerminal()
    focusTerminal()
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
      processWriteQueue()
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
      // Set disposal flag FIRST to prevent any async operations
      isDisposedRef.current = true
      
      // Clear any pending writes
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
      
      // Dispose terminal
      try {
        terminal.dispose()
      } catch {
        // Ignore errors during disposal
      }
      terminalRef.current = null
    }
  }, [tab.id])

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
