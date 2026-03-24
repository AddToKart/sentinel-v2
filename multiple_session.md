# Multi-Workspace Support with tmux-like Detach/Attach

## Overview

This document describes the implementation plan for adding multi-workspace support to Sentinel, allowing users to:
- Work with multiple projects simultaneously
- Switch between workspaces (detach/attach like tmux)
- Maintain running sessions across workspace switches
- View and manage all workspaces from a unified interface

---

## Current Architecture Limitations

### Single Project Model
- State management assumes one global project at a time
- No concept of workspace context or workspace switching
- Sessions are tied to the singular project state

### Session-Project Coupling
- Sessions are created within the context of one global project
- No workspace identifier on sessions
- Events don't include workspace context
- Cannot view or manage multiple projects simultaneously

---

## Proposed Architecture

### Core Concepts

**Workspace Context**
A workspace encapsulates:
- Unique identifier and display name
- Project state (path, git info, file tree)
- List of associated session IDs
- List of associated tab IDs
- Creation and last-active timestamps
- Default session strategy preference

**Global State Changes**
- Replace singular `project` with a collection of workspaces
- Add `active_workspace_id` to track current focus
- Sessions and tabs remain in their own maps but are now associated with a workspace ID
- Global preferences store default strategy and last-used workspace

---

## Backend Implementation Plan

### New Data Structures

**Workspace Context Model**
- Add to models.rs
- Fields: id, name, project state, session IDs list, tab IDs list, timestamps, default strategy

**Updated Sentinel State**
- Replace `project: ProjectState` with `workspaces: HashMap<WorkspaceContext>`
- Add `active_workspace_id: Option<String>`
- Move `default_session_strategy` from WorkspaceSummary to GlobalPreferences

**Global Preferences**
- Separate from per-workspace preferences
- Stores default session strategy
- Stores last workspace ID for restoration

### New Tauri Commands

**create_workspace**
- Parameters: name, project path
- Validates path exists and is accessible
- Scans project tree (reuses existing project loading logic)
- Generates unique workspace ID
- Returns created workspace context

**switch_workspace**
- Parameters: workspace ID
- Validates workspace exists
- Updates active_workspace_id
- Emits workspace-switched event
- Returns new active workspace context

**list_workspaces**
- No parameters
- Returns array of all workspace contexts
- Used for populating workspace switcher UI

**close_workspace**
- Parameters: workspace ID, close_sessions flag
- If close_sessions is true, terminates all sessions in workspace
- Removes workspace from state
- Emits workspace-removed event
- Returns success or error

**get_active_workspace**
- No parameters
- Returns current active workspace context
- Used for initial UI state

### Updated Existing Commands

**bootstrap**
- Extended return payload includes:
  - All workspaces array
  - Active workspace ID
  - Existing sessions, tabs, metrics, etc.

**create_session**
- Automatically associates new session with active_workspace_id
- Adds session ID to workspace's session_ids list
- Updates workspace last_active timestamp

**create_standalone_terminal**
- Associates new tab with active_workspace_id
- Adds tab ID to workspace's tab_ids list

### Event System Updates

**New Events**
- `workspace-switched` - Emitted when switching workspaces
- `workspace-created` - Emitted when creating workspace
- `workspace-removed` - Emitted when closing workspace

**Updated Events**
- Session state events include workspace_id
- Tab state events include workspace_id
- Workspace state events scoped to active workspace

---

## Frontend Implementation Plan

### Updated Type Definitions

**WorkspaceContext Type**
- Mirror Rust model structure
- Include all workspace metadata fields

**Extended Bootstrap Payload**
- Add workspaces array
- Add active_workspace_id field

**Session Summary Updates**
- Add workspace_id field to each session

**Tab Summary Updates**
- Add workspace_id field to each tab

**Global Preferences Type**
- Separate from workspace preferences
- Include default strategy and last workspace ID

### New SentinelApi Methods

**Workspace Management**
- createWorkspace(name, path)
- switchWorkspace(workspaceId)
- listWorkspaces()
- closeWorkspace(workspaceId, closeSessions)
- getActiveWorkspace()

