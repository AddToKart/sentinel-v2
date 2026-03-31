import { useEffect, useRef, useState } from 'react'
import type { CSSProperties } from 'react'
import { createPortal } from 'react-dom'
import {
  ChevronDown,
  FolderRoot,
  GitBranch,
  Layers3,
  MoreHorizontal,
  Pause,
  Plus,
  Search,
  Square,
  TerminalSquare,
  Trash2
} from 'lucide-react'

import type { WorkspaceContext } from '@shared/types'

interface WorkspaceSwitcherProps {
  workspaces: WorkspaceContext[]
  activeWorkspaceId: string | null
  runningSessionCounts: Record<string, number>
  sessionCounts: Record<string, number>
  tabCounts: Record<string, number>
  unreadNotificationCounts: Record<string, number>
  onCreateWorkspace: () => void
  onSwitchWorkspace: (workspaceId: string) => void
  onWorkspaceAction: (workspaceId: string, action: 'delete' | 'stop' | 'pause') => void
}

function matchesWorkspace(workspace: WorkspaceContext, query: string): boolean {
  const normalizedQuery = query.trim().toLowerCase()
  if (!normalizedQuery) {
    return true
  }

  return (
    workspace.name.toLowerCase().includes(normalizedQuery)
    || (workspace.project.path ?? '').toLowerCase().includes(normalizedQuery)
    || (workspace.project.branch ?? '').toLowerCase().includes(normalizedQuery)
  )
}

