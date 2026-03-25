import { Fragment } from 'react'
import { GripHorizontal, GripVertical, LayoutGrid, Sidebar as SidebarIcon } from 'lucide-react'
import { Group, Panel, Separator } from 'react-resizable-panels'

import type { SessionCommandEntry, SessionSummary } from '@shared/types'

import { SessionTile } from './SessionTile'

interface AgentDashboardProps {
  sessions: SessionSummary[]
  histories: Record<string, SessionCommandEntry[]>
  sessionDiffs: Record<string, string[]>
  onClose: (sessionId: string) => Promise<void>
  onPause: (sessionId: string) => Promise<void>
  onResume: (sessionId: string) => Promise<void>
  onDelete: (sessionId: string) => Promise<void>
  onToggleMaximize: (sessionId: string) => void
  maximizedSessionId: string | null
  fitNonce: number
  windowsBuildNumber?: number
  layoutMode: 'grid' | 'master-stack'
  onSetLayoutMode: (mode: 'grid' | 'master-stack') => void
}

function getColumnCount(sessionCount: number): number {
  if (sessionCount <= 1) return 1
  if (sessionCount <= 4) return 2
  if (sessionCount <= 9) return 3
  return Math.ceil(Math.sqrt(sessionCount))
}

function buildRows(sessions: SessionSummary[]): SessionSummary[][] {
  const columnCount = getColumnCount(sessions.length)
  const rowCount = Math.ceil(sessions.length / columnCount)
  const rows: SessionSummary[][] = []
  let cursor = 0

  const baseSize = Math.floor(sessions.length / rowCount)
  const remainder = sessions.length % rowCount

  for (let rowIndex = 0; rowIndex < rowCount; rowIndex += 1) {
    const nextRowSize = baseSize + (rowIndex < remainder ? 1 : 0)
    rows.push(sessions.slice(cursor, cursor + nextRowSize))
    cursor += nextRowSize
  }

  return rows
}

function rowMinSize(rowCount: number): number {
  return rowCount <= 1 ? 100 : Math.max(16, Math.floor(100 / (rowCount + 2)))
}

function columnMinSize(columnCount: number): number {
  return columnCount <= 1 ? 100 : Math.max(14, Math.floor(100 / (columnCount + 2)))
}

function DashboardResizeHandle({ direction }: { direction: 'horizontal' | 'vertical' }): JSX.Element {
  const isHorizontal = direction === 'horizontal'
  return (
    <Separator
      className={
        isHorizontal
          ? 'dashboard-handle dashboard-handle-horizontal'
          : 'dashboard-handle dashboard-handle-vertical'
      }
    >
      <div className="dashboard-handle-bar z-50 relative pointer-events-none">
        {isHorizontal ? <GripHorizontal className="h-3.5 w-3.5" /> : <GripVertical className="h-3.5 w-3.5" />}
      </div>
    </Separator>
  )
}

