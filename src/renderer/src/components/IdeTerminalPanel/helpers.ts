import type { IdeTerminalState } from '@shared/types'

export function createIdleState(projectPath?: string): IdeTerminalState {
  return {
    status: 'idle',
    shell: 'powershell.exe',
    cwd: projectPath,
    modifiedPaths: []
  }
}

export function describeState(state: IdeTerminalState): string {
  if (state.status === 'idle') return 'idle'
  if (state.status === 'starting') return 'starting'
  if (state.status === 'closing') return 'closing'
  if (state.status === 'error') return 'shell error'
  if (state.status === 'closed') return 'closed'
  return 'ready'
}
