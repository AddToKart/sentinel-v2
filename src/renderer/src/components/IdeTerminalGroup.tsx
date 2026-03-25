import { useState } from 'react'
import { Plus, TerminalSquare } from 'lucide-react'
import type { IdeTerminalState, TabSummary } from '@shared/types'
import { IdeTerminalPanel } from './IdeTerminalPanel'
import { StandaloneTerminalTile } from './StandaloneTerminalTile'

interface IdeTerminalGroupProps {
  projectPath?: string
  windowsBuildNumber?: number
  fitNonce: number
  ideTerminalState: IdeTerminalState
  tabs: TabSummary[]
  activeTerminalId: string
  onSelectTerminal: (id: string) => void
  onCreateTerminal: () => void
  onCloseTerminal: (id: string, e: React.MouseEvent) => void
  onToggleCollapse: () => void
}

export function IdeTerminalGroup({
  projectPath,
  windowsBuildNumber,
  fitNonce,
  ideTerminalState,
  tabs,
  activeTerminalId,
  onSelectTerminal,
  onCreateTerminal,
  onCloseTerminal,
  onToggleCollapse
}: IdeTerminalGroupProps): JSX.Element {
  const isValidTab = activeTerminalId === 'ide-workspace' || tabs.some(t => t.id === activeTerminalId)
  const displayId = isValidTab ? activeTerminalId : 'ide-workspace'
  const [actionsTarget, setActionsTarget] = useState<HTMLDivElement | null>(null)

  return (
    <div className="flex h-full w-full bg-[#0d1117] overflow-hidden">
      {/* Left: Active Terminal Content — ALL terminals stay mounted to preserve the xterm
          WebGL canvas. Only visibility is toggled to prevent canvas destroy/recreate
          on tab switch, which caused display corruption. */}
      <div className="flex-1 min-w-0 h-full relative border-r border-white/10">
        {/* IDE Workspace terminal — always mounted */}
        <div
          className="absolute inset-0"
          style={{ zIndex: displayId === 'ide-workspace' ? 10 : 0, visibility: displayId === 'ide-workspace' ? 'visible' : 'hidden', pointerEvents: displayId === 'ide-workspace' ? 'auto' : 'none' }}
          aria-hidden={displayId !== 'ide-workspace'}
        >
          <IdeTerminalPanel
            fitNonce={fitNonce}
            projectPath={projectPath}
            terminalState={ideTerminalState}
            windowsBuildNumber={windowsBuildNumber}
            onClose={onToggleCollapse}
            actionsTarget={actionsTarget}
            isVisible={displayId === 'ide-workspace'}
          />
        </div>

        {/* Standalone tab terminals — all tabs stay mounted */}
        {tabs.map((tab) => (
          <div
            key={tab.id}
            className="absolute inset-0"
            style={{
              zIndex: displayId === tab.id ? 10 : 0,
              visibility: displayId === tab.id ? 'visible' : 'hidden',
              pointerEvents: displayId === tab.id ? 'auto' : 'none'
            }}
            aria-hidden={displayId !== tab.id}
          >
            <StandaloneTerminalTile
              tab={tab}
              fitNonce={fitNonce}
              windowsBuildNumber={windowsBuildNumber}
              onClose={() => onCloseTerminal(tab.id, {} as any)}
              hideMaximize={true}
            />
          </div>
        ))}
      </div>

      {/* Right: Vertical Tab Sidebar (VS Code style) */}
      <div className="w-48 shrink-0 flex flex-col bg-[#080d14]">
        <div className="flex items-center justify-between px-3 py-2 border-b border-white/10">
          <span className="text-[10px] font-bold uppercase tracking-widest text-sentinel-mist">Terminals</span>
          <div className="flex items-center gap-1">
            <div ref={setActionsTarget} className="flex items-center gap-1" />
            <button
              onClick={onCreateTerminal}
              className="p-1 text-sentinel-mist hover:text-white transition-colors rounded hover:bg-white/10"
              title="New Terminal"
            >
              <Plus className="h-3.5 w-3.5" />
            </button>
          </div>
        </div>

        <div className="flex-1 overflow-y-auto min-h-0 py-1">
          {/* Main IDE Workspace */}
          <button
            onClick={() => onSelectTerminal('ide-workspace')}
            className={`w-full flex items-center justify-between px-3 py-1.5 text-xs transition-colors group ${
              displayId === 'ide-workspace'
                ? 'bg-sentinel-accent/15 text-white border-l-2 border-sentinel-accent'
                : 'text-sentinel-mist border-l-2 border-transparent hover:bg-white/[0.04]'
            }`}
          >
            <div className="flex items-center gap-2 truncate">
              <TerminalSquare className="h-3.5 w-3.5 shrink-0" />
              <span className="truncate">IDE Workspace</span>
            </div>
          </button>

          {/* Standalone Terminals */}
          {tabs.map((tab, idx) => (
            <button
              key={tab.id}
              onClick={() => onSelectTerminal(tab.id)}
              className={`w-full flex items-center justify-between px-3 py-1.5 text-xs transition-colors group ${
                displayId === tab.id
                  ? 'bg-sentinel-ice/10 text-white border-l-2 border-sentinel-ice'
                  : 'text-sentinel-mist border-l-2 border-transparent hover:bg-white/[0.04]'
              }`}
            >
              <div className="flex items-center gap-2 truncate pr-2">
                <TerminalSquare className="h-3.5 w-3.5 shrink-0" />
                <span className="truncate">{tab.label || `Terminal ${idx + 1}`}</span>
              </div>
            </button>
          ))}
        </div>
      </div>
    </div>
  )
}