export function AgentDashboard({
  sessions,
  histories,
  sessionDiffs,
  onClose,
  onPause,
  onResume,
  onDelete,
  onToggleMaximize,
  maximizedSessionId,
  fitNonce,
  windowsBuildNumber,
  layoutMode
}: AgentDashboardProps): JSX.Element {
  const visibleSessions = maximizedSessionId
    ? sessions.filter((session) => session.id === maximizedSessionId)
    : sessions

  if (visibleSessions.length === 0) {
    return <div className="h-full min-h-0 min-w-0 overflow-hidden border border-white/10 bg-black/10" />
  }

  if (visibleSessions.length === 1 && maximizedSessionId) {
    const session = visibleSessions[0]
    return (
      <div className="h-full min-h-0 min-w-0 overflow-hidden border border-white/10 bg-black/10 p-2">
        <div className="h-full min-h-0 min-w-0 p-1.5">
          <SessionTile
            fitNonce={fitNonce}
            historyEntries={histories[session.id] ?? []}
            modifiedPaths={sessionDiffs[session.id] || []}
            onDelete={onDelete}
            isMaximized
            onClose={onClose}
            onPause={onPause}
            onResume={onResume}
            onToggleMaximize={onToggleMaximize}
            session={session}
            applySession={() => window.sentinel.applySession(session.id)}
            commitSession={(msg) => window.sentinel.commitSession(session.id, msg)}
            discardSessionChanges={() => window.sentinel.discardSessionChanges(session.id)}
            windowsBuildNumber={windowsBuildNumber}
          />
        </div>
      </div>
    )
  }

  if (layoutMode === 'master-stack' && visibleSessions.length >= 3) {
    const masterSession = visibleSessions[0]
    const stackSessions = visibleSessions.slice(1)

    return (
      <div className="h-full min-h-0 min-w-0 overflow-hidden border border-white/10 bg-black/10 p-2">
        <Group className="h-full min-h-0" orientation="horizontal">
          <Panel className="min-h-0 min-w-0" defaultSize={65} minSize={30}>
            <div className="h-full min-h-0 min-w-0 p-1.5">
              <SessionTile
                fitNonce={fitNonce}
                historyEntries={histories[masterSession.id] ?? []}
                modifiedPaths={sessionDiffs[masterSession.id] || []}
                onDelete={onDelete}
                isMaximized={false}
                onClose={onClose}
                onPause={onPause}
                onResume={onResume}
                onToggleMaximize={onToggleMaximize}
                session={masterSession}
                applySession={() => window.sentinel.applySession(masterSession.id)}
                commitSession={(msg) => window.sentinel.commitSession(masterSession.id, msg)}
                discardSessionChanges={() => window.sentinel.discardSessionChanges(masterSession.id)}
                windowsBuildNumber={windowsBuildNumber}
              />
            </div>
          </Panel>
          <DashboardResizeHandle direction="vertical" />
          <Panel className="min-h-0 min-w-0" defaultSize={35} minSize={20}>
            <Group className="h-full min-h-0" orientation="vertical">
              {stackSessions.map((session, index) => (
                <Fragment key={session.id}>
                  {index > 0 && <DashboardResizeHandle direction="horizontal" />}
                  <Panel className="min-h-0 min-w-0" defaultSize={100 / stackSessions.length} minSize={15}>
                    <div className="h-full min-h-0 min-w-0 p-1.5">
                      <SessionTile
                        fitNonce={fitNonce}
                        historyEntries={histories[session.id] ?? []}
                        modifiedPaths={sessionDiffs[session.id] || []}
                        onDelete={onDelete}
                        isMaximized={false}
                        onClose={onClose}
                        onPause={onPause}
                        onResume={onResume}
                        onToggleMaximize={onToggleMaximize}
                        session={session}
                        applySession={() => window.sentinel.applySession(session.id)}
                        commitSession={(msg) => window.sentinel.commitSession(session.id, msg)}
                        discardSessionChanges={() => window.sentinel.discardSessionChanges(session.id)}
                        windowsBuildNumber={windowsBuildNumber}
                      />
                    </div>
                  </Panel>
                </Fragment>
              ))}
            </Group>
          </Panel>
        </Group>
      </div>
    )
  }

  const rows = buildRows(visibleSessions)

  return (
    <div className="h-full min-h-0 min-w-0 overflow-hidden border border-white/10 bg-black/10 p-2">
      <Group className="h-full min-h-0" orientation="vertical">
        {rows.map((row, rowIndex) => {
          const rowId = row.map((session) => session.id).join('-')
          return (
            <Fragment key={rowId}>
              {rowIndex > 0 && <DashboardResizeHandle direction="horizontal" />}
              <Panel
                className="min-h-0"
                defaultSize={100 / rows.length}
                minSize={rowMinSize(rows.length)}
              >
                <Group className="h-full min-h-0" orientation="horizontal">
                  {row.map((session, columnIndex) => (
                    <Fragment key={session.id}>
                      {columnIndex > 0 && <DashboardResizeHandle direction="vertical" />}
                      <Panel
                        className="min-h-0 min-w-0"
                        defaultSize={100 / row.length}
                        minSize={columnMinSize(row.length)}
                      >
                        <div className="h-full min-h-0 min-w-0 p-1.5">
                          <SessionTile
                            fitNonce={fitNonce}
                            historyEntries={histories[session.id] ?? []}
                            modifiedPaths={sessionDiffs[session.id] || []}
                            onDelete={onDelete}
                            isMaximized={false}
                            onClose={onClose}
                            onPause={onPause}
                            onResume={onResume}
                            onToggleMaximize={onToggleMaximize}
                            session={session}
                            applySession={() => window.sentinel.applySession(session.id)}
                            commitSession={(msg) => window.sentinel.commitSession(session.id, msg)}
                            discardSessionChanges={() => window.sentinel.discardSessionChanges(session.id)}
                            windowsBuildNumber={windowsBuildNumber}
                          />
                        </div>
                      </Panel>
                    </Fragment>
                  ))}
                </Group>
              </Panel>
            </Fragment>
          )
        })}
      </Group>
    </div>
  )
}
