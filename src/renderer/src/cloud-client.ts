import type {
  CreateSessionInput,
  SessionSummary,
  WorkspaceContext
} from '@shared/types'

export interface CloudClientOptions {
  url: string
  authToken: string
  onMessage?: (message: any) => void
  onOpen?: () => void
  onClose?: () => void
  onError?: (error: any) => void
}

export class CloudClient {
  private ws: WebSocket | null = null
  private connectPromise: Promise<void> | null = null
  private requests = new Map<string, { resolve: (data: any) => void; reject: (error: any) => void }>()
  private listeners = new Set<(message: any) => void>()

  constructor(private options: CloudClientOptions) {}

  isConnected(): boolean {
    return this.ws !== null && this.ws.readyState === WebSocket.OPEN
  }

  getUrl(): string {
    return this.options.url
  }

  async connect(timeoutMs = 2500): Promise<void> {
    if (this.isConnected()) {
      return
    }

    if (this.connectPromise) {
      return this.connectPromise
    }

    const wsUrl = new URL(this.options.url)
    wsUrl.protocol = wsUrl.protocol === 'https:' ? 'wss:' : 'ws:'
    if (wsUrl.pathname === '/') {
      wsUrl.pathname = '/ws'
    }
    wsUrl.searchParams.set('token', this.options.authToken)

    const targetUrl = wsUrl.toString()
    console.log('[sentinel-cloud] connecting to', targetUrl)

    this.connectPromise = new Promise<void>((resolve, reject) => {
      let settled = false
      let opened = false
      const timeout = window.setTimeout(() => {
        if (settled) {
          return
        }
        settled = true
        this.ws?.close()
        this.ws = null
        this.connectPromise = null
        reject(new Error(`Could not connect to Sentinel Cloud at ${this.options.url}. Start the backend and make sure the cloud key matches.`))
      }, timeoutMs)

      const socket = new WebSocket(targetUrl)
      this.ws = socket

      socket.onopen = () => {
        opened = true
        if (settled) {
          return
        }
        settled = true
        window.clearTimeout(timeout)
        this.connectPromise = null
        console.log('[sentinel-cloud] connected')
        this.options.onOpen?.()
        resolve()
      }

      socket.onclose = () => {
        console.log('[sentinel-cloud] disconnected')
        this.ws = null
        this.options.onClose?.()

        if (settled) {
          return
        }

        settled = true
        window.clearTimeout(timeout)
        this.connectPromise = null
        reject(new Error(
          opened
            ? 'The Sentinel Cloud connection was closed unexpectedly.'
            : `Could not connect to Sentinel Cloud at ${this.options.url}. Start the backend and make sure the cloud key matches.`
        ))
      }

      socket.onerror = (error) => {
        console.error('[sentinel-cloud] error', error)
        this.options.onError?.(error)
      }

      socket.onmessage = (event) => {
        try {
          const message = JSON.parse(event.data)
          this.handleMessage(message)
        } catch (error) {
          console.error('[sentinel-cloud] message parse error', error)
        }
      }
    })

    return this.connectPromise
  }

  disconnect(): void {
    this.ws?.close()
    this.ws = null
    this.connectPromise = null
  }

  private handleMessage(message: any): void {
    if (message.type === 'response' && message.requestId) {
      const request = this.requests.get(message.requestId)
      if (request) {
        if (message.ok) {
          request.resolve(message.data)
        } else {
          request.reject(new Error(message.error?.message || 'Unknown cloud error'))
        }
        this.requests.delete(message.requestId)
      }
    }

    for (const listener of this.listeners) {
      listener(message)
    }
    this.options.onMessage?.(message)
  }

  subscribe(listener: (message: any) => void): () => void {
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }

  async sendCommand<T = any>(type: string, payload?: any): Promise<T> {
    await this.connect()

    if (!this.isConnected()) {
      throw new Error(`Could not connect to Sentinel Cloud at ${this.options.url}. Start the backend and make sure the cloud key matches.`)
    }

    const requestId = Math.random().toString(36).substring(7)
    const envelope = { type, requestId, payload }

    return new Promise<T>((resolve, reject) => {
      this.requests.set(requestId, { resolve, reject })
      this.ws!.send(JSON.stringify(envelope))
    })
  }

  async createSession(input?: CreateSessionInput): Promise<SessionSummary> {
    return this.sendCommand('session.create', input)
  }

  async sendInput(sessionId: string, data: string): Promise<void> {
    return this.sendCommand('session.input', { sessionId, data })
  }

  async resizeSession(sessionId: string, cols: number, rows: number): Promise<void> {
    return this.sendCommand('session.resize', { sessionId, cols, rows })
  }

  async closeSession(sessionId: string): Promise<void> {
    return this.sendCommand('session.close', { sessionId })
  }

  async listSessions(workspaceId?: string): Promise<SessionSummary[]> {
    return this.sendCommand('session.list', { workspaceId })
  }

  async listWorkspaces(): Promise<WorkspaceContext[]> {
    return this.sendCommand('workspace.list')
  }

  async ensureWorkspace(input: { id: string; name: string; primaryCheckoutPath: string }): Promise<WorkspaceContext> {
    return this.sendCommand('workspace.ensure', input)
  }
}
