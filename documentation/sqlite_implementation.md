# SQLite Implementation Plan

## Overview

This document describes the implementation plan for migrating Sentinel's state management from in-memory HashMaps to a persistent SQLite database. This migration is foundational for enabling advanced features like the Swarm Dashboard, workspace time-travel, agent knowledge bases, and comprehensive audit trails.

---

## Why SQLite?

### Benefits for Sentinel

| Benefit | Impact on Sentinel |
|---------|-------------------|
| **Persistence** | State survives app restarts, crashes, power loss |
| **Querying** | Complex queries (find sessions by file, search command history) |
| **Relationships** | Natural modeling of workspace to session to tab relationships |
| **Audit Trail** | Complete history of all actions with timestamps |
| **Performance** | Indexed queries scale better than linear HashMap scans |
| **ACID Guarantees** | Crash-safe transactions prevent data corruption |
| **Embedded** | No external database server required, single file |
| **Maturity** | Battle-tested, used in Chrome, Firefox, macOS, iOS |

### Problems with Current In-Memory Approach

- No persistence across app restarts
- No query capabilities (can't search history)
- No audit trail (can't trace what happened when)
- No time-travel (can't restore previous states)
- HashMap scans are O(n) for all lookups
- No relationships (workspace to sessions is manual)
- Activity log limited to runtime memory

---

## Target Architecture

### Current State (In-Memory)

- All state stored in Rust HashMaps
- State lost on app restart
- No historical data retention
- No querying capabilities

### Target State (SQLite-Powered)

- SQLite is the **source of truth** for all persistent data
- In-memory HashMaps become **caches** for active data only
- All historical data stored in SQLite
- Query capabilities for search, filter, aggregate operations
- Full audit trail of all user and agent actions

### Data Flow

```
Tauri Commands → SentinelManager → SQLite Database
     ↓                                    ↓
  User API                         Persistent Storage
     ↓                                    ↓
React UI ← Event Emissions ← State Changes
```

---

## Database Schema Design

### Core Tables

#### workspaces
Stores workspace metadata and configuration.

**Fields**: id, name, project path, project name, is git repo, git branch, default session strategy, created at, last active at, is active, sandbox state (serialized), metadata

**Indexes**: active status, project path

#### sessions
Stores all agent session records.

**Fields**: id, workspace id (foreign key), label, project root, current working directory, workspace path, workspace strategy, branch name, status, cleanup state, shell, process id, created at, startup command, exit code, error message, cpu percent, memory mb, thread count, handle count, process count, last metrics update

**Indexes**: workspace id, status, created at

#### tabs
Stores all standalone terminal tab records.

**Fields**: id, workspace id (foreign key), tab type, label, status, current working directory, shell, process id, created at, exit code, error message, cpu percent, memory mb, thread count, handle count, process count, last metrics update

**Indexes**: workspace id, status

#### ide_terminal
Stores IDE terminal state per workspace.

**Fields**: id, workspace id (unique foreign key), status, current working directory, workspace path, shell, process id, created at, exit code, error message, modified paths (json)

---

### History & Audit Tables

#### command_history
Stores every command executed in every session.

**Fields**: id, session id (foreign key), workspace id (foreign key), command text, timestamp, source (interactive or startup), exit code, duration milliseconds, current working directory

**Indexes**: session id, workspace id, timestamp

**Full-Text Search**: FTS5 virtual table for fast command search

#### file_changes
Tracks all file modifications by sessions.

**Fields**: id, session id (foreign key), workspace id (foreign key), file path, change type (created, modified, deleted), before hash, after hash, timestamp, file size

**Indexes**: session id, workspace id, file path, timestamp

#### activity_log
High-level activity tracking for workspace actions.

**Fields**: id, workspace id (foreign key), session id (foreign key), timestamp, scope (git, workspace, session), status (started, completed, failed), command, current working directory, detail

**Indexes**: workspace id, timestamp

#### audit_log
Comprehensive audit trail for compliance and debugging.

**Fields**: id, workspace id (foreign key), session id (foreign key), tab id (foreign key), timestamp, action type, resource type, resource id, details (json), user id (for future multi-user)

**Indexes**: workspace id, timestamp, action type

---

### Swarm Tables (Future Swarm Dashboard Feature)

#### agents
Stores agent definitions within swarms.

**Fields**: id, workspace id (foreign key), role (orchestrator, frontend, backend, database, documentation), label, model provider, model name, cli tool, status (idle, working, waiting, error, paused), session id (foreign key), created at, last active at, configuration (json)

**Indexes**: workspace id, role, status

#### agent_messages
Stores inter-agent communication.

**Fields**: id, workspace id (foreign key), from agent id (foreign key), to agent id (foreign key), message type (task assignment, status update, request help, response, broadcast), subject, content, payload (json), status (pending, delivered, read, acknowledged), created at, delivered at, read at

**Indexes**: workspace id, status, created at

#### tasks
Stores task assignments and tracking.

**Fields**: id, workspace id (foreign key), parent task id (self-referencing foreign key), assigned to agent id (foreign key), title, description, status (pending, in progress, blocked, completed, cancelled), priority, context (json), result (json), error message, created at, started at, completed at

**Indexes**: workspace id, status, assigned to agent id

---

### Configuration Tables

#### preferences
Stores user and workspace preferences.

**Fields**: id, workspace id (foreign key, nullable for global prefs), category (workspace, session, editor, terminal), key, value, is sensitive flag, updated at

**Constraints**: Unique constraint on (workspace id, category, key)

**Indexes**: workspace id, category

#### workspace_snapshots
Stores point-in-time workspace snapshots for time-travel feature.

**Fields**: id, workspace id (foreign key), name, description, created at, snapshot data (json), file count, session count

**Indexes**: workspace id, created at

---

## Implementation Phases

### Phase 1: Foundation Setup (Week 1-2)

**Objectives**:
- Add SQLite dependencies to project
- Create database module structure
- Write migration files for all tables
- Implement database initialization
- Set up connection pooling

**Deliverables**:
- Database connection pool configured
- Migration system in place (sqlx migrate)
- All tables created with proper indexes
- Database file location configured (app data directory)
- Basic backup mechanism implemented

**Testing**:
- Verify database file created on first launch
- Verify migrations run successfully
- Verify connection pool works under load

---

### Phase 2: Repository Layer (Week 2-3)

**Objectives**:
- Implement repository pattern for each table
- Create CRUD operations for all entities
- Add query methods for common operations
- Implement batch operations for performance

**Repositories to Implement**:
- WorkspaceRepository (create, read, update, delete, find all, find by id)
- SessionRepository (create, read, update, delete, find by workspace, find active)
- TabRepository (create, read, update, delete, find by workspace)
- CommandHistoryRepository (insert, find by session, find by workspace, search)
- FileChangeRepository (insert, find by session, find by workspace, find by file)
- ActivityLogRepository (insert, find by workspace, find by date range)
- AuditLogRepository (insert, find by workspace, export)

**Testing**:
- Unit tests for each repository method
- Integration tests with real database
- Performance tests for query latency

---

### Phase 3: Dual-Write Migration (Week 3-4)

**Objectives**:
- Keep existing HashMap-based code working
- Add SQLite writes alongside HashMap writes
- Verify data consistency between both stores
- No user-facing changes during this phase

**Implementation Pattern**:
- On create session: Write to HashMap AND SQLite
- On update session: Update HashMap AND SQLite
- On close session: Remove from HashMap AND SQLite
- Same pattern for tabs, workspaces, preferences

**Verification**:
- Compare HashMap state with SQLite state periodically
- Log any inconsistencies
- Automated tests verify both stores match

**Risk Mitigation**:
- Feature flag to disable SQLite writes if issues arise
- Rollback plan to HashMap-only if critical issues found

---

### Phase 4: SQLite as Source of Truth (Week 4-5)

**Objectives**:
- Switch read operations to SQLite
- Keep HashMap as cache for active data only
- Update bootstrap to load from SQLite
- Populate HashMap cache from SQLite on startup

**Bootstrap Changes**:
- Load all workspaces from SQLite
- Load active sessions from SQLite
- Load active tabs from SQLite
- Populate HashMap caches with loaded data
- Restore last active workspace

**Read Operation Changes**:
- Query SQLite first for all data
- HashMap is cache, not source of truth
- Cache invalidation on writes

**Testing**:
- Verify bootstrap returns identical data before/after
- Verify app works with SQLite-only data
- Performance tests ensure no regression

---

### Phase 5: HashMap Optimization (Week 5-6)

**Objectives**:
- Reduce HashMap to cache for active data only
- Historical data lives only in SQLite
- Update cleanup logic to archive before removing

**New State Structure**:
- Active workspace id only (not all workspaces)
- Session cache: active sessions only (not closed)
- Tab cache: active tabs only (not closed)
- Database connection as source of truth

**Benefits**:
- Reduced memory footprint
- Faster startup (only load active data)
- Historical queries use SQLite (indexed, fast)

**Testing**:
- Verify memory usage reduced
- Verify app works with minimal HashMap state
- Verify historical data accessible via SQLite

---

### Phase 6: Advanced Features (Week 6-8)

**Objectives**:
- Enable features only possible with SQLite
- Leverage full-text search, aggregations, time queries

**Features to Implement**:

**Command History Search**:
- Full-text search across all commands
- Search by keyword, session, workspace, date range
- Ranked results by relevance

**File Change Timeline**:
- Query file changes by path
- Show modification history for any file
- Compare file state across time points

**Workspace Analytics**:
- Aggregate queries for workspace metrics
- Session success rates
- Command frequency analysis
- Resource usage trends

**Audit Log Export**:
- Export audit log to JSON or CSV
- Filter by date range, action type, session
- Compliance-friendly reporting

**Workspace Snapshots**:
- Create point-in-time snapshots
- Restore workspace to previous state
- List all snapshots for workspace

**Testing**:
- Feature tests for each new capability
- Performance tests for query latency
- User acceptance testing

---

## Rust Module Structure

### Directory Layout

```
src-tauri/src/database/
├── mod.rs                  # Database initialization, connection pool
├── migrations/             # SQL migration files
│   ├── 001_initial_schema.sql
│   ├── 002_add_indexes.sql
│   ├── 003_add_fts.sql
│   └── (future migrations)
├── repositories/
│   ├── mod.rs              # Repository exports
│   ├── workspace.rs        # Workspace CRUD
│   ├── session.rs          # Session CRUD
│   ├── tab.rs              # Tab CRUD
│   ├── command.rs          # Command history
│   ├── file_change.rs      # File changes
│   ├── activity.rs         # Activity log
│   └── audit.rs            # Audit log
├── models.rs               # Database row structs
└── queries/                # Complex query builders
    ├── mod.rs
    ├── workspace_queries.rs
    └── analytics_queries.rs
```

---

## Frontend Changes Required

### Updated Type Definitions

All existing types need workspace id field added:
- SessionSummary
- TabSummary
- SessionMetricsUpdate
- TabMetricsUpdate
- TabStateUpdate
- SessionHistoryUpdate
- SessionDiffUpdate

New types to add:
- CommandHistoryEntry
- FileChangeEntry
- AuditLogEntry
- WorkspaceAnalytics
- SnapshotSummary

### New API Methods

Add to SentinelApi interface:
- searchCommandHistory(workspaceId, query)
- getFileChangeTimeline(workspaceId, filePath)
- getWorkspaceAnalytics(workspaceId)
- exportAuditLog(workspaceId, startDate, endDate)
- createWorkspaceSnapshot(workspaceId, name, description)
- restoreWorkspaceSnapshot(snapshotId)
- listWorkspaceSnapshots(workspaceId)

### Updated Event Listeners

All events now include workspace id:
- onSessionOutput (includes workspaceId)
- onSessionState (includes workspaceId)
- onSessionMetrics (includes workspaceId)
- onTabOutput (includes workspaceId)
- onTabState (includes workspaceId)
- onTabMetrics (includes workspaceId)

---

## Database Management

### Database File Location

Database file stored in application data directory:
- Windows: AppData/Roaming/sentinel/sentinel.db
- macOS: Library/Application Support/sentinel/sentinel.db
- Linux: .config/sentinel/sentinel.db

Directory created automatically on first launch.

### Backup Strategy

**Automatic Backups**:
- Daily backup at app startup
- Backup before schema migrations
- Backup before major operations

**Backup Retention**:
- Keep last 10 daily backups
- Keep last 4 weekly backups
- Automatic cleanup of old backups

**Backup Location**:
- Same directory as database
- Subdirectory: backups/
- Filename includes timestamp

### Maintenance Operations

**Vacuum**:
- Reclaim space from deleted rows
- Run weekly or when database exceeds threshold
- Can be run manually by user

**Analyze**:
- Update statistics for query optimizer
- Run after vacuum
- Improves query performance

**Integrity Check**:
- Verify database file integrity
- Run on startup (optional, configurable)
- Alert user if corruption detected

---

## Performance Optimization

### Connection Pooling

**Configuration**:
- Maximum connections: 10 (adjust based on workload)
- Minimum connections: 2 (keep warm)
- Acquire timeout: 30 seconds
- Idle timeout: 10 minutes

**Benefits**:
- Reuse connections instead of creating new
- Reduce connection overhead
- Handle concurrent queries efficiently

### Query Optimization

**Indexing Strategy**:
- All foreign keys indexed
- All frequently-queried fields indexed
- Composite indexes for common query patterns
- Full-text search indexes for text fields

**Query Patterns**:
- Use prepared statements (prevents SQL injection)
- Batch operations when possible
- Avoid N+1 query patterns
- Use JOINs instead of multiple queries

### Caching Strategy

**What to Cache**:
- Active sessions (frequently accessed)
- Active tabs (frequently accessed)
- Workspace analytics (expensive queries)
- Recently accessed file changes

**Cache Invalidation**:
- Invalidate on write operations
- Time-based expiration for analytics
- Manual invalidation API

**Cache Implementation**:
- In-memory HashMap for active data
- LRU cache for query results
- Configurable cache sizes

---

## Error Handling

### Error Categories

**Connection Errors**:
- Database file not found
- Database file locked
- Connection pool exhausted

**Query Errors**:
- Constraint violations
- Type mismatches
- Invalid SQL

**Migration Errors**:
- Migration already applied
- Migration script error
- Schema mismatch

### Retry Logic

**When to Retry**:
- Database locked (transient)
- Connection timeout (transient)
- Pool exhausted (transient)

**Retry Strategy**:
- Exponential backoff
- Maximum 3 retries
- Log all retry attempts

### User-Facing Errors

**Graceful Degradation**:
- App works in read-only mode if database unavailable
- Clear error messages to user
- Option to retry or continue without persistence

**Data Loss Prevention**:
- Warn before operations that might lose data
- Automatic backup before risky operations
- Recovery options after errors

---

## Testing Strategy

### Unit Tests

**Repository Tests**:
- Test each CRUD operation
- Test query methods
- Test edge cases (empty results, not found)
- Test constraint violations

**Mock Database**:
- Use in-memory SQLite for tests
- Fast test execution
- Isolated test state

### Integration Tests

**Full Flow Tests**:
- Create workspace, create session, send commands, close session
- Verify all data persisted correctly
- Verify relationships maintained
- Verify indexes work correctly

**Multi-User Simulation**:
- Concurrent operations
- Lock contention handling
- Transaction isolation

### Performance Tests

**Query Latency**:
- Measure query time for common operations
- Target: under 50ms for 95th percentile
- Load test with large datasets

**Throughput Tests**:
- Operations per second
- Concurrent connection handling
- Connection pool efficiency

### Migration Tests

**Schema Migration Tests**:
- Test migration from scratch
- Test migration with existing data
- Test rollback migrations
- Test data preservation during migration

---

## Monitoring & Observability

### Query Logging

**Development Mode**:
- Log all SQL statements
- Log query parameters
- Log execution time

**Production Mode**:
- Log slow queries only (threshold: 100ms)
- Log errors only
- Configurable log levels

### Metrics Collection

**Database Metrics**:
- Total queries executed
- Average query duration
- Connection pool utilization
- Database file size

**Application Metrics**:
- Bootstrap time
- Session creation time
- Command logging latency
- Query error rate

### Alerting

**Critical Alerts**:
- Database corruption detected
- Migration failure
- Data integrity violation

**Warning Alerts**:
- Slow query threshold exceeded
- Connection pool near capacity
- Database file size growing rapidly

---

## Rollback Plan

### Immediate Rollback (Development)

**Feature Flag**:
- Environment variable: USE_SQLITE
- Default: false (HashMap-only)
- Enable for testing

**Fallback Path**:
- Existing HashMap code remains
- Switch back by disabling feature flag
- No data migration needed

### Production Rollback

**Prerequisites**:
- Database backup available
- Rollback migrations prepared
- User communication plan ready

**Rollback Steps**:
1. Disable SQLite feature flag
2. Export critical data from SQLite
3. Repopulate HashMap from export
4. Notify users of rollback
5. Investigate and fix issues

**Data Recovery**:
- Export SQLite data to JSON
- Manual inspection if needed
- Re-import after fix

---

## Success Criteria

### Functional Requirements

- All existing features work identically after migration
- App restarts preserve all state (workspaces, sessions, history)
- Command history searchable with under 100ms latency
- File change timeline queryable by path and date range
- Audit log exportable to JSON or CSV
- Workspace snapshots can be created and restored

### Performance Requirements

- Bootstrap time under 2 seconds (with 100 workspaces, 500 sessions)
- Session creation under 500ms (including SQLite write)
- Command logging under 10ms (async, non-blocking)
- Query latency under 50ms for 95th percentile
- Database file size under 500MB after 1 year of typical use

### Reliability Requirements

- Zero data loss on app crash (verified with stress tests)
- Automatic backup created daily
- Recovery from corrupted database (graceful degradation)
- No memory leaks from connection pool
- Connection pool handles concurrent operations without deadlocks

### User Experience Requirements

- No noticeable changes to existing workflows
- New features (search, timeline, analytics) discoverable
- Clear error messages if database issues occur
- Performance feels identical or better than HashMap version

---

## Risks & Mitigations

### Risk: Data Loss During Migration

**Probability**: Low
**Impact**: High

**Mitigations**:
- Dual-write phase verifies consistency
- Automatic backups before migration
- Rollback plan tested and documented
- Gradual rollout to users

---

### Risk: Performance Regression

**Probability**: Medium
**Impact**: Medium

**Mitigations**:
- Performance tests before/after migration
- Connection pooling configured properly
- Indexes on all frequently-queried fields
- Caching for expensive queries

---

### Risk: Database Corruption

**Probability**: Low
**Impact**: High

**Mitigations**:
- ACID transactions prevent partial writes
- Regular integrity checks
- Automatic backups
- Graceful degradation to read-only mode

---

### Risk: Migration Complexity

**Probability**: Medium
**Impact**: Medium

**Mitigations**:
- Incremental migration (6 phases)
- Each phase tested independently
- Feature flags for gradual rollout
- Rollback plan for each phase

---

## Future Enhancements

### Phase 2+ Features (After Core Migration)

**Agent Knowledge Base**:
- Store agent decisions and outcomes
- Cross-session learning within workspace
- Similarity search for past solutions

**Workflow Templates**:
- Save and load workflow definitions
- Share templates between workspaces
- Template marketplace

**Advanced Analytics**:
- Agent productivity dashboards
- Command pattern recognition
- Predictive suggestions

**Team Collaboration**:
- Multi-user workspace support
- User attribution in audit log
- Permission management per user

**Cloud Sync**:
- Sync workspace state across devices
- Conflict resolution for concurrent edits
- End-to-end encryption

---

## Conclusion

Migrating to SQLite is a foundational investment that transforms Sentinel from an in-memory workspace into a persistent, queryable, audit-ready platform. The migration is designed to be incremental and reversible, minimizing risk while maximizing long-term benefits.

**Key Benefits**:
- Persistence across restarts
- Powerful query capabilities
- Complete audit trail
- Foundation for Swarm Dashboard
- Time-travel and snapshots
- Analytics and insights

**Timeline**: 6-8 weeks for core migration
**Risk Level**: Medium (mitigated by incremental approach)
**Impact**: High (enables entire class of new features)
