import type { IdeTerminalState } from '@shared/types'

export interface IdeTerminalPanelProps {
  fitNonce: number
  projectPath?: string
  terminalState: IdeTerminalState
  windowsBuildNumber?: number
  onClose?: () => void
  actionsTarget?: HTMLDivElement | null
  isVisible?: boolean
}
