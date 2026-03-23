import type { TabOutputEvent } from '@shared/types'

type OutputListener = (data: string) => void

interface OutputBuffer {
  chunks: string[]
  totalLength: number
}

interface OutputStore {
  clear: (key: string) => void
  push: (key: string, data: string) => void
  subscribe: (key: string, listener: OutputListener, replay?: boolean) => () => void
}

const MAX_BUFFER_LENGTH = 500_000

function createOutputStore(): OutputStore {
  const buffers = new Map<string, OutputBuffer>()
  const listeners = new Map<string, Set<OutputListener>>()
  const clearedKeys = new Set<string>() // Track recently cleared keys

  function getBuffer(key: string): OutputBuffer | undefined {
    // Don't recreate buffers for recently cleared keys
    if (clearedKeys.has(key)) return undefined
    
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
      if (!removed) break
      buffer.totalLength -= removed.length
    }
  }

  return {
    clear(key: string) {
      // Mark as cleared to prevent race conditions
      clearedKeys.add(key)
      buffers.delete(key)
      listeners.delete(key)
      
      // Remove from cleared set after a tick to allow new subscriptions
      setTimeout(() => {
        clearedKeys.delete(key)
      }, 0)
    },
    push(key: string, data: string) {
      // Don't recreate buffers for cleared keys
      if (clearedKeys.has(key)) return
      
      const buffer = getBuffer(key)
      if (!buffer) return
      
      buffer.chunks.push(data)
      buffer.totalLength += data.length
      trimBuffer(buffer)

      const scopedListeners = listeners.get(key)
      if (!scopedListeners || scopedListeners.size === 0) return

      for (const listener of scopedListeners) {
        try {
          listener(data)
        } catch (e) {
          // Ignore listener errors to prevent crashing
        }
      }
    },
    subscribe(key: string, listener: OutputListener, replay = true) {
      // Remove from cleared set if re-subscribing
      clearedKeys.delete(key)
      
      let scopedListeners = listeners.get(key)
      if (!scopedListeners) {
        scopedListeners = new Set()
        listeners.set(key, scopedListeners)
      }

      scopedListeners.add(listener)

      if (replay) {
        const replayData = buffers.get(key)?.chunks.join('')
        if (replayData) {
          try {
            listener(replayData)
          } catch (e) {
            // Ignore listener errors
          }
        }
      }

      return () => {
        const currentListeners = listeners.get(key)
        if (!currentListeners) return
        currentListeners.delete(listener)
        if (currentListeners.size === 0) {
          listeners.delete(key)
        }
      }
    }
  }
}

const store = createOutputStore()

let tabBridgeStarted = false

function ensureTabBridge(): void {
  if (tabBridgeStarted) return
  if (typeof window === 'undefined' || typeof window.sentinel === 'undefined') return

  tabBridgeStarted = true
  window.sentinel.onTabOutput((event: TabOutputEvent) => {
    store.push(event.tabId, event.data)
  })
}

export function subscribeToTabOutput(
  tabId: string,
  listener: OutputListener,
  options: { replay?: boolean } = {}
): () => void {
  ensureTabBridge()
  return store.subscribe(tabId, listener, options.replay ?? true)
}

export function clearTabOutput(tabId: string): void {
  store.clear(tabId)
}

ensureTabBridge()
