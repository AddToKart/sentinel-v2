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
  // Ensure the active tab actually exists, otherwise fallback to ide-workspace
  const isValidTab = activeTerminalId === 'ide-workspace' || tabs.some(t => t.id === activeTerminalId)
  const displayId = isValidTab ? activeTerminalId : 'ide-workspace'
  const activeTab = displayId === 'ide-workspace'
    ? null
    : tabs.find((tab) => tab.id === displayId) ?? null
  const [actionsTarget, setActionsTarget] = useState<HTMLDivElement | null>(null)

  return (
    <div className="flex h-full w-full bg-[#0d1117] overflow-hidden">
      {/* Left: Active Terminal Content */}
      <div className="flex-1 min-w-0 h-full relative border-r border-white/10">
        {displayId === 'ide-workspace' ? (
          <div className="absolute inset-0 z-10">
            <IdeTerminalPanel
              fitNonce={fitNonce}
              projectPath={projectPath}
              terminalState={ideTerminalState}
              windowsBuildNumber={windowsBuildNumber}
              onClose={onToggleCollapse}
              actionsTarget={actionsTarget}
              isVisible
            />
          </div>
        ) : activeTab ? (
          <div className="absolute inset-0 z-10">
            <StandaloneTerminalTile
              key={activeTab.id}
              tab={activeTab}
              fitNonce={fitNonce}
              windowsBuildNumber={windowsBuildNumber}
              onClose={() => onCloseTerminal(activeTab.id, {} as any)}
              hideMaximize={true}
            />
          </div>
        ) : null}
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