export function WorkspaceSwitcher({
  workspaces,
  activeWorkspaceId,
  runningSessionCounts,
  sessionCounts,
  tabCounts,
  unreadNotificationCounts,
  onCreateWorkspace,
  onSwitchWorkspace,
  onWorkspaceAction
}: WorkspaceSwitcherProps): JSX.Element {
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [openMenuId, setOpenMenuId] = useState<string | null>(null)
  const [menuPosition, setMenuPosition] = useState<{ top: number; left: number } | null>(null)
  const [panelStyle, setPanelStyle] = useState<CSSProperties>({
    top: 0,
    left: 0,
    width: 440
  })

  const rootRef = useRef<HTMLDivElement | null>(null)
  const triggerRef = useRef<HTMLButtonElement | null>(null)
  const searchInputRef = useRef<HTMLInputElement | null>(null)

  const activeWorkspace =
    workspaces.find((workspace) => workspace.id === activeWorkspaceId) ?? null
  const filteredWorkspaces = workspaces.filter((workspace) => matchesWorkspace(workspace, query))
  const activeRunningCount = activeWorkspaceId ? (runningSessionCounts[activeWorkspaceId] ?? 0) : 0
  const activeSessionCount = activeWorkspaceId ? (sessionCounts[activeWorkspaceId] ?? 0) : 0
  const activeTabCount = activeWorkspaceId ? (tabCounts[activeWorkspaceId] ?? 0) : 0
  const activeUnreadCount = activeWorkspaceId ? (unreadNotificationCounts[activeWorkspaceId] ?? 0) : 0

  useEffect(() => {
    if (!open) {
      setQuery('')
      return
    }

    function updatePosition(): void {
      const rect = triggerRef.current?.getBoundingClientRect()
      if (!rect) {
        return
      }

      const viewportPadding = 12
      const maxWidth = Math.min(560, window.innerWidth - viewportPadding * 2)
      const width = Math.max(Math.min(maxWidth, Math.max(rect.width + 36, 420)), 320)
      const left = Math.min(
        Math.max(viewportPadding, rect.left),
        window.innerWidth - width - viewportPadding
      )

      setPanelStyle({
        top: rect.bottom + 10,
        left,
        width
      })
    }

    function handleEscape(event: KeyboardEvent): void {
      if (event.key === 'Escape') {
        setOpen(false)
        triggerRef.current?.focus()
      }
    }

    updatePosition()
    const focusTimer = window.setTimeout(() => searchInputRef.current?.focus(), 30)

    window.addEventListener('resize', updatePosition)
    window.addEventListener('scroll', updatePosition, true)
    window.addEventListener('keydown', handleEscape)

    return () => {
      window.clearTimeout(focusTimer)
      window.removeEventListener('resize', updatePosition)
      window.removeEventListener('scroll', updatePosition, true)
      window.removeEventListener('keydown', handleEscape)
    }
  }, [open])

  return (
    <>
      <div className="min-w-0" ref={rootRef}>
        <button
          ref={triggerRef}
          aria-expanded={open}
          aria-haspopup="dialog"
          className={`group flex h-9 w-full min-w-0 items-stretch border px-0 text-left transition ${
            open
              ? 'border-sentinel-accent/35 bg-sentinel-accent/10 text-white'
              : 'border-white/10 bg-white/[0.04] text-sentinel-mist hover:border-white/20 hover:bg-white/[0.08] hover:text-white'
          }`}
          onClick={() => setOpen((current) => !current)}
          type="button"
        >
          <div className="flex w-10 shrink-0 items-center justify-center border-r border-white/10 bg-black/25">
            <Layers3 className="h-4 w-4 text-sentinel-accent" />
          </div>

          <div className="flex min-w-0 flex-1 items-center gap-3 px-3">
            <div className="min-w-0 flex-1">
              <div className="truncate text-[11px] font-semibold uppercase tracking-[0.18em] text-white/92">
                {activeWorkspace?.name ?? 'No Workspace'}
              </div>
              <div className="truncate text-[10px] text-sentinel-mist/80">
                {activeWorkspace?.project.path ?? 'Open a project to create a workspace'}
              </div>
            </div>

            <div className="hidden shrink-0 items-center gap-1 md:flex">
              {activeRunningCount > 0 && (
                <span className="workspace-running-badge inline-flex items-center gap-1 border border-emerald-300/20 bg-emerald-400/12 px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-[0.2em] text-emerald-100">
                  <span className="workspace-running-dot h-1.5 w-1.5 bg-emerald-200" />
                  running
                </span>
              )}
              {activeUnreadCount > 0 && (
                <span className="border border-amber-300/20 bg-amber-300/12 px-1.5 py-0.5 text-[9px] uppercase tracking-[0.18em] text-amber-100">
                  {activeUnreadCount} alert{activeUnreadCount === 1 ? '' : 's'}
                </span>
              )}
              {activeWorkspace && (
                <span className="border border-sky-300/20 bg-sky-300/10 px-1.5 py-0.5 text-[9px] uppercase tracking-[0.18em] text-sky-100">
                  {activeWorkspace.mode}
                </span>
              )}
              <span className="border border-white/10 bg-black/25 px-1.5 py-0.5 text-[9px] uppercase tracking-[0.18em] text-sentinel-mist/85">
                {activeSessionCount} sessions
              </span>
              <span className="border border-white/10 bg-black/25 px-1.5 py-0.5 text-[9px] uppercase tracking-[0.18em] text-sentinel-mist/85">
                {activeTabCount} tabs
              </span>
            </div>

            <ChevronDown
              className={`h-4 w-4 shrink-0 text-sentinel-mist transition-transform duration-200 ${open ? 'rotate-180 text-white' : 'group-hover:text-white'}`}
            />
          </div>
        </button>
      </div>

      {open && typeof document !== 'undefined' && createPortal(
        <>
          <div
            aria-hidden="true"
            className="fixed inset-0 z-[120] bg-black/10 backdrop-blur-[1px]"
            onClick={() => setOpen(false)}
          />

          <div
            aria-label="Workspace switcher"
            className="fixed z-[130] border border-white/10 bg-[#09131b]/98 shadow-[0_28px_90px_rgba(0,0,0,0.55)] backdrop-blur-2xl"
            role="dialog"
            style={{
              ...panelStyle,
              WebkitAppRegion: 'no-drag'
            } as CSSProperties}
          >
            <div className="border-b border-white/10 px-4 py-3">
              <div className="flex items-center justify-between gap-4">
                <div>
                  <div className="text-[10px] font-semibold uppercase tracking-[0.24em] text-sentinel-mist">
                    Workspaces
                  </div>
                  <div className="mt-1 text-sm text-white/90">
                    Switch projects without stopping detached sessions.
                  </div>
                </div>

                <div className="border border-white/10 bg-black/25 px-2 py-1 text-[10px] uppercase tracking-[0.18em] text-sentinel-mist/85">
                  {workspaces.length} total
                </div>
              </div>

              <label className="mt-3 flex items-center gap-2 border border-white/10 bg-black/30 px-3 py-2 text-sentinel-mist focus-within:border-sentinel-accent/35 focus-within:text-white">
                <Search className="h-4 w-4 shrink-0" />
                <input
                  ref={searchInputRef}
                  aria-label="Search workspaces"
                  className="min-w-0 flex-1 bg-transparent text-sm text-white outline-none placeholder:text-sentinel-mist/55"
                  onChange={(event) => setQuery(event.target.value)}
                  placeholder="Filter by name, path, or branch"
                  type="search"
                  value={query}
                />
              </label>
            </div>

            <div className="max-h-[420px] overflow-auto p-2">
              {filteredWorkspaces.length === 0 ? (
                <div className="border border-dashed border-white/10 bg-white/[0.02] px-4 py-8 text-center text-sm text-sentinel-mist">
                  No workspaces match the current filter.
                </div>
              ) : (
                <div className="space-y-2">
                  {filteredWorkspaces.map((workspace) => {
                    const active = workspace.id === activeWorkspaceId
                    const runningCount = runningSessionCounts[workspace.id] ?? 0
                    const workspaceSessionCount = sessionCounts[workspace.id] ?? workspace.sessionIds.length
                    const workspaceTabCount = tabCounts[workspace.id] ?? workspace.tabIds.length
                    const unreadCount = unreadNotificationCounts[workspace.id] ?? 0

                    return (
                      <div
                        key={workspace.id}
                        className={`grid grid-cols-[minmax(0,1fr)_auto] gap-2 border ${
                          active
                            ? 'border-sentinel-accent/35 bg-sentinel-accent/10'
                            : 'border-white/10 bg-white/[0.03]'
                        }`}
                      >
                        <button
                          className="flex min-w-0 items-start gap-3 px-3 py-3 text-left transition hover:bg-white/[0.04]"
                          onClick={() => {
                            setOpen(false)
                            onSwitchWorkspace(workspace.id)
                          }}
                          type="button"
                        >
                          <div className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center border border-white/10 bg-black/30">
                            <FolderRoot className="h-4 w-4 text-sentinel-accent" />
                          </div>

                          <div className="min-w-0 flex-1">
                            <div className="flex flex-wrap items-center gap-2">
                              <span className="truncate text-sm font-semibold text-white">
                                {workspace.name}
                              </span>
                              {runningCount > 0 && (
                                <span className="workspace-running-badge inline-flex items-center gap-1 border border-emerald-300/20 bg-emerald-400/12 px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-[0.2em] text-emerald-100">
                                  <span className="workspace-running-dot h-1.5 w-1.5 bg-emerald-200" />
                                  running
                                </span>
                              )}
                              {unreadCount > 0 && (
                                <span className="inline-flex items-center gap-1 border border-amber-300/20 bg-amber-300/12 px-1.5 py-0.5 text-[9px] uppercase tracking-[0.18em] text-amber-100">
                                  {unreadCount} alert{unreadCount === 1 ? '' : 's'}
                                </span>
                              )}
                              <span className="border border-sky-300/20 bg-sky-300/10 px-1.5 py-0.5 text-[9px] uppercase tracking-[0.18em] text-sky-100">
                                {workspace.mode}
                              </span>
                              {active && (
                                <span className="border border-sentinel-accent/35 bg-black/25 px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-[0.22em] text-white">
                                  current
                                </span>
                              )}
                            </div>

                            <div className="mt-1 truncate text-[11px] text-sentinel-mist">
                              {workspace.project.path}
                            </div>

                            <div className="mt-2 flex flex-wrap items-center gap-2 text-[10px] uppercase tracking-[0.18em] text-sentinel-mist/85">
                              <span className="inline-flex items-center gap-1 border border-white/10 bg-black/25 px-1.5 py-0.5">
                                <TerminalSquare className="h-3 w-3" />
                                {workspaceSessionCount} sessions
                              </span>
                              <span className="inline-flex items-center gap-1 border border-white/10 bg-black/25 px-1.5 py-0.5">
                                <Layers3 className="h-3 w-3" />
                                {workspaceTabCount} tabs
                              </span>
                              {workspace.project.branch && (
                                <span className="inline-flex items-center gap-1 border border-sentinel-ice/25 bg-sentinel-ice/10 px-1.5 py-0.5 text-sentinel-ice">
                                  <GitBranch className="h-3 w-3" />
                                  {workspace.project.branch}
                                </span>
                              )}
                            </div>
                          </div>
                        </button>

                        <div className="relative flex shrink-0 items-center justify-center px-3">
                          <button
                            className={`inline-flex h-8 w-8 items-center justify-center rounded transition-colors ${
                              openMenuId === workspace.id ? 'bg-white/20 text-white' : 'text-sentinel-mist hover:bg-white/10 hover:text-white'
                            }`}
                            onClick={(e) => {
                              e.stopPropagation()
                              const rect = e.currentTarget.getBoundingClientRect()
                              setMenuPosition({ top: rect.bottom + 8, left: rect.right - 192 }) // 192px = w-48
                              setOpenMenuId(openMenuId === workspace.id ? null : workspace.id)
                            }}
                          >
                            <MoreHorizontal className="h-5 w-5" />
                          </button>

                          {openMenuId === workspace.id && createPortal(
                            <>
                              <div
                                className="fixed inset-0 z-[140]"
                                onClick={(e) => { e.stopPropagation(); setOpenMenuId(null) }}
                              />
                              <div 
                                className="fixed z-[150] w-48 rounded-md border border-white/10 bg-[#0b1219] p-1 shadow-[0_10px_40px_rgba(0,0,0,0.8)] backdrop-blur-3xl"
                                style={{
                                  top: menuPosition?.top,
                                  left: menuPosition?.left
                                }}
                              >
                                <button
                                  className="flex justify-between w-full items-center gap-2 rounded px-2 py-2 text-[11px] uppercase tracking-wide font-medium text-white transition hover:bg-white/10"
                                  onClick={(e) => { e.stopPropagation(); setOpenMenuId(null); setOpen(false); onWorkspaceAction(workspace.id, 'pause') }}
                                >
                                  Pause Workspace
                                  <Pause className="h-3.5 w-3.5 text-sentinel-mist" />
                                </button>
                                <button
                                  className="flex justify-between w-full items-center gap-2 rounded px-2 py-2 text-[11px] uppercase tracking-wide font-medium text-amber-300 transition hover:bg-amber-500/15"
                                  onClick={(e) => { e.stopPropagation(); setOpenMenuId(null); setOpen(false); onWorkspaceAction(workspace.id, 'stop') }}
                                >
                                  Stop Workspace
                                  <Square className="h-3.5 w-3.5 text-amber-300/80" />
                                </button>
                                <div className="my-1 h-px bg-white/10" />
                                <button
                                  className="flex justify-between w-full items-center gap-2 rounded px-2 py-2 text-[11px] uppercase tracking-wide font-bold text-rose-400 transition hover:bg-rose-500/20"
                                  onClick={(e) => { e.stopPropagation(); setOpenMenuId(null); setOpen(false); onWorkspaceAction(workspace.id, 'delete') }}
                                >
                                  Delete Workspace
                                  <Trash2 className="h-4 w-4" />
                                </button>
                              </div>
                            </>,
                            document.body
                          )}
                        </div>
                      </div>
                    )
                  })}
                </div>
              )}
            </div>

            <div className="border-t border-white/10 p-2">
              <button
                className="flex h-10 w-full items-center justify-center gap-2 border border-white/10 bg-white/[0.04] px-3 text-[11px] font-semibold uppercase tracking-[0.22em] text-white transition hover:border-white/20 hover:bg-white/[0.08]"
                onClick={() => {
                  setOpen(false)
                  onCreateWorkspace()
                }}
                type="button"
              >
                <Plus className="h-4 w-4" />
                Open Project as Workspace
              </button>
            </div>
          </div>
        </>,
        document.body
      )}
    </>
  )
}
