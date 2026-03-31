import { useState, useRef, useCallback } from 'react'
import { X, LayoutGrid, Terminal } from 'lucide-react'
import type { TabSummary } from '@shared/types'

interface WorkspaceTabsProps {
  tabs: TabSummary[]
  activeTabId: string
  onTabSelect: (tabId: string) => void
  onTabClose: (tabId: string) => void
}

export function WorkspaceTabs({
  tabs,
  activeTabId,
  onTabSelect,
  onTabClose
}: WorkspaceTabsProps): JSX.Element {
  const [hoveredTabId, setHoveredTabId] = useState<string | null>(null)
  const scrollContainerRef = useRef<HTMLDivElement>(null)

  const handleWheel = useCallback((event: React.WheelEvent) => {
    if (scrollContainerRef.current) {
      scrollContainerRef.current.scrollLeft += event.deltaY
    }
  }, [])

  const dashboardTab: TabSummary = {
    id: 'dashboard',
    workspaceId: 'dashboard',
    tabType: 'dashboard',
    label: 'Agents Dashboard',
    status: 'ready',
    cwd: '',
    shell: '',
    createdAt: 0,
    metrics: {
      cpuPercent: 0,
      memoryMb: 0,
      threadCount: 0,
      handleCount: 0,
      processCount: 0
    }
  }

  const allTabs = [dashboardTab, ...tabs]

  return (
    <div 
      className="flex items-center border-b border-white/10 bg-black/40"
      style={{ height: '24px' }}
    >
      {/* Tabs Container */}
      <div 
        ref={scrollContainerRef}
        className="flex-1 flex overflow-x-auto scrollbar-hide"
        onWheel={handleWheel}
        style={{ scrollbarWidth: 'none', msOverflowStyle: 'none' }}
      >
        {allTabs.map((tab) => {
          const isActive = tab.id === activeTabId
          const isHovered = tab.id === hoveredTabId
          const canClose = tab.tabType === 'terminal'

          return (
            <div
              key={tab.id}
              onClick={() => onTabSelect(tab.id)}
              onMouseEnter={() => setHoveredTabId(tab.id)}
              onMouseLeave={() => setHoveredTabId(null)}
              className={`
                group flex items-center gap-1.5 px-2 h-full border-r border-white/10 cursor-pointer
                transition-colors select-none min-w-[100px] max-w-[180px]
                ${isActive 
                  ? 'bg-white/[0.08] text-white border-b-0' 
                  : 'bg-transparent text-sentinel-mist hover:bg-white/[0.04] hover:text-white/80'
                }
              `}
            >
              {/* Tab Icon */}
              {tab.tabType === 'dashboard' ? (
                <LayoutGrid className="h-3 w-3 shrink-0 opacity-70" />
              ) : (
                <Terminal className="h-3 w-3 shrink-0 opacity-70" />
              )}

              {/* Tab Label */}
              <span className="flex-1 text-[10px] font-medium truncate">
                {tab.label}
              </span>

              {/* Status Indicator */}
              {tab.status === 'starting' && (
                <span className="h-1.5 w-1.5 rounded-full bg-sentinel-accent animate-pulse shrink-0" />
              )}

              {/* Close Button - shown on hover for closable tabs */}
              {canClose && (
                <button
                  onClick={(e) => {
                    e.stopPropagation()
                    onTabClose(tab.id)
                  }}
                  className={`
                    flex items-center justify-center h-3.5 w-3.5 rounded-none
                    transition-opacity duration-150
                    ${isHovered || isActive ? 'opacity-100' : 'opacity-0'}
                    hover:bg-white/10 text-sentinel-mist hover:text-white
                  `}
                  title="Close tab"
                >
                  <X className="h-2.5 w-2.5" />
                </button>
              )}
            </div>
          )
        })}
      </div>

      {/* Right spacer */}
      <div className="w-[140px] shrink-0" />
    </div>
  )
}