**Event Listeners**
- onWorkspaceSwitched(listener)
- onWorkspaceCreated(listener)
- onWorkspaceRemoved(listener)

### New UI Components

**WorkspaceSwitcher Component**
- Dropdown or popover interface
- Displays list of all workspaces
- Shows for each workspace:
  - Name and project path
  - Active session count
  - Visual indicator for active workspace
- Click to switch workspace
- Create new workspace option
- Close workspace option (with confirmation)

**WorkspaceHeader Component**
- Displays current workspace info in header bar
- Shows workspace name and project name
- Click to open workspace switcher
- Session count badge

**Sidebar Enhancement (Alternative)**
- Workspace list in sidebar
- Expandable workspace items showing sessions
- Quick switch between workspaces
- Create/close workspace actions

### App.tsx State Updates

**New State Variables**
- workspaces: array of WorkspaceContext
- activeWorkspaceId: string or null

**Session/Tab Filtering**
- Filter sessions by activeWorkspaceId for display
- Filter tabs by activeWorkspaceId for display
- All sessions/tabs remain in state (not removed on switch)

**Event Listener Updates**
- Listen for workspace-switched events
- Update activeWorkspaceId on switch
- Refresh workspace list on create/remove

**Detach/Attach Behavior**
- Detach: Switch workspace, sessions remain in state and continue running
- Attach: Switch back, sessions re-render with preserved terminal buffers

---

## Detach/Attach Behavior Specification

### Detach (Switch Away from Workspace)

1. User clicks different workspace in switcher
2. Frontend calls switchWorkspace API
3. Backend updates active_workspace_id
4. Backend emits workspace-switched event
5. Sessions in previous workspace continue running unchanged
6. Frontend filters display to show only new workspace's sessions
7. Metrics and events still collected for all sessions
8. Terminal output buffers preserved in memory

### Attach (Switch Back to Workspace)

1. User clicks previous workspace in switcher
2. Frontend calls switchWorkspace API
3. Backend updates active_workspace_id
4. Backend emits workspace-switched event
5. Frontend re-renders with previous workspace's sessions
6. Terminal buffers fully preserved (full history visible)
7. Metrics display resumes immediately
8. Session interactivity restored

---

## UI/UX Design Specifications

### Workspace Switcher (Dropdown Style)

**Trigger**
- Located in header bar
- Shows current workspace name with dropdown indicator
- Click opens popover

**Popover Content**
- List of workspaces with:
  - Active indicator (dot or highlight)
  - Workspace name
  - Project path (truncated if long)
  - Session count badge
  - Tab count badge
- Divider
- "Create Workspace" action item
- Each workspace row has close button (with confirmation)

**Visual States**
- Active workspace highlighted
- Hover state on rows
- Disabled state if switching in progress

### Workspace Header Display

**Content**
- Workspace name or project name
- Project path (optional, truncated)
- Session count
- Quick actions (refresh, settings)

**Placement**
- Top header bar, adjacent to existing controls
- Or integrated into existing project info display

### Status Bar Enhancement

**Additional Info**
- Active workspace name
- Total sessions across all workspaces
- Total resource usage (CPU, memory) across all workspaces

---

## File Structure Changes

### New Backend Files

**src-tauri/src/sentinelApi/workspaces.rs**
- Workspace CRUD operations
- create_workspace function
- switch_workspace function
- list_workspaces function
- close_workspace function
- Helper functions for workspace management

### New Frontend Files

**src/renderer/src/components/WorkspaceSwitcher.tsx**
- Dropdown/popover component
- Workspace list rendering
- Create workspace action
- Close workspace action
- Active workspace indicator

**src/renderer/src/components/WorkspaceHeader.tsx**
- Header display component
- Workspace info display
- Click handler to open switcher

---

## Migration Path for Existing Users

### First Launch After Update

1. Detect existing single-project state
2. Auto-create default workspace:
   - Use existing project path and state
   - Generate workspace name from project
   - Associate all existing sessions with new workspace
   - Set as active workspace
3. Persist workspace to state
4. User sees identical UI to before (seamless transition)
5. Workspace feature available but not intrusive

### Migration Logic

