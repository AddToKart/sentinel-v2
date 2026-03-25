import type { TabOutputEvent } from '@shared/types'

type OutputListener = (data: string) => void

interface OutputBuffer {
  chunks: string[]
  totalLength: number
  generation: number
}

interface OutputStore {
  clear: (key: string) => void
  push: (key: string, data: string) => void
  subscribe: (key: string, listener: OutputListener, replay?: boolean) => () => void
}

interface TabStreamState {
  store: OutputStore
  unsubscribe: (() => void) | null
}

const MAX_BUFFER_LENGTH = 500_000

declare global {
  var __sentinelTabStreamState__: TabStreamState | undefined
}

function createOutputStore(): OutputStore {
  const buffers = new Map<string, OutputBuffer>()
  const listeners = new Map<string, Set<OutputListener>>()
  const generations = new Map<string, number>() // Track generation for each key

  function getBuffer(key: string): OutputBuffer | undefined {
    const currentGen = generations.get(key) ?? 0
    let buffer = buffers.get(key)

    if (!buffer || buffer.generation !== currentGen) {
      buffer = { chunks: [], totalLength: 0, generation: currentGen }
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
      // Increment generation to invalidate existing buffer
      const currentGen = generations.get(key) ?? 0
      generations.set(key, currentGen + 1)
      buffers.delete(key)
      listeners.delete(key)
    },
    push(key: string, data: string) {
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
          console.error('Tab output listener error:', e)
        }
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
          try {
            listener(replayData)
          } catch (e) {
            // Ignore listener errors
            console.error('Tab output listener error:', e)
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

const state = globalThis.__sentinelTabStreamState__ ??= {
  store: createOutputStore(),
  unsubscribe: null
}

function ensureTabBridge(): void {
  if (state.unsubscribe) return
  if (typeof window === 'undefined' || typeof window.sentinel === 'undefined') return

  state.unsubscribe = window.sentinel.onTabOutput((event: TabOutputEvent) => {
    state.store.push(event.tabId, event.data)
  })
}

export function subscribeToTabOutput(
  tabId: string,
  listener: OutputListener,
  options: { replay?: boolean } = {}
): () => void {
  ensureTabBridge()
  return state.store.subscribe(tabId, listener, options.replay ?? true)
}

export function clearTabOutput(tabId: string): void {
  state.store.clear(tabId)
}

ensureTabBridge()
