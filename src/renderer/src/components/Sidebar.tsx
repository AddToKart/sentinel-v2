import { useEffect, useState } from 'react'
import type { MouseEvent, ReactNode } from 'react'
import {
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  Copy,
  FileCode2,
  FileText,
  Folder,
  FolderOpen,
  FolderRoot,
  GitBranch,
  GitFork,
  RefreshCw,
  Search,
  Sparkles
} from 'lucide-react'

import type { ProjectState, SessionWorkspaceStrategy } from '@shared/types'
import {
  buildSidebarTree,
  collectAutoExpandedPaths,
  type SelectedFileEntry,
  type SidebarTreeNode,
  type WorkspaceOverlayFile
} from '../workspace-overlay'

interface SidebarProps {
  project: ProjectState
  refreshing: boolean
  collapsed: boolean
  diffBadges: Record<string, string[]>
  overlayFiles: WorkspaceOverlayFile[]
  defaultSessionStrategy: SessionWorkspaceStrategy
  selectedFileProjectPath?: string
  onOpenProject: () => void
  onRefreshProject: () => void
  onChangeDefaultSessionStrategy: (strategy: SessionWorkspaceStrategy) => void
  onToggleCollapse: () => void
  onFileSelect: (file: SelectedFileEntry) => void
  globalMode: 'multiplex' | 'ide'
  onToggleGlobalMode: (mode: 'multiplex' | 'ide') => void
}

interface FileContextMenuState {
  node: SidebarTreeNode
  x: number
  y: number
}

function fileIcon(name: string): JSX.Element {
  if (/\.(tsx?|jsx?|py|go|rs|json|ya?ml|css|md|toml|html)$/i.test(name)) {
    return <FileCode2 className="h-4 w-4 text-sentinel-ice" />
  }

  return <FileText className="h-4 w-4 text-sentinel-mist" />
}

function renderDiffBadges(badges: string[]): JSX.Element | null {
  if (badges.length === 0) {
    return null
  }

  return (
    <div className="ml-auto flex shrink-0 items-center gap-1">
      <span className="border border-sentinel-accent/35 bg-sentinel-accent/12 px-2 py-0.5 text-[9px] uppercase tracking-[0.22em] text-white">
        {badges[0]}
      </span>
      {badges.length > 1 && (
        <span className="border border-white/10 bg-white/[0.04] px-1.5 py-0.5 text-[9px] uppercase tracking-[0.22em] text-sentinel-mist">
          +{badges.length - 1}
        </span>
      )}
    </div>
  )
}

function countFiles(nodes: SidebarTreeNode[]): number {
  let total = 0

  for (const node of nodes) {
    if (node.kind === 'file') {
      total += 1
      continue
    }

    total += countFiles(node.children ?? [])
  }

  return total
}

function collectDirectoryPaths(nodes: SidebarTreeNode[]): string[] {
  const paths: string[] = []

  function visit(node: SidebarTreeNode): void {
    if (node.kind === 'directory') {
      paths.push(node.path)
      node.children?.forEach(visit)
    }
  }

  nodes.forEach(visit)
  return paths
}

function matchesTreeQuery(node: SidebarTreeNode, query: string): boolean {
  const normalizedName = node.name.toLowerCase()
  const normalizedPath = node.path.toLowerCase()

  return normalizedName.includes(query) || normalizedPath.includes(query)
}

function filterTreeNode(
  node: SidebarTreeNode,
  query: string,
  changedOnly: boolean,
  diffBadges: Record<string, string[]>
): SidebarTreeNode | null {
  const normalizedQuery = query.trim().toLowerCase()

  if (node.kind === 'file') {
    const isChanged = (diffBadges[node.path] ?? []).length > 0
    if (changedOnly && !isChanged) {
      return null
    }
    if (normalizedQuery && !matchesTreeQuery(node, normalizedQuery)) {
      return null
    }
    return node
  }

  const matchesSelf = normalizedQuery !== '' && matchesTreeQuery(node, normalizedQuery)
  const nextQuery = matchesSelf ? '' : normalizedQuery
  const nextChildren = (node.children ?? [])
    .map((child) => filterTreeNode(child, nextQuery, changedOnly, diffBadges))
    .filter((child): child is SidebarTreeNode => child !== null)

  if (nextChildren.length > 0) {
    return {
      ...node,
      children: nextChildren
    }
  }

  if (matchesSelf) {
    return {
      ...node,
      children: []
    }
  }

  return null
}

