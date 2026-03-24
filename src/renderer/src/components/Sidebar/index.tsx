import { ChevronLeft, RefreshCw } from 'lucide-react'

import { FilesSection } from './FilesSection'
import { ModesSection } from './ModesSection'
import { WorkspaceSection } from './WorkspaceSection'
import type { SidebarProps } from './types'

export function Sidebar({
  collapsed,
  defaultSessionStrategy,
  diffBadges,
  globalMode,
  onChangeDefaultSessionStrategy,
  onFileSelect,
  onOpenProject,
  onRefreshProject,
  onToggleCollapse,
  onToggleGlobalMode,
  onWorkspaceAction,
  overlayFiles,
  project,
  refreshing,
  selectedFileProjectPath
}: SidebarProps): JSX.Element {
  const modifiedFileCount = Object.values(diffBadges).filter((badges) => badges.length > 0).length

  return (
    <aside
      aria-hidden={collapsed}
      className={`relative flex h-full min-h-0 flex-col overflow-hidden border-r bg-[#081018]/96 transition-[opacity,transform,filter,border-color] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)] ${
        collapsed
          ? 'pointer-events-none border-transparent opacity-0 -translate-x-6 blur-[2px]'
          : 'border-white/10 opacity-100 translate-x-0 blur-0'
      }`}
    >
      <div className="pointer-events-none absolute inset-y-0 left-0 w-px bg-gradient-to-b from-sentinel-accent/20 via-sentinel-ice/10 to-transparent" />
      <div className="pointer-events-none absolute inset-x-0 top-0 h-16 bg-gradient-to-b from-white/[0.05] to-transparent opacity-70" />

      <div className="flex shrink-0 items-center justify-between border-b border-black/40 bg-black/20 px-4 py-3">
        <div className="text-[11px] font-bold uppercase tracking-[0.28em] text-white/50">Explorer</div>
        <div className="flex shrink-0 items-center gap-1.5">
          <button
            className="inline-flex h-7 w-7 items-center justify-center rounded border border-white/10 bg-white/[0.04] text-white transition hover:border-white/20 hover:bg-white/[0.08] active:bg-white/10"
            onClick={onRefreshProject}
            title="Refresh tree"
            type="button"
          >
            <RefreshCw className={`h-3.5 w-3.5 ${refreshing ? 'animate-spin' : ''}`} />
          </button>
          <button
            className="inline-flex h-7 w-7 items-center justify-center rounded border border-white/10 bg-white/[0.04] text-white transition hover:border-white/20 hover:bg-white/[0.08] active:bg-white/10"
            onClick={onToggleCollapse}
            title="Collapse sidebar"
            type="button"
          >
            <ChevronLeft className="h-3 w-3" />
          </button>
        </div>
      </div>

      <WorkspaceSection
        collapsed={collapsed}
        modifiedFileCount={modifiedFileCount}
        onOpenProject={onOpenProject}
        onWorkspaceAction={onWorkspaceAction}
        project={project}
      />

      <ModesSection
        defaultSessionStrategy={defaultSessionStrategy}
        globalMode={globalMode}
        onChangeDefaultSessionStrategy={onChangeDefaultSessionStrategy}
        onToggleGlobalMode={onToggleGlobalMode}
        project={project}
      />

      <FilesSection
        collapsed={collapsed}
        diffBadges={diffBadges}
        onFileSelect={onFileSelect}
        overlayFiles={overlayFiles}
        project={project}
        selectedFileProjectPath={selectedFileProjectPath}
      />
    </aside>
  )
}
