import { IdeTerminalActions } from './IdeTerminalActions'
import type { IdeTerminalPanelProps } from './types'
import { useIdeTerminalRuntime } from './useIdeTerminalRuntime'

export function IdeTerminalPanel({
  fitNonce,
  projectPath,
  terminalState,
  windowsBuildNumber,
  onClose,
  actionsTarget,
  isVisible = false
}: IdeTerminalPanelProps): JSX.Element {
  const runtime = useIdeTerminalRuntime({
    fitNonce,
    isVisible,
    projectPath,
    terminalState,
    windowsBuildNumber
  })

  if (!projectPath) {
    return (
      <div className="flex h-full items-center justify-center border-t border-white/10 bg-[#060a0f] text-sm text-sentinel-mist">
        Open a project folder to start the IDE terminal.
      </div>
    )
  }

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-col overflow-hidden border-t border-white/10 bg-[#060a0f]">
      <IdeTerminalActions
        actionsTarget={actionsTarget}
        connecting={runtime.connecting}
        onClose={onClose}
        onRecoverOrReconnect={() => { void runtime.recoverOrReconnect() }}
        onRunWorkspaceOp={(op) => { void runtime.runWorkspaceOp(op) }}
        operationLoading={runtime.operationLoading}
        projectPath={projectPath}
        terminalState={runtime.terminalState}
      />

      <div
        className="terminal-host h-full min-h-0 w-full overflow-hidden"
        onMouseDown={runtime.focusTerminal}
        onWheel={runtime.handleWheel}
        ref={runtime.terminalHostRef}
      />
    </div>
  )
}