- Check if workspaces collection is empty on startup
- If empty and project exists, create default workspace
- Migrate all session associations
- Set default workspace as active
- Emit workspace-created event for UI update

---

## Edge Cases & Considerations

### Session Resource Usage

**Problem**: Detached sessions continue consuming CPU/RAM

**Solutions**:
- Show total resource usage across all workspaces in status bar
- Badge on workspace switcher showing active session count per workspace
- Future: Option to pause/suspend detached sessions

### Terminal Buffer Memory

**Problem**: Many detached sessions = large memory footprint

**Solutions**:
- Configurable buffer size limits per session
- Future: Optional buffer compression for detached sessions
- Future: Option to clear buffer on detach

### Cross-Workspace File Conflicts

**Problem**: Two workspaces modifying same project path

**Solutions**:
- Warn if opening same project path in multiple workspaces
- Show conflict indicator in workspace switcher
- Future: File locking or merge conflict detection

### Workspace Cleanup

**Problem**: User closes app with many workspaces

**Solutions**:
- Persist workspace list on exit
- Restore workspaces on launch
- Future: Option to auto-close workspace sessions on app exit
- Future: Option to remove empty workspaces on exit

### Event Storm

**Problem**: Many workspaces = many events flooding frontend

**Solutions**:
- Frontend filters events by active workspace for rendering
- Backend still processes all events
- Future: Throttle metrics updates for inactive workspaces
- Future: Batch events for detached workspaces

### No Active Workspace

**Problem**: User closes active workspace, no workspace selected

**Solutions**:
- Auto-switch to another workspace if available
- Or show "no workspace" state with prompt to create/open
- Prevent closing last workspace without confirmation

### Workspace with No Sessions

**Problem**: Empty workspace may confuse users

**Solutions**:
- Show helpful message in dashboard when workspace has no sessions
- Provide quick action to create first session
- Option to auto-close empty workspaces

---

## Testing Checklist

### Backend Tests

**Workspace CRUD**
- Create workspace with valid path
- Create workspace with invalid path (error handling)
- Create workspace with non-existent path
- Switch between workspaces
- Switch to non-existent workspace (error)
- List workspaces returns all workspaces
- Close workspace without sessions
- Close workspace with sessions (close_sessions = true)
- Close workspace with sessions (close_sessions = false)
- Close last workspace (confirmation behavior)

**Session Association**
- Create session in workspace A
- Verify session has workspace_id
- Verify workspace session_ids list updated
- Switch to workspace B
- Verify session in A still running
- Switch back to workspace A
- Verify session still interactive

**Tab Association**
- Create tab in workspace A
- Verify tab has workspace_id
- Verify workspace tab_ids list updated
- Switch workspace, tab persists

**Event Emission**
- Workspace-created event on create
- Workspace-switched event on switch
- Workspace-removed event on close
- Session events include workspace_id

**Migration**
- Start with existing single-project state
- Verify default workspace created
- Verify sessions migrated
- Verify active workspace set

### Frontend Tests

**Workspace Switcher**
- Renders list of all workspaces
- Shows active workspace indicator
- Shows session counts
- Click switches workspace
- Create workspace action works
- Close workspace action shows confirmation
- Close workspace action works

**Workspace Header**
- Displays current workspace name
- Click opens switcher
- Updates on workspace switch

**Session Filtering**
- Sessions filter by active workspace
- Switching workspace updates displayed sessions
- Session creation adds to active workspace

**Tab Filtering**
- Tabs filter by active workspace
- Switching workspace updates displayed tabs

**State Persistence**
- Workspace list persists across app restart
- Active workspace restored on launch
- Session state preserved across workspace switches

### Integration Tests

**Detach/Attach Flow**
- Create session in workspace A
- Switch to workspace B
- Verify session A still running (check metrics)
- Switch back to workspace A
- Verify session A interactive
- Verify terminal buffer preserved

**Multi-Workspace Workflow**
- Create workspace A with sessions
- Create workspace B with sessions
- Switch between A and B multiple times
- Verify both workspaces maintain state
- Verify sessions in both remain functional

**Resource Tracking**
- Create sessions in multiple workspaces
- Verify status bar shows total resources
- Verify per-workspace session counts accurate

---

