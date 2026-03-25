import type { IdeTerminalOutputEvent, SessionOutputEvent } from '@shared/types'

type OutputListener = (data: string) => void

interface OutputBuffer {
  chunks: string[]
  totalLength: number
}

interface OutputStore {
  clear: (key: string) => void
  connect: (key: string, listener: OutputListener) => { replayData: string; unsubscribe: () => void }
  push: (key: string, data: string) => void
  subscribe: (key: string, listener: OutputListener, replay?: boolean) => () => void
}

interface TerminalStreamState {
  ideUnsubscribe: (() => void) | null
  sessionUnsubscribe: (() => void) | null
  store: OutputStore
}

const MAX_BUFFER_LENGTH = 500_000
const IDE_TERMINAL_KEY = 'ide-terminal'

declare global {
  var __sentinelTerminalStreamState__: TerminalStreamState | undefined
}

function createOutputStore(): OutputStore {
  const buffers = new Map<string, OutputBuffer>()
  const listeners = new Map<string, Set<OutputListener>>()

  function getBuffer(key: string): OutputBuffer {
    let buffer = buffers.get(key)
    if (!buffer) {
      buffer = { chunks: [], totalLength: 0 }
      buffers.set(key, buffer)
    }

    return buffer
  }

  function trimBuffer(buffer: OutputBuffer): void {
    while (buffer.totalLength > MAX_BUFFER_LENGTH && buffer.chunks.length > 1) {
      const removed = buffer.chunks.shift()
      if (!removed) {
        break
      }

      buffer.totalLength -= removed.length
    }
  }

  return {
    clear(key: string) {
      buffers.delete(key)
      listeners.delete(key)
    },
    connect(key: string, listener: OutputListener) {
      let scopedListeners = listeners.get(key)
      if (!scopedListeners) {
        scopedListeners = new Set()
        listeners.set(key, scopedListeners)
      }

      scopedListeners.add(listener)
      const replayData = buffers.get(key)?.chunks.join('') ?? ''

      return {
        replayData,
        unsubscribe: () => {
          const currentListeners = listeners.get(key)
          if (!currentListeners) {
            return
          }

          currentListeners.delete(listener)
          if (currentListeners.size === 0) {
            listeners.delete(key)
          }
        }
      }
    },
    push(key: string, data: string) {
      const buffer = getBuffer(key)
      buffer.chunks.push(data)
      buffer.totalLength += data.length
      trimBuffer(buffer)

      const scopedListeners = listeners.get(key)
      if (!scopedListeners || scopedListeners.size === 0) {
        return
      }

      for (const listener of scopedListeners) {
        listener(data)
      }
    },
    subscribe(key: string, listener: OutputListener, replay = true) {
      let scopedListeners = listeners.get(key)
      if (!scopedListeners) {
        scopedListeners = new Set()
        listeners.set(key, scopedListeners)
      }

      scopedListeners.add(listener)

      if (replay) {
        const replayData = buffers.get(key)?.chunks.join('')
        if (replayData) {
          listener(replayData)
        }
      }

      return () => {
        const currentListeners = listeners.get(key)
        if (!currentListeners) {
          return
        }

        currentListeners.delete(listener)
        if (currentListeners.size === 0) {
          listeners.delete(key)
        }
      }
    }
  }
}

const state = globalThis.__sentinelTerminalStreamState__ ??= {
  ideUnsubscribe: null,
  sessionUnsubscribe: null,
  store: createOutputStore()
}

function ensureSessionBridge(): void {
  if (state.sessionUnsubscribe) {
    return
  }

  if (typeof window === 'undefined' || typeof window.sentinel === 'undefined') {
    return
  }

  state.sessionUnsubscribe = window.sentinel.onSessionOutput((event: SessionOutputEvent) => {
    state.store.push(event.sessionId, event.data)
  })
}

function ensureIdeBridge(): void {
  if (state.ideUnsubscribe) {
    return
  }

  if (typeof window === 'undefined' || typeof window.sentinel === 'undefined') {
    return
  }

  state.ideUnsubscribe = window.sentinel.onIdeTerminalOutput((event: IdeTerminalOutputEvent) => {
    state.store.push(IDE_TERMINAL_KEY, event.data)
  })
}

export function subscribeToSessionOutput(
  sessionId: string,
  listener: OutputListener,
  options: { replay?: boolean } = {}
): () => void {
  ensureSessionBridge()
  return state.store.subscribe(sessionId, listener, options.replay ?? true)
}

export function attachSessionOutput(
  sessionId: string,
  listener: OutputListener
): { replayData: string; unsubscribe: () => void } {
  ensureSessionBridge()
  return state.store.connect(sessionId, listener)
}

export function clearSessionOutput(sessionId: string): void {
  state.store.clear(sessionId)
}

export function subscribeToIdeTerminalOutput(
  listener: OutputListener,
  options: { replay?: boolean } = {}
): () => void {
  ensureIdeBridge()
  return state.store.subscribe(IDE_TERMINAL_KEY, listener, options.replay ?? true)
}

export function attachIdeTerminalOutput(
  listener: OutputListener
): { replayData: string; unsubscribe: () => void } {
  ensureIdeBridge()
  return state.store.connect(IDE_TERMINAL_KEY, listener)
}

export function clearIdeTerminalOutput(): void {
  state.store.clear(IDE_TERMINAL_KEY)
}

ensureSessionBridge()
ensureIdeBridge()
