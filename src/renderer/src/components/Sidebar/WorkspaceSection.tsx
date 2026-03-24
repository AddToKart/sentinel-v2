import { useEffect, useState } from 'react'
import { createPortal } from 'react-dom'
import { GitBranch, MoreHorizontal, Pause, Square, Trash2 } from 'lucide-react'

import type { ProjectState } from '@shared/types'

import { SidebarSection } from './SidebarSection'

interface WorkspaceSectionProps {
  collapsed: boolean
  modifiedFileCount: number
  project: ProjectState
  onOpenProject: () => void
  onWorkspaceAction: (action: 'delete' | 'stop' | 'pause') => void
}

export function WorkspaceSection({
  collapsed,
  modifiedFileCount,
  project,
  onOpenProject,
  onWorkspaceAction
}: WorkspaceSectionProps): JSX.Element {
  const [expanded, setExpanded] = useState(true)
  const [workspaceMenuOpen, setWorkspaceMenuOpen] = useState(false)
  const [menuPosition, setMenuPosition] = useState<{ top: number; left: number } | null>(null)

  useEffect(() => {
    setExpanded(true)
    setWorkspaceMenuOpen(false)
  }, [project.path])

  useEffect(() => {
    if (collapsed) {
      setWorkspaceMenuOpen(false)
    }
  }, [collapsed])

  return (
    <SidebarSection
      expanded={expanded}
      meta={project.path ? 'Active' : 'Idle'}
      onToggle={() => setExpanded((current) => !current)}
      title="Workspace"
    >
      <div className="relative pt-1">
        <button
          className="group flex w-full flex-col rounded-md border border-white/[0.08] bg-white/[0.02] p-3 text-left transition-all hover:border-white/20 hover:bg-white/[0.05] active:bg-white/[0.08] focus:outline-none focus:ring-1 focus:ring-sentinel-accent/50"
          onClick={onOpenProject}
        >
          <div className="flex w-full min-w-0 items-start justify-between">
            <div className="truncate text-sm font-semibold text-white transition-colors group-hover:text-sentinel-ice">
              {project.name || 'Select a Repository'}
            </div>
            {project.path && (
              <div className="relative">
                <button
                  className={`inline-flex h-6 w-6 items-center justify-center rounded transition-colors ${
                    workspaceMenuOpen ? 'bg-white/20 text-white' : 'text-sentinel-mist hover:bg-white/10 hover:text-white'
                  }`}
                  onClick={(event) => {
                    event.stopPropagation()
                    const rect = event.currentTarget.getBoundingClientRect()
                    setMenuPosition({ top: rect.bottom + 8, left: rect.right - 192 })
                    setWorkspaceMenuOpen((current) => !current)
                  }}
                  type="button"
                >
                  <MoreHorizontal className="h-4 w-4" />
                </button>

                {workspaceMenuOpen && menuPosition && typeof document !== 'undefined' && createPortal(
                  <>
                    <div
                      className="fixed inset-0 z-40"
                      onClick={(event) => {
                        event.stopPropagation()
                        setWorkspaceMenuOpen(false)
                      }}
                    />
                    <div
                      className="fixed z-50 w-48 rounded-md border border-white/10 bg-[#0b1219] p-1 shadow-2xl backdrop-blur-2xl"
                      style={{
                        top: menuPosition.top,
                        left: menuPosition.left
                      }}
                    >
                      <button
                        className="flex w-full items-center justify-between gap-2 rounded px-2 py-1.5 text-xs text-white transition hover:bg-white/10"
                        onClick={(event) => {
                          event.stopPropagation()
                          setWorkspaceMenuOpen(false)
                          onWorkspaceAction('pause')
                        }}
                        type="button"
                      >
                        Pause Workspace
                        <Pause className="h-3.5 w-3.5 text-sentinel-mist" />
                      </button>
                      <button
                        className="flex w-full items-center justify-between gap-2 rounded px-2 py-1.5 text-xs text-rose-300 transition hover:bg-rose-500/20"
                        onClick={(event) => {
                          event.stopPropagation()
                          setWorkspaceMenuOpen(false)
                          onWorkspaceAction('stop')
                        }}
                        type="button"
                      >
                        Stop Workspace
                        <Square className="h-3.5 w-3.5" />
                      </button>
                      <div className="my-1 h-px bg-white/10" />
                      <button
                        className="flex w-full items-center justify-between gap-2 rounded px-2 py-1.5 text-xs text-red-500 transition hover:bg-red-500/20"
                        onClick={(event) => {
                          event.stopPropagation()
                          setWorkspaceMenuOpen(false)
                          onWorkspaceAction('delete')
                        }}
                        type="button"
                      >
                        Delete Workspace
                        <Trash2 className="h-3.5 w-3.5" />
                      </button>
                    </div>
                  </>,
                  document.body
                )}
              </div>
            )}
          </div>

          <div className="mt-2 max-w-[200px] truncate text-xs text-sentinel-mist/60" title={project.path}>
            {project.path || 'Click to browse for a project'}
          </div>

          <div className="mt-3 flex flex-wrap items-center gap-1.5 text-[10px] font-medium uppercase tracking-[0.2em] text-sentinel-mist">
            <span className={`rounded-sm px-1.5 py-0.5 ${project.path ? 'bg-sentinel-accent/15 text-sentinel-accent' : 'bg-white/[0.04]'}`}>
              {project.path ? 'active' : 'idle'}
            </span>
            {modifiedFileCount > 0 && (
              <span className="rounded-sm border border-amber-500/30 bg-amber-500/15 px-1.5 py-0.5 text-amber-300">
                {modifiedFileCount} uncommitted
              </span>
            )}
            {project.branch && (
              <span className="inline-flex items-center gap-1 rounded-sm border border-sentinel-ice/20 bg-sentinel-ice/10 px-1.5 py-0.5 text-sentinel-ice">
                <GitBranch className="h-3 w-3" />
                {project.branch}
              </span>
            )}
          </div>
        </button>
      </div>
    </SidebarSection>
  )
}