## Future Enhancements (Phase 2+)

### Workspace Templates
- Pre-configured workspace setups
- Save workspace configuration
- Load template when creating workspace

### Session Migration
- Move sessions between workspaces
- Drag-and-drop session to different workspace
- Copy session to multiple workspaces

### Workspace Groups
- Organize workspaces into categories
- Collapse/expand groups
- Group by project, client, or custom tags

### Auto-Save Sessions
- Persist session state across app restarts
- Restore sessions when reopening workspace
- Configurable auto-save behavior

### Workspace Search
- Quick switcher (Ctrl+P style)
- Search workspaces by name or path
- Fuzzy matching

### Per-Workspace Preferences
- Different default strategies per workspace
- Workspace-specific settings
- Custom workspace icons or colors

### Workspace Export/Import
- Export workspace configuration
- Share workspace setups
- Import workspace from config file

### Workspace Analytics
- Time spent per workspace
- Session history per workspace
- Productivity metrics

---

## API Reference Summary

### Backend Commands

| Command | Input | Output | Description |
|---------|-------|--------|-------------|
| create_workspace | name, path | WorkspaceContext | Create new workspace from project path |
| switch_workspace | workspace_id | WorkspaceContext | Switch active workspace |
| list_workspaces | none | Array of WorkspaceContext | List all workspaces |
| close_workspace | workspace_id, close_sessions | none | Close workspace |
| get_active_workspace | none | WorkspaceContext | Get current active workspace |

### Frontend API Methods

| Method | Parameters | Returns | Description |
|--------|------------|---------|-------------|
| createWorkspace | name, path | WorkspaceContext | Create new workspace |
| switchWorkspace | workspaceId | WorkspaceContext | Switch to workspace |
| listWorkspaces | none | WorkspaceContext[] | Get all workspaces |
| closeWorkspace | workspaceId, closeSessions | void | Close workspace |
| getActiveWorkspace | none | WorkspaceContext | Get active workspace |

### Event Listeners

| Event | Payload | Description |
|-------|---------|-------------|
| onWorkspaceSwitched | WorkspaceContext | Fired when workspace switched |
| onWorkspaceCreated | WorkspaceContext | Fired when workspace created |
| onWorkspaceRemoved | workspaceId | Fired when workspace closed |

---

## Implementation Phases

### Phase 1: Core Backend Infrastructure
- Add WorkspaceContext model to models.rs
- Update SentinelState structure
- Implement workspace CRUD operations
- Update session/tab creation to associate with workspace
- Update bootstrap command

### Phase 2: Event System & Type Updates
- Add workspace_id to all relevant events
- Update TypeScript type definitions
- Update SentinelApi interface
- Add new API methods

### Phase 3: UI Components
- Build WorkspaceSwitcher component
- Build WorkspaceHeader component
- Update App.tsx state management
- Implement session/tab filtering
- Add event listeners

### Phase 4: Migration & Polish
- Implement migration for existing users
- Add confirmation dialogs
- Update status bar with workspace info
- Error handling and edge cases
- Testing and bug fixes

### Phase 5: Documentation & Release
- Update user documentation
- Update README with workspace feature
- Release notes
- User feedback collection

---

## Success Criteria

### Functional Requirements
- User can create multiple workspaces
- User can switch between workspaces instantly
- Sessions continue running when detached
- Terminal buffers preserved on reattach
- All existing functionality works unchanged

### Performance Requirements
- Workspace switch completes in under 100ms
- No noticeable performance degradation with multiple workspaces
- Memory usage scales reasonably with detached sessions

### UX Requirements
- Workspace switcher is intuitive and discoverable
- Active workspace is always clearly indicated
- Migration is seamless for existing users
- Error states are handled gracefully with clear messages

---

## Conclusion

This implementation provides tmux-like detach/attach functionality:
- **Detach**: Switch workspace, sessions continue running
- **Attach**: Switch back, full terminal history preserved
- **Multi-project**: Work with multiple codebases simultaneously
- **Unified view**: See all workspaces, switch instantly

The architecture maintains backward compatibility while enabling powerful multi-workspace workflows. The implementation is phased to allow incremental development and testing, with a clear migration path for existing users.
