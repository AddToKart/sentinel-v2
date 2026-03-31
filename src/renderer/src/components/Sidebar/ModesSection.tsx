import { useEffect, useState } from 'react'
import { Copy, GitFork } from 'lucide-react'

import type { ProjectState, SessionWorkspaceStrategy } from '@shared/types'

import { SidebarSection } from './SidebarSection'
import { toggleButtonClasses } from './sidebar-utils'

interface ModesSectionProps {
  defaultSessionStrategy: SessionWorkspaceStrategy
  globalMode: 'multiplex' | 'ide'
  project: ProjectState
  onChangeDefaultSessionStrategy: (strategy: SessionWorkspaceStrategy) => void
  onToggleGlobalMode: (mode: 'multiplex' | 'ide') => void
}

export function ModesSection({
  defaultSessionStrategy,
  globalMode,
  project,
  onChangeDefaultSessionStrategy,
  onToggleGlobalMode
}: ModesSectionProps): JSX.Element {
  const [expanded, setExpanded] = useState(false)

  useEffect(() => {
    setExpanded(false)
  }, [project.path])

  return (
    <SidebarSection
      expanded={expanded}
      meta={globalMode === 'ide' ? 'ide' : 'agents'}
      onToggle={() => setExpanded((current) => !current)}
      title="Modes"
    >
      <div className="space-y-4">
        <div>
          <div className="text-[10px] font-semibold uppercase tracking-[0.22em] text-sentinel-mist">Session Workspace</div>
          <div className="mt-2 grid grid-cols-2 gap-2">
            <button
              className={`flex items-center gap-2 border px-3 py-2 text-left text-[11px] font-semibold uppercase tracking-[0.2em] transition ${toggleButtonClasses(defaultSessionStrategy === 'sandbox-copy')}`}
              onClick={() => onChangeDefaultSessionStrategy('sandbox-copy')}
              type="button"
            >
              <Copy className="h-3.5 w-3.5 shrink-0 text-sentinel-accent" />
              Sandbox
            </button>

            <button
              className={`flex items-center gap-2 border px-3 py-2 text-left text-[11px] font-semibold uppercase tracking-[0.2em] transition ${toggleButtonClasses(defaultSessionStrategy === 'git-worktree')} ${project.isGitRepo ? '' : 'cursor-not-allowed opacity-50'}`}
              disabled={!project.isGitRepo}
              onClick={() => onChangeDefaultSessionStrategy('git-worktree')}
              type="button"
            >
              <GitFork className="h-3.5 w-3.5 shrink-0 text-sentinel-ice" />
              Worktree
            </button>
          </div>
        </div>

        <div>
          <div className="text-[10px] font-semibold uppercase tracking-[0.22em] text-sentinel-mist">View</div>
          <div className="mt-2 grid grid-cols-2 gap-2">
            <button
              className={`border px-3 py-2 text-[11px] font-semibold uppercase tracking-[0.2em] transition ${toggleButtonClasses(globalMode === 'multiplex')}`}
              onClick={() => onToggleGlobalMode('multiplex')}
              type="button"
            >
              Agents
            </button>

            <button
              className={`border px-3 py-2 text-[11px] font-semibold uppercase tracking-[0.2em] transition ${globalMode === 'ide'
                ? 'border-emerald-500/30 bg-emerald-500/12 text-white'
                : 'border-white/10 bg-white/[0.03] text-sentinel-mist hover:border-white/20 hover:bg-white/[0.05] hover:text-white'
              }`}
              onClick={() => onToggleGlobalMode('ide')}
              type="button"
            >
              IDE
            </button>
          </div>
        </div>
      </div>
    </SidebarSection>
  )
}
