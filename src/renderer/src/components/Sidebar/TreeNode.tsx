import type { MouseEvent } from 'react'
import { ChevronRight, Folder, FolderOpen } from 'lucide-react'

import type { SelectedFileEntry, SidebarTreeNode } from '../../workspace-overlay'
import { fileIcon, renderDiffBadges } from './sidebar-utils'

interface TreeNodeProps {
  node: SidebarTreeNode
  depth: number
  expandedPaths: Set<string>
  diffBadges: Record<string, string[]>
  forceExpanded: boolean
  toggle: (path: string) => void
  onFileSelect: (file: SelectedFileEntry) => void
  onFileContextMenu: (event: MouseEvent<HTMLButtonElement>, node: SidebarTreeNode) => void
  selectedPath?: string
}

export function TreeNode({
  node,
  depth,
  expandedPaths,
  diffBadges,
  forceExpanded,
  toggle,
  onFileSelect,
  onFileContextMenu,
  selectedPath
}: TreeNodeProps): JSX.Element {
  const isDirectory = node.kind === 'directory'
  const expanded = forceExpanded || expandedPaths.has(node.path)
  const hasChildren = Boolean(node.children && node.children.length > 0)
  const badges = node.kind === 'file' ? diffBadges[node.path] ?? [] : []
  const isModified = badges.length > 0
  const isSelected = node.kind === 'file' && selectedPath === node.path

  return (
    <div className="space-y-1">
      <button
        className={`group flex w-full items-center gap-2 border-l-2 px-2 py-1.5 text-left text-[13px] transition-all duration-150 ${isSelected
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