function filterSidebarTree(
  nodes: SidebarTreeNode[],
  query: string,
  changedOnly: boolean,
  diffBadges: Record<string, string[]>
): SidebarTreeNode[] {
  const normalizedQuery = query.trim().toLowerCase()
  if (!normalizedQuery && !changedOnly) {
    return nodes
  }

  return nodes
    .map((node) => filterTreeNode(node, normalizedQuery, changedOnly, diffBadges))
    .filter((node): node is SidebarTreeNode => node !== null)
}

function toggleButtonClasses(active: boolean): string {
  return active
    ? 'border-sentinel-accent/35 bg-sentinel-accent/12 text-white'
    : 'border-white/10 bg-white/[0.03] text-sentinel-mist hover:border-white/20 hover:bg-white/[0.05] hover:text-white'
}

function SidebarSection({
  title,
  meta,
  expanded,
  onToggle,
  children
}: {
  title: string
  meta?: string
  expanded: boolean
  onToggle: () => void
  children: ReactNode
}): JSX.Element {
  return (
    <section className="shrink-0 border-b border-white/10">
      <button
        className="flex w-full items-center justify-between gap-3 px-3 py-2.5 text-left text-[11px] font-semibold uppercase tracking-[0.24em] text-sentinel-mist transition hover:bg-white/[0.04] hover:text-white"
        onClick={onToggle}
        type="button"
      >
        <span>{title}</span>
        <span className="flex items-center gap-2">
          {meta && <span className="text-[10px] tracking-[0.2em] text-sentinel-mist/70">{meta}</span>}
          {expanded ? <ChevronDown className="h-3.5 w-3.5" /> : <ChevronRight className="h-3.5 w-3.5" />}
        </span>
      </button>

      <div className={`overflow-hidden transition-all duration-300 ease-out ${expanded ? 'max-h-[28rem] opacity-100' : 'max-h-0 opacity-0'}`}>
        <div className="px-3 pb-3">{children}</div>
      </div>
    </section>
  )
}

