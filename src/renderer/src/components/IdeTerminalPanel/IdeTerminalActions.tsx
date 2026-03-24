import { createPortal } from 'react-dom'
import { CheckCheck, LoaderCircle, RefreshCw, RotateCcw, TerminalSquare, X } from 'lucide-react'

import type { IdeTerminalState } from '@shared/types'

import { describeState } from './helpers'

interface IdeTerminalActionsProps {
  actionsTarget?: HTMLDivElement | null
  connecting: boolean
  onClose?: () => void
  onRecoverOrReconnect: () => void
  onRunWorkspaceOp: (op: 'apply' | 'discard') => void
  operationLoading: 'apply' | 'discard' | null
  projectPath?: string
  terminalState: IdeTerminalState
}

export function IdeTerminalActions({
  actionsTarget,
  connecting,
  onClose,
  onRecoverOrReconnect,
  onRunWorkspaceOp,
  operationLoading,
  projectPath,
  terminalState
}: IdeTerminalActionsProps): JSX.Element {
  const actions = (
    <>
      {terminalState.modifiedPaths.length > 0 && (
        <span className="mr-1 text-[10px] uppercase tracking-[0.2em] text-amber-300/80">
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
        onClick={() => onRunWorkspaceOp('apply')}
        title="Apply IDE workspace to main project"
        type="button"
      >
        <CheckCheck className="h-3.5 w-3.5" />
      </button>
      <button
        className="inline-flex h-5 w-5 items-center justify-center text-sentinel-mist/60 transition hover:text-rose-300 disabled:opacity-30"
        disabled={terminalState.modifiedPaths.length === 0 || operationLoading !== null}
        onClick={() => onRunWorkspaceOp('discard')}
        title="Reset IDE workspace"
        type="button"
      >
        <RotateCcw className="h-3.5 w-3.5" />
      </button>
      <button
        className="inline-flex h-5 w-5 items-center justify-center text-sentinel-mist/60 transition hover:text-white"
        onClick={onRecoverOrReconnect}
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
          <X className="h-3.5 w-3.5" />
        </button>
      )}
    </>
  )

  if (actionsTarget) {
    return createPortal(actions, actionsTarget)
  }

  return (
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
  )
}
