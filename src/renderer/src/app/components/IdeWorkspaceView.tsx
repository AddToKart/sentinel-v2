import { Group, Panel, PanelImperativeHandle, Separator } from 'react-resizable-panels'
import type { RefObject } from 'react'

import type { IdeTerminalState, TabSummary } from '@shared/types'

import { CodePreview } from '../../components/CodePreview'
import { IdeTerminalGroup } from '../../components/IdeTerminalGroup'
import type { SelectedFileEntry } from '../../workspace-overlay'

interface IdeWorkspaceViewProps {
  activeTerminalId: string
  fitNonce: number
  ideTerminalCollapsed: boolean
  ideTerminalPanelRef: RefObject<PanelImperativeHandle | null>
  ideTerminalState: IdeTerminalState
  projectPath?: string
  selectedFile: SelectedFileEntry | null
  tabs: TabSummary[]
  windowsBuildNumber?: number
  onCloseSelectedFile: () => void
  onCloseTerminal: (id: string) => Promise<void>
  onCreateTerminal: () => Promise<void>
  onSelectTerminal: (id: string) => void
  onToggleCollapse: () => void
}

export function IdeWorkspaceView({
  activeTerminalId,
  fitNonce,
  ideTerminalCollapsed,
  ideTerminalPanelRef,
  ideTerminalState,
  projectPath,
  selectedFile,
  tabs,
  windowsBuildNumber,
  onCloseSelectedFile,
  onCloseTerminal,
  onCreateTerminal,
  onSelectTerminal,
  onToggleCollapse
}: IdeWorkspaceViewProps): JSX.Element {
  return (
    <Group orientation="vertical">
      <Panel defaultSize={65} minSize={20} className="min-h-0">
        <CodePreview
          ideTerminalCollapsed={ideTerminalCollapsed}
          ideTerminalState={ideTerminalState}
          onClose={onCloseSelectedFile}
          onToggleIdeTerminal={onToggleCollapse}
          projectPath={projectPath}
          selectedFile={selectedFile}
        />
      </Panel>
      <Separator
        className={`relative bg-transparent transition-[height,opacity] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)] ${
          ideTerminalCollapsed
            ? 'pointer-events-none h-0 opacity-0'
            : 'h-[3px] opacity-100 hover:bg-sentinel-accent/20 active:bg-sentinel-accent/40 cursor-row-resize'
        }`}
      />
      <Panel
        panelRef={ideTerminalPanelRef}
        defaultSize={35}
        minSize={10}
        collapsible
        collapsedSize={0}
        className="min-h-0 transition-[flex-basis,height,max-height,min-height] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)]"
        style={{ overflow: 'hidden' }}
      >
        <IdeTerminalGroup
          activeTerminalId={activeTerminalId}
          fitNonce={fitNonce}
          ideTerminalState={ideTerminalState}
          onCloseTerminal={(id) => { void onCloseTerminal(id) }}
          onCreateTerminal={() => { void onCreateTerminal() }}
          onSelectTerminal={onSelectTerminal}
          onToggleCollapse={onToggleCollapse}
          projectPath={projectPath}
          tabs={tabs}
          windowsBuildNumber={windowsBuildNumber}
        />
      </Panel>
    </Group>
  )
}