function TreeNode({
  depth,
  expandedPaths,
  diffBadges,
  forceExpanded,
  node,
  onFileContextMenu,
  onFileSelect,
  selectedPath,
  toggle
}: {
  node: SidebarTreeNode
  depth: number
  expandedPaths: Set<string>
  diffBadges: Record<string, string[]>
  forceExpanded: boolean
  toggle: (path: string) => void
  onFileSelect: (file: SelectedFileEntry) => void
  onFileContextMenu: (event: MouseEvent<HTMLButtonElement>, node: SidebarTreeNode) => void
  selectedPath?: string
}): JSX.Element {
  const isDirectory = node.kind === 'directory'
  const expanded = forceExpanded || expandedPaths.has(node.path)
  const hasChildren = Boolean(node.children && node.children.length > 0)
  const badges = node.kind === 'file' ? diffBadges[node.path] ?? [] : []
  const isModified = badges.length > 0
  const isSelected = node.kind === 'file' && selectedPath === node.path

  return (
    <div className="space-y-1">
      <button
        className={`group flex w-full items-center gap-2 border-l-2 px-2 py-1.5 text-left text-[13px] transition-all duration-150 ${
          isSelected
            ? 'border-sentinel-accent bg-white/[0.08] text-white'
            : isModified
              ? 'border-sentinel-ice/70 bg-white/[0.04] text-white'
              : 'border-transparent text-sentinel-mist hover:border-white/10 hover:bg-white/[0.04] hover:text-white'
        }`}
        onClick={() => {
          if (isDirectory) {
            toggle(node.path)
          } else {
            onFileSelect({
              projectPath: node.path,
              workspacePath: node.targetPath !== node.path ? node.targetPath : undefined
            })
          }
        }}
        onContextMenu={(event) => {
          if (!isDirectory) {
            onFileContextMenu(event, node)
          }
        }}
        style={{ paddingLeft: 8 + depth * 14 }}
        title={node.path}
        type="button"
      >
        {isDirectory ? (
          <>
            {hasChildren ? (
              <ChevronRight className={`h-4 w-4 shrink-0 transition-transform duration-200 ${expanded ? 'rotate-90 text-white' : ''}`} />
            ) : (
              <span className="inline-block h-4 w-4 shrink-0" />
            )}
            {expanded ? (
              <FolderOpen className="h-4 w-4 shrink-0 text-sentinel-accent" />
            ) : (
              <Folder className="h-4 w-4 shrink-0 text-sentinel-accent" />
            )}
          </>
        ) : (
          <>
            <span className="inline-block h-4 w-4 shrink-0" />
            {fileIcon(node.name)}
          </>
        )}

        <span className="min-w-0 flex-1 truncate">{node.name}</span>
        {renderDiffBadges(badges)}
      </button>

      {isDirectory && (
        <div className={`ml-2 overflow-hidden border-l border-white/[0.05] pl-1 transition-all duration-300 ease-in-out ${expanded && hasChildren ? 'max-h-[2200px] opacity-100' : 'max-h-0 opacity-0'}`}>
          <div className="space-y-1 py-0.5">
            {node.children?.map((child) => (
              <TreeNode
                key={child.path}
                depth={depth + 1}
                diffBadges={diffBadges}
                expandedPaths={expandedPaths}
                forceExpanded={forceExpanded}
                node={child}
                onFileContextMenu={onFileContextMenu}
                onFileSelect={onFileSelect}
                selectedPath={selectedPath}
                toggle={toggle}
              />
            ))}
          </div>
        </div>
      )}
    </div>
  )
}

