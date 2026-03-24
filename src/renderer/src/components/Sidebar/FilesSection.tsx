import { useEffect, useState } from 'react'
import type { MouseEvent } from 'react'
import { ChevronDown, ChevronRight, Folder, FolderOpen, FolderRoot, Search, Sparkles } from 'lucide-react'

import type { ProjectState } from '@shared/types'
import {
  buildSidebarTree,
  collectAutoExpandedPaths,
  type SelectedFileEntry,
  type SidebarTreeNode,
  type WorkspaceOverlayFile
} from '../../workspace-overlay'
import { FileContextMenu } from './FileContextMenu'
import { collectDirectoryPaths, countFiles, filterSidebarTree } from './sidebar-utils'
import { TreeNode } from './TreeNode'

interface FileContextMenuState {
  node: SidebarTreeNode
  x: number
  y: number
}

interface FilesSectionProps {
  collapsed: boolean
  diffBadges: Record<string, string[]>
  overlayFiles: WorkspaceOverlayFile[]
  project: ProjectState
  selectedFileProjectPath?: string
  onFileSelect: (file: SelectedFileEntry) => void
}

export function FilesSection({
  collapsed,
  diffBadges,
  overlayFiles,
  project,
  selectedFileProjectPath,
  onFileSelect
}: FilesSectionProps): JSX.Element {
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set())
  const [contextMenu, setContextMenu] = useState<FileContextMenuState | null>(null)
  const [searchQuery, setSearchQuery] = useState('')
  const [showChangedOnly, setShowChangedOnly] = useState(false)
  const [expanded, setExpanded] = useState(true)

  const displayTree = buildSidebarTree(project.path, project.tree, overlayFiles)
  const autoExpandedPaths = collectAutoExpandedPaths(project.path, project.tree, overlayFiles)
  const overlaySignature = overlayFiles.map((file) => file.projectPath).sort().join('|')
  const filteredTree = filterSidebarTree(displayTree, searchQuery, showChangedOnly, diffBadges)
  const totalFileCount = countFiles(displayTree)
  const visibleFileCount = countFiles(filteredTree)
  const forceExpanded = searchQuery.trim().length > 0 || showChangedOnly
  const allDirectoryPaths = collectDirectoryPaths(displayTree)

  useEffect(() => {
    setExpandedPaths(autoExpandedPaths)
    setSearchQuery('')
    setShowChangedOnly(false)
    setExpanded(true)
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
    <section className={`border-b border-black/40 bg-black/10 ${expanded ? 'min-h-0 flex flex-1 flex-col' : 'shrink-0'}`}>
      <button
        className="flex w-full items-center justify-between gap-3 px-4 py-3 text-left text-[11px] font-bold uppercase tracking-[0.24em] text-sentinel-mist/70 transition hover:bg-white/[0.04] hover:text-white focus:outline-none focus:bg-white/[0.02]"
        onClick={() => setExpanded((current) => !current)}
        type="button"
      >
        <span>Files</span>
        <span className="flex items-center gap-2">
          <span className="text-[10px] tracking-[0.2em] text-white/40">{visibleFileCount}/{totalFileCount}</span>
          {expanded ? <ChevronDown className="h-3.5 w-3.5" /> : <ChevronRight className="h-3.5 w-3.5" />}
        </span>
      </button>

      <div className={`overflow-hidden transition-all duration-300 ease-out ${expanded ? 'min-h-0 flex-1 opacity-100' : 'max-h-0 opacity-0'}`}>
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

            <div className="mb-2 grid grid-cols-2 gap-2">
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

      {contextMenu && (
        <FileContextMenu
          filePath={contextMenu.node.targetPath}
          onOpenInSystemEditor={() => { void openInSystemEditor(contextMenu.node.targetPath) }}
          onRevealInExplorer={() => { void revealInExplorer(contextMenu.node.targetPath) }}
          x={contextMenu.x}
          y={contextMenu.y}
        />
      )}
    </section>
  )
}
