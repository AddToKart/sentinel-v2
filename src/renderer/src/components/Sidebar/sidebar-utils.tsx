import { FileCode2, FileText } from 'lucide-react'

import type { SidebarTreeNode } from '../../workspace-overlay'

export function fileIcon(name: string): JSX.Element {
  if (/\.(tsx?|jsx?|py|go|rs|json|ya?ml|css|md|toml|html)$/i.test(name)) {
    return <FileCode2 className="h-4 w-4 text-sentinel-ice" />
  }

  return <FileText className="h-4 w-4 text-sentinel-mist" />
}

export function renderDiffBadges(badges: string[]): JSX.Element | null {
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

export function countFiles(nodes: SidebarTreeNode[]): number {
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

export function collectDirectoryPaths(nodes: SidebarTreeNode[]): string[] {
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

export function filterSidebarTree(
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

export function toggleButtonClasses(active: boolean): string {
  return active
    ? 'border-sentinel-accent/35 bg-sentinel-accent/12 text-white'
    : 'border-white/10 bg-white/[0.03] text-sentinel-mist hover:border-white/20 hover:bg-white/[0.05] hover:text-white'
}