export function Sidebar({
  project,
  refreshing,
  collapsed,
  diffBadges,
  overlayFiles,
  defaultSessionStrategy,
  selectedFileProjectPath,
  onOpenProject,
  onRefreshProject,
  onChangeDefaultSessionStrategy,
  onToggleCollapse,
  onFileSelect,
  globalMode,
  onToggleGlobalMode
}: SidebarProps): JSX.Element {
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set())
  const [contextMenu, setContextMenu] = useState<FileContextMenuState | null>(null)
  const [searchQuery, setSearchQuery] = useState('')
  const [showChangedOnly, setShowChangedOnly] = useState(false)
  const [workspaceSectionOpen, setWorkspaceSectionOpen] = useState(true)
  const [projectSectionOpen, setProjectSectionOpen] = useState(true)
  const [modesSectionOpen, setModesSectionOpen] = useState(false)
  const [filesSectionOpen, setFilesSectionOpen] = useState(true)

  const displayTree = buildSidebarTree(project.path, project.tree, overlayFiles)
  const autoExpandedPaths = collectAutoExpandedPaths(project.path, project.tree, overlayFiles)
  const overlaySignature = overlayFiles.map((file) => file.projectPath).sort().join('|')
  const filteredTree = filterSidebarTree(displayTree, searchQuery, showChangedOnly, diffBadges)
  const totalFileCount = countFiles(displayTree)
  const visibleFileCount = countFiles(filteredTree)
  const modifiedFileCount = Object.values(diffBadges).filter((badges) => badges.length > 0).length
  const forceExpanded = searchQuery.trim().length > 0 || showChangedOnly
  const allDirectoryPaths = collectDirectoryPaths(displayTree)

  useEffect(() => {
    setExpandedPaths(autoExpandedPaths)
    setSearchQuery('')
    setShowChangedOnly(false)
    setWorkspaceSectionOpen(true)
    setProjectSectionOpen(true)
    setModesSectionOpen(false)
    setFilesSectionOpen(true)
  }, [project.path])

  useEffect(() => {
    if (autoExpandedPaths.size === 0) {
      return
    }

    setExpandedPaths((current) => {
      const next = new Set(current)
      autoExpandedPaths.forEach((pathValue) => next.add(pathValue))
      return next
    })
  }, [project.path, overlaySignature])

  useEffect(() => {
    function closeContextMenu(): void {
      setContextMenu(null)
    }

    function handleEscape(event: KeyboardEvent): void {
      if (event.key === 'Escape') {
        closeContextMenu()
      }
    }

    window.addEventListener('pointerdown', closeContextMenu)
    window.addEventListener('keydown', handleEscape)

    return () => {
      window.removeEventListener('pointerdown', closeContextMenu)
      window.removeEventListener('keydown', handleEscape)
    }
  }, [])

  useEffect(() => {
    if (collapsed) {
      setContextMenu(null)
    }
  }, [collapsed])

  function toggle(pathValue: string): void {
    if (forceExpanded) {
      return
    }

    setExpandedPaths((current) => {
      const next = new Set(current)

      if (next.has(pathValue)) {
        next.delete(pathValue)
      } else {
        next.add(pathValue)
      }

      return next
    })
  }

  function handleFileContextMenu(event: MouseEvent<HTMLButtonElement>, node: SidebarTreeNode): void {
    event.preventDefault()
    event.stopPropagation()

    setContextMenu({
      node,
      x: event.clientX,
      y: event.clientY
    })
  }

  async function revealInExplorer(filePath: string): Promise<void> {
    setContextMenu(null)
    await window.sentinel.revealInFileExplorer(filePath)
  }

  async function openInSystemEditor(filePath: string): Promise<void> {
    setContextMenu(null)
    await window.sentinel.openInSystemEditor(filePath)
  }

  function expandAllDirectories(): void {
    setExpandedPaths(new Set(allDirectoryPaths))
  }

  function collapseDirectories(): void {
    setExpandedPaths(new Set(autoExpandedPaths))
  }

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

      <div className="flex shrink-0 items-center justify-between border-b border-white/10 px-4 py-3">
        <div className="text-[10px] font-semibold uppercase tracking-[0.28em] text-sentinel-mist">Explorer</div>
        <div className="flex shrink-0 items-center gap-1">
          <button
            className="inline-flex h-7 w-7 items-center justify-center border border-white/10 bg-white/[0.04] text-white transition hover:border-white/20 hover:bg-white/[0.08]"
            onClick={onRefreshProject}
            title="Refresh tree"
            type="button"
          >
            <RefreshCw className={`h-3 w-3 ${refreshing ? 'animate-spin' : ''}`} />
          </button>
          <button
            className="inline-flex h-7 w-7 items-center justify-center border border-white/10 bg-white/[0.04] text-white transition hover:border-white/20 hover:bg-white/[0.08]"
            onClick={onToggleCollapse}
            title="Collapse sidebar"
            type="button"
          >
            <ChevronLeft className="h-3 w-3" />
          </button>
        </div>
      </div>

      <SidebarSection
        expanded={workspaceSectionOpen}
        meta={project.path ? 'active' : 'idle'}
        onToggle={() => setWorkspaceSectionOpen((current) => !current)}
        title="Workspace"
      >
        <div className="space-y-4 pt-1">
          <div className="min-w-0">
            <div className="truncate text-sm font-semibold text-white">
              {project.name || 'No project selected'}
            </div>
            <div className="mt-2 flex flex-wrap items-center gap-2 text-[10px] uppercase tracking-[0.22em] text-sentinel-mist">
              <span className="border border-white/10 bg-white/[0.04] px-2 py-1">
                {project.path ? 'workspace' : 'idle'}
              </span>
              <span className="border border-white/10 bg-white/[0.04] px-2 py-1">
                {modifiedFileCount} changed
              </span>
              {project.branch && (
                <span className="inline-flex items-center gap-1 border border-sentinel-ice/25 bg-sentinel-ice/10 px-2 py-1 text-sentinel-ice">
                  <GitBranch className="h-3 w-3" />
                  {project.branch}
                </span>
              )}
            </div>
          </div>

          <div className="grid grid-cols-2 gap-2">
            <button
              className="inline-flex items-center justify-center gap-2 border border-white/10 bg-white/[0.04] px-3 py-2 text-[11px] font-semibold uppercase tracking-[0.22em] text-white transition hover:border-white/20 hover:bg-white/[0.08]"
              onClick={onOpenProject}
              type="button"
            >
              <FolderRoot className="h-3.5 w-3.5" />
              Open
            </button>

            <button
              className={`inline-flex items-center justify-center gap-2 border px-3 py-2 text-[11px] font-semibold uppercase tracking-[0.22em] transition ${
                globalMode === 'ide'
                  ? 'border-emerald-500/30 bg-emerald-500/12 text-white'
                  : 'border-white/10 bg-white/[0.04] text-sentinel-mist hover:border-white/20 hover:bg-white/[0.08] hover:text-white'
              }`}
              onClick={() => onToggleGlobalMode(globalMode === 'multiplex' ? 'ide' : 'multiplex')}
              type="button"
            >
              <Sparkles className="h-3.5 w-3.5" />
              {globalMode === 'ide' ? 'Agents' : 'IDE'}
            </button>
          </div>
        </div>
      </SidebarSection>

      <SidebarSection
        expanded={projectSectionOpen}
        meta={project.path ? 'project' : 'idle'}
        onToggle={() => setProjectSectionOpen((current) => !current)}
        title="Project"
      >
        <div className="space-y-3 border-l border-white/10 pl-3">
          <div className="text-sm font-medium text-white">{project.name || 'No project selected'}</div>
          {project.path ? (
            <div className="font-mono text-[11px] leading-5 text-sentinel-mist break-all">
              {project.path}
            </div>
          ) : (
            <div className="text-xs leading-6 text-sentinel-mist">
              Open a folder to browse files and run agent sessions.
            </div>
          )}
        </div>
      </SidebarSection>

      <SidebarSection
        expanded={modesSectionOpen}
        meta={globalMode === 'ide' ? 'ide' : 'agents'}
        onToggle={() => setModesSectionOpen((current) => !current)}
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
                className={`border px-3 py-2 text-[11px] font-semibold uppercase tracking-[0.2em] transition ${
                  globalMode === 'ide'
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

      <section className={`border-b border-white/10 ${filesSectionOpen ? 'min-h-0 flex flex-1 flex-col' : 'shrink-0'}`}>
        <button
          className="flex w-full items-center justify-between gap-3 px-3 py-2.5 text-left text-[11px] font-semibold uppercase tracking-[0.24em] text-sentinel-mist transition hover:bg-white/[0.04] hover:text-white"
          onClick={() => setFilesSectionOpen((current) => !current)}
          type="button"
        >
          <span>Files</span>
          <span className="flex items-center gap-2">
            <span className="text-[10px] tracking-[0.2em] text-sentinel-mist/70">{visibleFileCount}/{totalFileCount}</span>
            {filesSectionOpen ? <ChevronDown className="h-3.5 w-3.5" /> : <ChevronRight className="h-3.5 w-3.5" />}
          </span>
        </button>

        <div className={`overflow-hidden transition-all duration-300 ease-out ${filesSectionOpen ? 'min-h-0 flex-1 opacity-100' : 'max-h-0 opacity-0'}`}>
          <div className="flex h-full min-h-0 flex-col px-3 pb-3">
            <div className="space-y-2 border-b border-white/10 pb-3">
              <label className="flex min-w-0 items-center gap-2 border border-white/10 bg-black/20 px-3 py-2 text-sentinel-mist transition focus-within:border-sentinel-accent/35 focus-within:text-white">
                <Search className="h-4 w-4 shrink-0" />
                <input
                  className="min-w-0 flex-1 bg-transparent text-sm text-white outline-none placeholder:text-sentinel-mist/60"
                  onChange={(event) => setSearchQuery(event.target.value)}
                  placeholder="Search files"
                  type="search"
                  value={searchQuery}
                />
              </label>

              <div className="grid grid-cols-2 gap-2 mb-2">
                <button
                  className={`inline-flex h-8 items-center justify-center gap-2 border px-3 text-[10px] font-semibold uppercase tracking-[0.22em] transition ${!showChangedOnly ? 'border-sentinel-accent/35 bg-sentinel-accent/12 text-white' : 'border-white/10 bg-white/[0.04] text-sentinel-mist hover:border-white/20 hover:bg-white/[0.08] hover:text-white'}`}
                  onClick={() => setShowChangedOnly(false)}
                  type="button"
                >
                  <FolderRoot className="h-3 w-3" />
                  Files
                </button>

                <button
                  className={`inline-flex h-8 items-center justify-center gap-2 border px-3 text-[10px] font-semibold uppercase tracking-[0.22em] transition ${showChangedOnly ? 'border-sentinel-accent/35 bg-sentinel-accent/12 text-white' : 'border-white/10 bg-white/[0.04] text-sentinel-mist hover:border-white/20 hover:bg-white/[0.08] hover:text-white'}`}
                  onClick={() => setShowChangedOnly(true)}
                  type="button"
                >
                  <Sparkles className="h-3 w-3" />
                  Changed
                </button>
              </div>

              <div className="grid grid-cols-2 gap-2">
                <button
                  className="inline-flex h-7 items-center justify-center gap-2 border border-white/10 bg-white/[0.04] px-3 text-[10px] font-semibold uppercase tracking-[0.22em] text-sentinel-mist transition hover:border-white/20 hover:bg-white/[0.08] hover:text-white"
                  onClick={expandAllDirectories}
                  title="Expand all folders"
                  type="button"
                >
                  <FolderOpen className="h-3 w-3" />
                  Expand
                </button>

                <button
                  className="inline-flex h-7 items-center justify-center gap-2 border border-white/10 bg-white/[0.04] px-3 text-[10px] font-semibold uppercase tracking-[0.22em] text-sentinel-mist transition hover:border-white/20 hover:bg-white/[0.08] hover:text-white"
                  onClick={collapseDirectories}
                  title="Collapse folders"
                  type="button"
                >
                  <Folder className="h-3 w-3" />
                  Reset
                </button>
              </div>
            </div>

            <div className="mt-3 min-h-0 flex-1 overflow-auto pr-1">
              {filteredTree.length === 0 ? (
                <div className="border border-dashed border-white/10 bg-white/[0.02] p-4 text-sm leading-6 text-sentinel-mist">
                  {project.path
                    ? 'No files match the current search or filter.'
                    : 'Open a project to browse files here.'}
                </div>
              ) : (
                <div className="space-y-1">
                  {filteredTree.map((node) => (
                    <TreeNode
                      key={node.path}
                      depth={0}
                      diffBadges={diffBadges}
                      expandedPaths={expandedPaths}
                      forceExpanded={forceExpanded}
                      node={node}
                      onFileContextMenu={handleFileContextMenu}
                      onFileSelect={onFileSelect}
                      selectedPath={selectedFileProjectPath}
                      toggle={toggle}
                    />
                  ))}
                </div>
              )}
            </div>
          </div>
        </div>
      </section>

      {contextMenu && (
        <div
          className="fixed z-50 min-w-[220px] border border-white/10 bg-[#0b1219] p-1.5 shadow-terminal backdrop-blur-2xl"
          style={{ left: contextMenu.x, top: contextMenu.y }}
        >
          <button
            className="flex w-full items-center justify-between gap-3 px-3 py-2 text-left text-sm text-white transition hover:bg-white/[0.06]"
            onClick={() => {
              void revealInExplorer(contextMenu.node.targetPath)
            }}
            type="button"
          >
            <span>Reveal in File Explorer</span>
            <span className="font-mono text-[11px] text-sentinel-mist">explorer</span>
          </button>
          <button
            className="flex w-full items-center justify-between gap-3 px-3 py-2 text-left text-sm text-white transition hover:bg-white/[0.06]"
            onClick={() => {
              void openInSystemEditor(contextMenu.node.targetPath)
            }}
            type="button"
          >
            <span>Open in System Editor</span>
            <span className="font-mono text-[11px] text-sentinel-mist">system</span>
          </button>
        </div>
      )}
    </aside>
  )
}
