# AI Changes Manager Feature Plan

## Overview

This document describes the implementation plan for the **AI Changes Manager** — a floating sidebar panel that gives users complete visibility into which AI agents have modified which files, a diff viewer per agent, and a system for aggregating all agent sandboxes into a single unified sandbox from which changes can be pushed to the real project directory.

This feature is directly complementary to the Swarm Dashboard and is designed to work both in **Swarm (multi-agent)** mode and in **Local (single-agent)** mode.

---

## Core Concepts

### The Problem Being Solved

In the current architecture, each agent created in Local Mode runs inside its own isolated sandbox. This means:
- Agent A's file changes are not visible to Agent B
- There is no single consolidated view of what all agents changed together
- There is no clean mechanism to promote all changes at once to the real project directory
- Users have no visual indicator of which AI agent touched which file

### What This Feature Delivers

1. **Floating Changes Sidebar** — A toggleable side panel (like OpenCode's changes view) that shows a live list of every file that has been modified by any AI agent, grouped by agent, with `+` and `-` diff indicators
2. **Per-Agent Attribution** — Every file change is tagged to the agent that made it, with timestamps and agent identity
3. **Unified Changes Sandbox** — A virtual "master sandbox" that aggregates the file changes from all individual agent sandboxes into a single coherent view
4. **Push Controls** — Ability to push changes from the unified sandbox to the real project directory, OR push changes selectively from a specific agent's sub-sandbox only

---

## Feature Breakdown

---

### Feature 1: Floating Changes Sidebar (UI)

#### Concept

A toggle button (e.g., in the header or right edge of the window) opens a floating panel anchored to the right side of the screen. It does not replace or push the main content — it overlays it, similar to how OpenCode renders its changes panel.

#### Panel Layout

```
┌─────────────────────────────────────────────┐
│ AI Changes Manager               [×] [Push All ↑] │
├─────────────────────────────────────────────┤
│ 🅐 Frontend Agent (Gemini CLI)               │
│   ├── +12 -3   src/components/Checkout.tsx   │
│   ├── +5  -0   src/styles/checkout.css       │
│   └── +1  -1   src/shared/types.ts           │
│                                              │
│ 🅑 Backend Agent (Claude Code)               │
│   ├── +28 -6   src/api/payment.ts            │
│   └── +4  -0   src/api/stripe.ts             │
│                                              │
│ 🅒 Database Agent (Aider)                    │
│   └── +15 -0   migrations/20260401_orders.sql │
│                                              │
├─────────────────────────────────────────────┤
│ Unified Sandbox: 6 files changed             │
│ [View Diff] [Push All ↑] [Discard All]      │
└─────────────────────────────────────────────┘
```

#### UI Behavior

- **Toggle Button**: A button (icon of lines/diff symbol) in the app's header or toolbar opens/closes the panel
- **Panel Presence**: The panel floats over the main content; it does not reflow or resize the workspace/terminal area
- **Live Updates**: As agents write files to their sandboxes, the panel updates in real time — new file entries appear with a brief highlight animation
- **File Click**: Clicking a file entry opens an inline diff viewer showing exactly what that agent changed in that file
- **Agent Collapse**: Each agent section can be collapsed/expanded independently
- **Status Indicators**: Files show colored `+` (additions) and `-` (deletions) line counts at a glance
- **Agent Identity Badge**: Each section header shows the agent's role icon, label, and CLI tool name

---

### Feature 2: Per-Agent File Change Tracking

#### How Agent File Changes Are Detected

Each agent sandbox is a directory on disk. A filesystem watcher monitors each sandbox's working directory for file-system events (file create, modify, delete). When a change is detected:

1. The changed file path is recorded, associated with the agent that owns that sandbox
2. A snapshot diff is computed (previous state vs. current state)
3. The change is stored in the SQLite database with the agent ID, file path, diff content, and timestamp
4. A Tauri event is emitted to the frontend to update the Changes Manager panel in real time

#### Change Record Structure (Conceptual)

Each tracked change stores:
- **Agent ID** — which agent caused this change
- **Sandbox ID** — which sandbox directory the change is in
- **File path** — the relative path within the sandbox
- **Operation type** — created, modified, deleted, renamed
- **Diff** — unified diff of the before/after state
- **Timestamp** — when the change occurred
- **Committed to Unified Sandbox** — whether this change has been merged into the master sandbox

#### What Counts as a Change

- Any file write/create/delete within the agent's sandbox working directory
- Changes within `.sentinel/swarm/` mailbox files are tracked separately (as communication artifacts, not project changes)
- Binary files are tracked (created/deleted) but diffs are not computed — only a size indicator is shown

---

### Feature 3: The Unified Changes Sandbox

#### Concept

The Unified Sandbox is a special virtual "staging area" that aggregates the file changes from all active agent sub-sandboxes into one coherent view. Think of it as a git staging area, but across multiple AI agents.

#### How It Works

```
Agent A Sandbox               Agent B Sandbox              Agent C Sandbox
─────────────────             ─────────────────            ─────────────────
src/components/               src/api/                     migrations/
  Checkout.tsx (modified)       payment.ts (modified)        orders.sql (created)
  checkout.css (created)        stripe.ts (created)

                        ↓ merge into ↓

                      Unified Sandbox
               ─────────────────────────────
               src/
                 components/
                   Checkout.tsx   ← from Agent A
                   checkout.css   ← from Agent A
                 api/
                   payment.ts     ← from Agent B
                   stripe.ts      ← from Agent B
               migrations/
                 orders.sql       ← from Agent C
```

#### Conflict Detection in the Unified Sandbox

If two agents modify the same file, both versions are surfaced in the Changes Manager:

- The panel shows the conflict with a ⚠️ indicator on the file
- Both versions are shown side by side in the diff viewer
- The user can choose which version wins (or manually merge)
- A file can only be pushed to the real project directory after conflicts on that file are resolved

#### Sandbox Directory Structure (Conceptual)

```
.sentinel/
└── sandboxes/
    ├── agent-frontend-abc123/         ← Agent A's sub-sandbox
    │   └── [working copy of project files]
    ├── agent-backend-def456/          ← Agent B's sub-sandbox
    │   └── [working copy of project files]
    ├── agent-database-ghi789/         ← Agent C's sub-sandbox
    │   └── [working copy of project files]
    └── unified/                       ← Master merged sandbox
        └── [aggregated changes from all agents]
```

#### Merge Strategy

- When an agent writes a file to its sub-sandbox, the change is automatically mirrored into the unified sandbox
- If no conflict: the unified sandbox file is updated automatically
- If a conflict exists (two agents modified the same file): the unified sandbox marks the file as conflicted and does not auto-merge — user intervention is required
- The unified sandbox always contains a clean snapshot of what the project would look like if all agent changes were applied together

---

### Feature 4: Push Controls

#### Push From Unified Sandbox (Main Push)

The primary push action: takes everything in the unified sandbox and writes it to the real open project directory.

- **"Push All" button**: Pushes all non-conflicted files from the unified sandbox to the project
- Conflicted files are skipped and flagged — user must resolve them first
- A confirmation dialog shows exactly which files will be written/overwritten before execution
- After a successful push, the changed files are cleared from the unified sandbox and the agents' sub-sandboxes are marked as "synced"

#### Push From Sub-Sandbox (Agent-Level Push)

The user can also push from an individual agent's sandbox, bypassing the unified sandbox:

- Each agent section in the Changes Manager has a **"Push from [Agent Name]"** button
- This writes only that agent's changed files directly to the real project directory
- Useful when one agent has completed a fully isolated task (e.g., the Database agent added a migration) and the user wants to ship it independently
- A confirmation dialog is shown before pushing

#### Discard Controls

- **"Discard All"**: Clears all changes from all sandboxes (prompts for confirmation)
- **"Discard Agent Changes"**: Clears only a specific agent's sub-sandbox
- **"Discard File"**: Reverts a single file entry back to the state it was in before the agent modified it

#### Git Integration (Optional Enhancement)

If the open project directory is a git repository:
- After pushing, the Changes Manager can optionally auto-stage the changed files (`git add`) 
- An option to auto-commit with an AI-generated commit message (derived from the agent's task context) can be offered
- This is an opt-in feature toggled in settings — default is off

---

## Data Model Additions

### New SQLite Tables (Conceptual)

#### `agent_file_changes` Table

Stores every file change made by any agent across all sandboxes.

Fields:
- `id` — unique change ID
- `workspace_id` — scoped to a workspace
- `agent_id` — which agent caused this change
- `sandbox_id` — which sandbox directory
- `file_path` — relative path within the sandbox
- `operation` — `created`, `modified`, `deleted`, `renamed`
- `diff_content` — unified diff text (null for binaries)
- `additions` — count of added lines
- `deletions` — count of deleted lines
- `timestamp` — when the change was detected
- `unified_status` — `pending`, `merged`, `conflicted`, `pushed`, `discarded`

#### `unified_sandbox_state` Table

Tracks the current aggregated state of the unified sandbox.

Fields:
- `id` — unique record ID
- `workspace_id` — scoped to a workspace
- `file_path` — relative path in unified sandbox
- `source_agent_id` — which agent's version is currently canonical in unified sandbox
- `conflict_agent_ids` — list of conflicting agent IDs (if any)
- `status` — `clean`, `conflicted`, `pushed`
- `last_updated_at` — timestamp of last update

---

## Backend Implementation Plan

### New Rust Modules Needed

#### Filesystem Watcher per Sandbox

- Each agent sandbox directory needs a filesystem watcher that emits change events
- When a file event is detected, compute a diff against the previous known state of that file
- Write the change record to SQLite
- Emit a Tauri event to refresh the frontend Changes Manager panel

#### Unified Sandbox Merger

- A Rust service that subscribes to file change events from all sub-sandboxes
- On receiving a change, attempts to apply it to the unified sandbox
- Detects conflicts by checking if the same file has been modified by more than one agent since the last push
- Emits Tauri events when the unified sandbox changes (including conflict flags)

#### Push Service

- Handles copying files from the unified sandbox (or a sub-sandbox) to the real project directory
- Validates that there are no unresolved conflicts before a full push
- Optionally performs git staging after push if the project is a git repo
- Records push actions in the `audit_log` table

### New Tauri Commands (Conceptual)

- `get_agent_changes(workspace_id, agent_id?)` — returns list of file changes, optionally filtered by agent
- `get_unified_sandbox_state(workspace_id)` — returns the current unified sandbox file list with conflict status
- `push_unified_sandbox(workspace_id)` — writes all non-conflicted unified sandbox files to the project directory
- `push_agent_sandbox(workspace_id, agent_id)` — writes a specific agent's files to the project directory
- `resolve_conflict(workspace_id, file_path, winning_agent_id)` — resolves a file conflict in the unified sandbox
- `discard_agent_changes(workspace_id, agent_id?)` — clears changes from one or all agent sandboxes
- `discard_file_change(workspace_id, change_id)` — reverts a single file to its pre-change state

---

## Frontend Implementation Plan

### New Components Needed

#### `ChangesManagerPanel`
The root floating sidebar component. Renders the panel overlay anchored to the right side of the window. Manages open/close state, stores preference in local storage so the panel remembers whether it was open.

#### `ChangesToggleButton`
A button placed in the app's main header or toolbar. Shows a badge count of total changed files to give users a quick indicator even when the panel is closed.

#### `AgentChangesGroup`
A collapsible section within the panel representing one agent. Shows the agent's identity (role, CLI tool, label) and lists all files that agent has changed.

#### `ChangedFileRow`
A single file entry within an agent group. Shows: file path, operation type (created/modified/deleted), and the `+` / `-` line count indicators. Clickable to open the diff viewer.

#### `FileDiffViewer`
An inline (or modal) diff viewer that shows the unified diff for a specific agent + file combination. Shows additions in green, deletions in red, with line numbers.

#### `ConflictResolver`
A side-by-side view shown when a file in the unified sandbox has conflicting changes from two or more agents. Allows the user to pick a winner or manually edit the merged content.

#### `UnifiedSandboxSummary`
A footer section at the bottom of the Changes Manager panel that shows the total count of changed files in the unified sandbox, any unresolved conflicts, and the main "Push All" and "Discard All" action buttons.

### New Hooks Needed

#### `useChangesManager(workspaceId)`
- Subscribes to Tauri events for real-time file change updates
- Exposes the list of agent changes grouped by agent
- Exposes the unified sandbox state
- Provides mutation functions: push, discard, resolve conflict

#### `useAgentChanges(workspaceId, agentId)`
- Fetches and subscribes to changes for a specific agent
- Provides per-agent push and discard functions

---

## Integration with Existing Systems

### Integration with Swarm Mode

- When the Swarm Dashboard is active, the Changes Manager panel is always available as a toggle
- Each deployed agent automatically gets a sandbox assigned and the watcher starts
- Agent identity is pulled from the existing `agent_messages` and `agents` tables

### Integration with Local (Single-Agent) Mode

- In Local mode, a single agent runs in its own sandbox
- The Changes Manager still works — it shows just one agent group
- The unified sandbox in this case is essentially the same as the single agent's sandbox
- Push still works the same way — the user can push the single agent's changes to the project directory

### Integration with the Workspace System

- Sandboxes are scoped per workspace — each open project has its own sandbox directory under `.sentinel/sandboxes/`
- Closing a workspace does not auto-push anything — changes persist in the sandbox until the user explicitly pushes or discards
- On workspace re-open, previously unpushed sandbox changes are shown in the Changes Manager, allowing the user to continue from where they left off

---

## UI/UX Principles for This Feature

- **Non-Intrusive by Default**: The panel is closed by default on first launch. Users open it when they want it.
- **Always Up to Date**: Changes appear in real time — no manual refresh button needed.
- **Attribution is First-Class**: Every change clearly shows which agent made it. Users should never be confused about what changed or who changed it.
- **Safe Defaults**: Nothing is pushed to the real project directory without explicit user action. The sandbox is always a safe staging area.
- **Conflict-First**: Conflicts are surfaced prominently. The UI prevents pushing conflicted files silently.
- **Granular Control**: Users can push everything at once (unified), or push just one agent's changes, or push just one file, or discard at any of those levels.

---

## Implementation Phases

### Phase 1: Sandbox Infrastructure (Foundation)

- Define the sandbox directory structure under `.sentinel/sandboxes/`
- Implement per-agent sandbox creation on agent launch (Local and Swarm modes)
- Implement the filesystem watcher per sandbox in Rust
- Implement diff computation for text files on change detection
- Write changes to the new `agent_file_changes` SQLite table
- Emit Tauri events on new file changes

### Phase 2: Unified Sandbox Merger

- Implement the UnifiedSandboxMerger Rust service
- Implement conflict detection logic (two agents changed the same file)
- Implement the `unified_sandbox_state` table and its population
- Implement the merge algorithm for non-conflicted files
- Emit Tauri events on unified sandbox state changes

### Phase 3: Changes Manager UI (Read-Only)

- Implement the `ChangesToggleButton` in the app header
- Implement the `ChangesManagerPanel` floating overlay
- Implement `AgentChangesGroup` and `ChangedFileRow` components
- Implement `FileDiffViewer` for inspecting individual file diffs
- Implement `useChangesManager` hook with real-time Tauri event subscription
- Wire up the unified sandbox summary footer (no push buttons yet — read only)

### Phase 4: Push and Discard Controls

- Implement the `push_unified_sandbox` Tauri command (Push All)
- Implement the `push_agent_sandbox` Tauri command (per-agent push)
- Implement `discard_agent_changes` and `discard_file_change` Tauri commands
- Add "Push All", "Push from Agent", and "Discard" buttons to the UI
- Add confirmation dialogs before any push or discard action
- Show post-push success state (clear resolved files from the panel)

### Phase 5: Conflict Resolution

- Implement `resolve_conflict` Tauri command
- Implement the `ConflictResolver` side-by-side UI component
- Surface conflict indicators in `AgentChangesGroup` and `UnifiedSandboxSummary`
- Block "Push All" when unresolved conflicts exist
- Allow per-file conflict resolution

### Phase 6: Polish and Git Integration (Optional)

- Optional git auto-stage after push (if project is a git repo)
- Optional AI-generated commit message on push
- Agent change activity in the Network Visualization view (show a pulse when an agent writes a file)
- Keyboard shortcut for toggling the Changes Manager panel
- Panel persistence (remember last open/closed state)
- Animation when new file changes appear in the panel

---

## Open Questions to Resolve Before Implementation

1. **Sandbox initialization**: Should sandboxes be full copies of the project directory (expensive for large repos) or should they be delta-only (just tracking diffs without duplicating files)? A delta-only approach is more efficient but more complex to implement.

2. **Binary file handling**: How should binary files (images, compiled assets) be handled in the diff viewer? Should they be shown as a special entry with just a size indicator?

3. **Sandbox persistence across sessions**: Should sandbox changes persist if the app is closed and reopened, or should each app session start with a clean sandbox? Persistence is more powerful but increases storage requirements.

4. **Conflict resolution depth**: Should the conflict resolver support free-text manual merge (complex) or only "pick one side" resolution (simpler)? Start with "pick one side" for Phase 5.

5. **Sub-sandbox push ordering**: If Agent A and Agent B both changed `src/shared/types.ts`, and the user pushes Agent A's sandbox first, then Agent B's sandbox, the second push will silently overwrite the first. Should the UI warn about this? — Yes, it should warn.
