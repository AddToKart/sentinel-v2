# Swarm Dashboard Feature Plan

## Overview

This document describes the implementation plan for the **Swarm Dashboard** feature—a CLI-first agent orchestration system that transforms Sentinel into an intelligent multi-agent coordination platform.

### Core Philosophy: CLI-Native Agents

Agents in Sentinel **are not custom-built LLM wrappers**. They are real, production CLI tools—Gemini CLI, Claude Code, Codex CLI, Qwen CLI, Kimi CLI, Aider, OpenCode, etc.—running inside real PTY terminal sessions. Sentinel orchestrates these CLIs, it does not replace them.

**Why CLI-native?**
- **Zero reinvention**: Every major AI provider already ships a battle-tested CLI. Building custom frontends for each provider is redundant, fragile, and always behind.
- **Real terminal output**: Users see the actual CLI working—streaming tokens, running commands, editing files—not a sanitized abstraction.
- **Per-CLI configuration**: Each CLI has its own config files, environment variables, and flags. Users configure them the same way they would outside Sentinel.
- **Provider independence**: New CLIs can be added by simply registering the launch command. No new Rust modules, no new API integrations.
- **Composability**: CLIs can be chained, piped, and scripted using standard shell primitives.

### What Sentinel Adds

Sentinel is not the agent—it is the **control plane**:
- **Multi-terminal orchestration**: Deploy N CLI agents across N PTY sessions in one click
- **Visual coordination**: Network view shows which agent is doing what
- **Terminal dashboard**: See all agent terminals simultaneously in a tiled/tabbed layout
- **Inter-agent communication**: File-based mailbox system that CLIs can read/write
- **Task tracking**: Track what each agent was asked to do and what it produced
- **Workspace isolation**: Each swarm is scoped to a workspace with no cross-contamination

---

## Design Principles

### 1. Fortress Isolation
- Each workspace maintains complete isolation
- Agent swarms exist only within their deployed workspace
- No cross-workspace agent communication
- All agent activity logged to workspace-scoped SQLite database

### 2. CLI-First, Not CLI-Wrapped
- Agents **are** CLIs, not abstractions over CLIs
- Sentinel launches, monitors, and routes between CLIs—it does not intercept or reinterpret their output
- Users configure CLIs using the CLI's own native config mechanisms
- The terminal view is the primary view, not a debug afterthought

### 3. Orchestrator as CLI
- The Orchestrator is also a CLI agent (e.g., Claude Code or Gemini CLI)
- It receives the user's high-level prompt and coordinates by writing task files that other agents pick up
- No custom orchestration engine—the LLM inside the orchestrator CLI does the reasoning

### 4. Transparency & Control
- Terminal View shows every agent's real PTY output in a multi-pane layout
- Network View provides a high-level status overview
- Full audit trail of all agent activity in SQLite
- Users can type directly into any agent's terminal at any time

### 5. Minimal Backend Complexity
- No model provider traits, no API key management, no token counting
- The backend manages PTY lifecycles, file-based mailboxes, and SQLite logging
- All LLM interaction happens inside the CLI process—Sentinel never touches it

---

## Agent Architecture

### What Is an Agent?

An agent is a **CLI process** running in a **PTY session** with:
- A **role label** (orchestrator, frontend, backend, database, docs, or custom)
- A **CLI command** (e.g., `gemini`, `claude`, `codex`, `aider`, `qwen`, `kimi`)
- A **launch configuration** (CLI flags, env vars, working directory)
- A **mailbox directory** for inter-agent communication

### Supported CLI Tools (Extensible)

| CLI Tool | Provider | Install | Example Launch Command |
|---|---|---|---|
| Gemini CLI | Google | `npm i -g @anthropic-ai/gemini-cli` | `gemini --model gemini-2.5-pro` |
| Claude Code | Anthropic | `npm i -g @anthropic-ai/claude-code` | `claude --model claude-sonnet-4-5-20250929` |
| Codex CLI | OpenAI | `npm i -g @openai/codex` | `codex --model o4-mini` |
| Qwen CLI | Alibaba | `pip install qwen-cli` | `qwen chat --model qwen-max` |
| Kimi CLI | Moonshot | `pip install kimi-cli` | `kimi --model kimi-latest` |
| Aider | Aider | `pip install aider-chat` | `aider --model gpt-4o` |
| OpenCode | OpenCode | `go install opencode.dev@latest` | `opencode` |
| Custom | Any | User-defined | User-defined command |

> **Adding a new CLI**: Register a name + launch command. No code changes required.

### Agent Roles

Roles are **labels**, not hardcoded behaviors. The role determines:
- Default restricted paths (configurable)
- Position in the network visualization
- Which task types the orchestrator sends to it

| Role | Purpose | Default Restricted Paths |
|---|---|---|
| **Orchestrator** | Coordinates all agents, receives user prompts | None (full access) |
| **Frontend** | UI/UX, components, styling, client-side logic | `/backend/**`, `/database/**`, `/.env*` |
| **Backend** | Server logic, APIs, auth, business rules | `/frontend/**`, `/src/renderer/**` |
| **Database** | Schema, migrations, queries, data modeling | `/frontend/**`, `/backend/src/**` |
| **Documentation** | Docs, READMEs, changelogs, API docs | All source code (read-only access) |
| **Custom** | User-defined role with custom label | User-defined |

---

## Inter-Agent Communication

### File-Based Mailbox System

Agents communicate through a workspace-scoped mailbox directory. This is simple, debuggable, and works with any CLI that can read/write files.

```
.sentinel/swarm/
├── mailbox/
│   ├── orchestrator/
│   │   ├── inbox/
│   │   │   ├── 001_from_frontend_status_update.md
│   │   │   └── 002_from_backend_task_complete.md
│   │   └── outbox/
│   │       ├── 001_to_frontend_build_checkout.md
│   │       └── 002_to_backend_implement_api.md
│   ├── frontend/
│   │   ├── inbox/
│   │   └── outbox/
│   ├── backend/
│   │   ├── inbox/
│   │   └── outbox/
│   └── ...
├── tasks/
│   ├── active/
│   │   ├── task_001_checkout_ui.md
│   │   └── task_002_payment_api.md
│   ├── completed/
│   └── blocked/
└── config/
    ├── swarm.toml          # Swarm-level configuration
    ├── orchestrator.toml   # Per-agent CLI config overrides
    ├── frontend.toml
    └── ...
```

### Message Format

```markdown
---
id: msg_001
from: orchestrator
to: frontend
type: task_assignment
priority: high
timestamp: 2025-03-25T10:23:15Z
---

## Task: Build Checkout Flow

Build the checkout page component with the following requirements:
- Cart summary sidebar
- Payment form (Stripe integration)
- Address form with validation
- Order confirmation step

### Context
- Design mockups are in `/docs/designs/checkout/`
- Backend API spec is in `/docs/api/checkout.yaml`
- Use the existing `<Button>` and `<Input>` components

### Constraints
- Do not modify any backend files
- Use the existing design system tokens
```

### How Orchestration Works

1. **User sends prompt** → Sentinel writes it to the Orchestrator CLI's stdin
2. **Orchestrator CLI reasons** → Breaks the task down, writes task files to `.sentinel/swarm/tasks/active/`
3. **Sentinel detects new tasks** → Routes task files to agent inboxes via filesystem watcher
4. **Agent CLIs pick up tasks** → Sentinel injects the task content into each agent's stdin (or the CLI reads from the mailbox)
5. **Agents work** → Real terminal output visible in Terminal View
6. **Agents complete** → Write status updates to their outbox
7. **Sentinel routes updates** → Moves completion messages to orchestrator's inbox
8. **Orchestrator synthesizes** → Reports back to user

> **Key insight**: Sentinel is a **router and display layer**, not an execution engine. The CLIs do all the thinking.

---

## SQLite Schema Design

All tables are workspace-scoped via `workspace_id` foreign key. The schema is simplified to track CLI processes rather than model providers.

### Agents Table
```sql
CREATE TABLE agents (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    role TEXT NOT NULL,                -- 'orchestrator', 'frontend', 'backend', 'database', 'documentation', 'custom'
    label TEXT,                        -- User-friendly display name
    cli_command TEXT NOT NULL,         -- Full launch command (e.g., 'claude --model claude-sonnet-4-5-20250929')
    cli_name TEXT NOT NULL,            -- CLI tool identifier (e.g., 'claude', 'gemini', 'codex')
    status TEXT NOT NULL,              -- 'idle', 'working', 'waiting', 'error', 'paused', 'stopped'
    pty_session_id TEXT,               -- Linked terminal PTY session
    pid INTEGER,                       -- OS process ID
    cwd TEXT,                          -- Working directory
    created_at INTEGER NOT NULL,
    last_active_at INTEGER,
    config_json TEXT,                  -- Agent-specific overrides (env vars, flags)
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id)
);
```

### Agent Messages Table
```sql
CREATE TABLE agent_messages (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    from_agent TEXT,                   -- Agent role/id
    to_agent TEXT,                     -- Agent role/id
    message_type TEXT NOT NULL,        -- 'task_assignment', 'status_update', 'completion', 'error', 'user_message'
    content TEXT NOT NULL,             -- Full message content (markdown)
    file_path TEXT,                    -- Path to the mailbox file
    status TEXT NOT NULL,              -- 'pending', 'delivered', 'read'
    created_at INTEGER NOT NULL,
    delivered_at INTEGER,
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id)
);
```

### Tasks Table
```sql
CREATE TABLE tasks (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    parent_task_id TEXT,               -- For subtask hierarchy
    assigned_to TEXT,                  -- Agent role/id
    title TEXT NOT NULL,
    description TEXT,
    status TEXT NOT NULL,              -- 'pending', 'active', 'blocked', 'completed', 'failed'
    priority TEXT DEFAULT 'normal',    -- 'low', 'normal', 'high', 'critical'
    task_file_path TEXT,               -- Path to the task file in .sentinel/swarm/tasks/
    result_summary TEXT,               -- Agent's completion output
    created_at INTEGER NOT NULL,
    started_at INTEGER,
    completed_at INTEGER,
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id),
    FOREIGN KEY (parent_task_id) REFERENCES tasks(id)
);
```

### Audit Log Table
```sql
CREATE TABLE audit_log (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    agent_id TEXT,
    action_type TEXT NOT NULL,         -- 'agent_started', 'agent_stopped', 'task_assigned', 'message_sent', 'user_input', 'file_modified'
    details TEXT,                      -- Human-readable description
    details_json TEXT,                 -- Structured data
    timestamp INTEGER NOT NULL,
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id)
);
```

---

## UI/UX Specifications

### Swarm Deployment Modal

#### Trigger
- Button in header: "Deploy Swarm"
- Keyboard shortcut: `Ctrl+Shift+S`

#### Modal Layout

```
┌──────────────────────────────────────────────────────────────────────┐
│  Deploy Agent Swarm                                           [×]   │
├──────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  Team Preset:                                                        │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │ ○ Full Stack       ○ Frontend Focus    ○ Backend Focus        │  │
│  │ ○ Docs Only        ○ Custom                                   │  │
│  └────────────────────────────────────────────────────────────────┘  │
│                                                                      │
│  Agent Configuration:                                                │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │ Role            CLI Tool            Launch Flags               │  │
│  │ ─────────────── ─────────────────── ────────────────────────── │  │
│  │ Orchestrator    [Claude Code ▼]     [--model claude-sonnet-4-5-20250929   ] │  │
│  │ Frontend        [Gemini CLI  ▼]     [--model gemini-2.5-pro  ] │  │
│  │ Backend         [Codex CLI   ▼]     [--model o4-mini         ] │  │
│  │ Database        [Aider       ▼]     [--model gpt-4o          ] │  │
│  │ Documentation   [Qwen CLI    ▼]     [--model qwen-max        ] │  │
│  │                                                                │  │
│  │ [+ Add Custom Agent]                                           │  │
│  └────────────────────────────────────────────────────────────────┘  │
│                                                                      │
│  Initial Prompt (sent to Orchestrator):                              │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │ Build a checkout flow with Stripe payment integration         │  │
│  │                                                                │  │
│  └────────────────────────────────────────────────────────────────┘  │
│                                                                      │
│  [Cancel]                                    [Deploy Swarm →]        │
└──────────────────────────────────────────────────────────────────────┘
```

**Key changes from original plan:**
- No API key fields—CLIs use their own auth (env vars, config files, OAuth)
- No temperature/token sliders—those are CLI flags, not Sentinel settings
- Launch flags are editable text, not dropdowns for model parameters
- Initial prompt field lets users kickstart the orchestrator immediately

---

### Terminal View (Primary View)

The Terminal View is the **primary** swarm view, not a debug toggle. This is where users see the real CLIs working.

#### Layout: Multi-Pane Terminal Dashboard

```
┌──────────────────────────────────────────────────────────────────────┐
│  Swarm: E-commerce Platform     [Terminal View ▼]  [⚙️ Swarm Config] │
├──────────────────────────────────────────────────────────────────────┤
│ [🤖 Orchestrator] [🎨 Frontend] [⚙️ Backend] [🗄️ Database] [📝 Docs]│
├─────────────────────────────────┬────────────────────────────────────┤
│                                 │                                    │
│  🤖 Orchestrator (Claude Code)  │  🎨 Frontend (Gemini CLI)          │
│  ── ── ── ── ── ── ── ── ── ── │  ── ── ── ── ── ── ── ── ── ── ── │
│  $ claude                       │  $ gemini                          │
│                                 │                                    │
│  I'll break this into 3 tasks:  │  Reading task from inbox...        │
│                                 │                                    │
│  1. Frontend: Build checkout    │  ✦ Building checkout component     │
│     page with cart summary      │                                    │
│  2. Backend: Create payment     │  Creating src/components/          │
│     API with Stripe             │    Checkout/index.tsx              │
│  3. Database: Add orders table  │                                    │
│                                 │  Writing component code...         │
│  Assigning tasks now...         │                                    │
│                                 │  ██████████░░░░░░░░ 55%            │
│  ✓ Task assigned to Frontend    │                                    │
│  ✓ Task assigned to Backend     │                                    │
│  ✓ Task assigned to Database    │                                    │
│                                 │                                    │
│  Waiting for agents...          │                                    │
│  _                              │  _                                 │
│                                 │                                    │
├─────────────────────────────────┼────────────────────────────────────┤
│                                 │                                    │
│  ⚙️ Backend (Codex CLI)          │  🗄️ Database (Aider)               │
│  ── ── ── ── ── ── ── ── ── ── │  ── ── ── ── ── ── ── ── ── ── ── │
│  $ codex                        │  $ aider                           │
│                                 │                                    │
│  Reading task assignment...     │  Reading task assignment...        │
│                                 │                                    │
│  Creating payment endpoint:     │  Creating migration:               │
│  POST /api/checkout/pay         │  20250325_add_orders.sql           │
│                                 │                                    │
│  _                              │  _                                 │
│                                 │                                    │
└─────────────────────────────────┴────────────────────────────────────┘
│ Activity: 5 tasks active │ 2 agents working │ 1 waiting │ Uptime: 3m │
└──────────────────────────────────────────────────────────────────────┘
```

#### Terminal Pane Features
- **Real PTY output**: Exactly what you'd see if you ran the CLI in a standalone terminal
- **Interactive**: Click into any pane and type—your input goes to that CLI's stdin
- **Resizable**: Drag borders to resize panes
- **Detachable**: Pop out any pane into a standalone terminal tab
- **Scrollback**: Full scrollback buffer per pane
- **Layout modes**: Grid (default), tabbed, horizontal split, vertical split

#### Tab Bar
- Each agent has a tab with role icon, CLI name, and status indicator
- Click tab to focus/maximize that agent's terminal
- Right-click for context menu: Restart, Stop, Clear, Detach, Configure

---

### Network Visualization View (Secondary View)

A high-level overview toggled via the view dropdown. Shows agent status and communication flow.

```
┌──────────────────────────────────────────────────────────────────────┐
│  Swarm: E-commerce Platform     [Network View ▼]  [⚙️ Swarm Config]  │
├──────────────────────────────────────────────────────────────────────┤
│                                                                      │
│                  ┌─────────────────────┐                             │
│                  │  🎨 Frontend Agent   │                             │
│                  │  Gemini CLI          │                             │
│                  │  🟢 Working          │                             │
│                  │  Task: Checkout UI   │                             │
│                  └──────────┬──────────┘                             │
│                             │                                        │
│                             │ task assigned                          │
│                             │                                        │
│              ┌──────────────┴──────────────┐                        │
│              │   🤖 ORCHESTRATOR           │                        │
│              │   Claude Code               │                        │
│              │   🟢 Monitoring             │                        │
│              │                             │                        │
│              │  [💬 Send Input] [⏹ Stop]   │                        │
│              └──────────────┬──────────────┘                        │
│                    ┌────────┴────────┐                               │
│                    │                 │                                │
│           ┌────────┴──────┐  ┌──────┴────────┐                      │
│           │ ⚙️ Backend     │  │ 🗄️ Database    │                      │
│           │ Codex CLI     │  │ Aider         │                      │
│           │ 🟢 Working    │  │ 🟡 Waiting    │                      │
│           │ Task: API     │  │ Task: Schema  │                      │
│           └───────────────┘  └───────────────┘                      │
│                                                                      │
│  ─────────────────────────────────────────────────────────────────── │
│  Activity Feed:                                                      │
│  • 10:23 — Orchestrator broke task into 3 subtasks                   │
│  • 10:23 — Frontend agent started "Build Checkout UI"                │
│  • 10:24 — Backend agent started "Create Payment API"                │
│  • 10:24 — Database agent waiting for schema review from Backend     │
│                                                                      │
│  [Open Terminal View]                                                │
└──────────────────────────────────────────────────────────────────────┘
```

#### Node States
- **🟢 Working**: CLI actively producing output
- **🟡 Waiting**: CLI idle, waiting for input or dependency
- **⚪ Idle**: CLI launched but no active task
- **🔴 Error**: CLI process crashed or returned error
- **⏹ Stopped**: CLI process terminated

#### Clicking a Node
- Single-click: Show agent details sidebar (CLI info, current task, recent output snippet)
- Double-click: Switch to Terminal View focused on that agent
- Right-click: Context menu (Restart, Stop, Send Input, View Terminal)

---

### Agent Configuration (Per-CLI Settings)

Instead of model parameters managed by Sentinel, each agent's configuration is the CLI's own config.

```
┌──────────────────────────────────────────────────────────────────────┐
│  Configure: Frontend Agent (Gemini CLI)                        [×]  │
├──────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  CLI Tool:        [Gemini CLI ▼]                                     │
│  Launch Command:  [gemini --model gemini-2.5-pro                   ] │
│                                                                      │
│  Environment Variables:                                              │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │ GEMINI_API_KEY    = ••••••••••••••••••••   [Show] [Test ✓]    │  │
│  │ GEMINI_MODEL      = gemini-2.5-pro                            │  │
│  │ [+ Add Variable]                                              │  │
│  └────────────────────────────────────────────────────────────────┘  │
│                                                                      │
│  Working Directory:                                                  │
│  [C:\workspace\ecommerce                                        📁] │
│                                                                      │
│  Workspace Restrictions:                                             │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │ Restricted Paths (agent cannot modify):                       │  │
│  │  /backend/**                                                  │  │
│  │  /database/migrations/**                                      │  │
│  │  /.env*                                                       │  │
│  │  [+ Add Path]                                                 │  │
│  └────────────────────────────────────────────────────────────────┘  │
│                                                                      │
│  CLI Config File (optional - passed via flag):                       │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │ # Custom Gemini CLI config                                    │  │
│  │ temperature: 0.7                                              │  │
│  │ system_prompt: "You are a frontend specialist..."             │  │
│  │                                                               │  │
│  └────────────────────────────────────────────────────────────────┘  │
│                                                                      │
│  [Cancel]                                    [Save Configuration]    │
└──────────────────────────────────────────────────────────────────────┘
```

**Key differences:**
- Launch command is a raw text field—full flexibility
- Environment variables are key-value pairs—CLI auth handled natively
- CLI config file is an optional text area for CLI-specific config
- No Sentinel-managed API keys, temperature sliders, or token limits

---

## Backend Implementation Plan

### Architecture: Lean PTY Orchestration

The Rust backend does **not** implement model providers. It manages:
1. **PTY sessions** for each CLI agent (reusing existing terminal infrastructure)
2. **Filesystem watchers** for the mailbox directory
3. **SQLite logging** for audit trail and task tracking
4. **Tauri event emission** for real-time UI updates

### New Rust Modules

```
src-tauri/src/sentinelApi/
├── swarm/
│   ├── mod.rs              # SwarmManager struct, lifecycle
│   ├── agent.rs            # Agent spawn/stop/restart (PTY wrapper)
│   ├── mailbox.rs          # File-based message routing + fs watcher
│   ├── tasks.rs            # Task file creation/tracking
│   └── db.rs               # SQLite operations for swarm tables
```

**What's NOT here**: No `providers/` directory. No `gemini.rs`, `claude.rs`, etc. The CLI handles all LLM communication internally.

### New Tauri Commands

```rust
// ── Swarm Lifecycle ──

#[tauri::command]
fn deploy_swarm(
    app: AppHandle,
    state: State<'_, Arc<SwarmManager>>,
    workspace_id: String,
    agents: Vec<AgentSpawnConfig>,   // role + cli_command + env_vars + cwd
    initial_prompt: Option<String>,  // Sent to orchestrator's stdin on launch
) -> Result<SwarmSummary, String>

#[tauri::command]
fn teardown_swarm(
    app: AppHandle,
    state: State<'_, Arc<SwarmManager>>,
    workspace_id: String,
) -> Result<(), String>

#[tauri::command]
fn get_swarm_status(
    state: State<'_, Arc<SwarmManager>>,
    workspace_id: String,
) -> Result<SwarmStatus, String>

// ── Agent Management ──

#[tauri::command]
fn restart_agent(
    app: AppHandle,
    state: State<'_, Arc<SwarmManager>>,
    workspace_id: String,
    agent_id: String,
) -> Result<(), String>

#[tauri::command]
fn stop_agent(
    app: AppHandle,
    state: State<'_, Arc<SwarmManager>>,
    workspace_id: String,
    agent_id: String,
) -> Result<(), String>

#[tauri::command]
fn send_input_to_agent(
    state: State<'_, Arc<SwarmManager>>,
    workspace_id: String,
    agent_id: String,
    input: String,               // Written to agent's PTY stdin
) -> Result<(), String>

// ── Mailbox ──

#[tauri::command]
fn route_message(
    app: AppHandle,
    state: State<'_, Arc<SwarmManager>>,
    workspace_id: String,
    from_agent: String,
    to_agent: String,
    message: String,
) -> Result<String, String>      // Returns message file path

#[tauri::command]
fn get_agent_messages(
    state: State<'_, Arc<SwarmManager>>,
    workspace_id: String,
    agent_id: String,
    limit: u32,
) -> Result<Vec<AgentMessage>, String>

// ── Task Tracking ──

#[tauri::command]
fn get_swarm_tasks(
    state: State<'_, Arc<SwarmManager>>,
    workspace_id: String,
    status_filter: Option<String>,
) -> Result<Vec<TaskSummary>, String>

// ── Visualization ──

#[tauri::command]
fn get_swarm_graph(
    state: State<'_, Arc<SwarmManager>>,
    workspace_id: String,
) -> Result<NetworkGraph, String>
```

### Reusing Existing Terminal Infrastructure

The swarm agents **reuse Sentinel's existing PTY terminal system** (the same infrastructure that powers IDE mode terminals):

- `SwarmManager` calls the existing `create_terminal` → gets a PTY session
- Each agent's terminal is tagged with `swarm:{workspace_id}:{role}` for isolation
- Terminal output is streamed to the frontend using existing Tauri events
- The `IdeTerminalGroup` component is extended (or a new `SwarmTerminalGrid` is created) to render multi-pane agent terminals

---

## Frontend Implementation Plan

### New React Components

```
src/renderer/src/components/swarm/
├── SwarmDashboard.tsx          # Main swarm container (replaces workspace view when active)
├── SwarmDeployModal.tsx        # Deploy swarm modal dialog
├── SwarmTerminalGrid.tsx       # Multi-pane terminal layout for agent CLIs
├── SwarmTerminalPane.tsx       # Individual agent terminal pane (wraps existing terminal)
├── NetworkVisualization.tsx    # D3.js/Canvas network graph view
├── AgentNode.tsx               # Individual agent node in network graph
├── AgentConfigModal.tsx        # Per-agent CLI configuration modal
├── ActivityFeed.tsx            # Real-time activity stream
└── SwarmStatusBar.tsx          # Bottom bar with swarm metrics
```

### New Hooks

```typescript
// src/renderer/src/hooks/useSwarm.ts
export function useSwarm(workspaceId: string) {
  // deploy/teardown swarm
  // get swarm status (polling or event-driven)
  // list agents with statuses
  // send input to agent
  // restart/stop agent
}

// src/renderer/src/hooks/useSwarmMessages.ts
export function useSwarmMessages(workspaceId: string) {
  // get messages for agent
  // route new message between agents
  // subscribe to new messages via Tauri events
}

// src/renderer/src/hooks/useSwarmGraph.ts
export function useSwarmGraph(workspaceId: string) {
  // get network graph data
  // compute layout positions
  // subscribe to status changes
}
```

### Updated Types

```typescript
// src/shared/types.ts (additions)

export type AgentRole = 'orchestrator' | 'frontend' | 'backend' | 'database' | 'documentation' | 'custom'

export type CliTool = 'gemini' | 'claude' | 'codex' | 'qwen' | 'kimi' | 'aider' | 'opencode' | 'custom'

export type AgentStatus = 'idle' | 'working' | 'waiting' | 'error' | 'stopped'

export interface AgentSpawnConfig {
  role: AgentRole
  customLabel?: string             // For 'custom' role
  cliTool: CliTool
  cliCommand: string               // Full launch command with flags
  envVars?: Record<string, string>  // Additional env vars
  cwd?: string                     // Working directory override
  restrictedPaths?: string[]       // Paths agent should not modify
  cliConfigContent?: string        // Optional config file content
}

export interface AgentSummary {
  id: string
  workspaceId: string
  role: AgentRole
  label: string
  cliTool: CliTool
  cliCommand: string
  status: AgentStatus
  ptySessionId: string
  activeTaskId?: string
  lastActiveAt: number
}

export interface SwarmStatus {
  workspaceId: string
  isDeployed: boolean
  agents: AgentSummary[]
  activeTaskCount: number
  completedTaskCount: number
  startedAt: number
  uptimeSeconds: number
}

export interface AgentMessage {
  id: string
  fromAgent: string
  toAgent: string
  messageType: string
  content: string
  filePath: string
  status: 'pending' | 'delivered' | 'read'
  createdAt: number
}

export interface TaskSummary {
  id: string
  assignedTo: string
  title: string
  description?: string
  status: 'pending' | 'active' | 'blocked' | 'completed' | 'failed'
  priority: 'low' | 'normal' | 'high' | 'critical'
  taskFilePath: string
  createdAt: number
  completedAt?: number
}

export interface NetworkGraph {
  nodes: NetworkGraphNode[]
  links: NetworkGraphLink[]
}

export interface NetworkGraphNode {
  id: string
  role: AgentRole
  label: string
  cliTool: CliTool
  status: AgentStatus
  currentTask?: string
}

export interface NetworkGraphLink {
  source: string
  target: string
  type: 'task_assignment' | 'status_update' | 'dependency'
  label?: string
}
```

---

## Implementation Phases

### Phase 1: Foundation — PTY Swarm (Weeks 1–2)
- [ ] Create `.sentinel/swarm/` directory structure on deploy
- [ ] `SwarmManager` Rust struct: spawn N CLI agents as PTY sessions
- [ ] Reuse existing terminal infrastructure for agent PTYs
- [ ] Tag agent terminals for isolation from IDE/standalone terminals
- [ ] `deploy_swarm` / `teardown_swarm` Tauri commands
- [ ] Send initial prompt to orchestrator stdin on deploy
- [ ] SQLite schema migration for swarm tables

### Phase 2: Terminal Dashboard UI (Weeks 3–4)
- [ ] `SwarmDeployModal` component with team presets + CLI config
- [ ] `SwarmTerminalGrid` — multi-pane terminal layout
- [ ] `SwarmTerminalPane` — individual agent pane wrapping existing terminal renderer
- [ ] Tab bar with agent role icons and status indicators
- [ ] Layout modes: grid, tabbed, horizontal, vertical
- [ ] Pane resize, focus, and detach interactions
- [ ] `SwarmStatusBar` with aggregate metrics

### Phase 3: Mailbox & Task Routing (Weeks 5–6)
- [ ] File-based mailbox directory management in Rust
- [ ] Filesystem watcher for new messages (`notify` crate)
- [ ] `route_message` Tauri command
- [ ] Task file creation and status tracking
- [ ] Orchestrator stdin injection for task routing
- [ ] `ActivityFeed` component with real-time updates
- [ ] SQLite logging for all messages and task transitions

### Phase 4: Network Visualization (Weeks 7–8)
- [ ] `NetworkVisualization` component (Canvas/D3.js)
- [ ] `AgentNode` with status indicators and current task display
- [ ] Connection lines with communication flow animation
- [ ] View toggle between Terminal View and Network View
- [ ] Node click interactions (focus terminal, show details)
- [ ] `useSwarmGraph` hook with real-time status subscription

### Phase 5: Configuration & Polish (Weeks 9–10)
- [ ] `AgentConfigModal` with CLI-native settings
- [ ] Agent restart/stop from UI
- [ ] Direct user input to any agent terminal
- [ ] Team preset save/load (workspace-scoped)
- [ ] Custom agent role support
- [ ] Error handling and recovery (auto-restart crashed CLIs)
- [ ] Keyboard shortcuts for swarm operations

---

## Success Metrics

### Functional Requirements
- [ ] User can deploy a swarm of 2–5 CLI agents in one click
- [ ] Each agent runs in a real, interactive PTY terminal
- [ ] User can type directly into any agent's terminal
- [ ] Orchestrator can distribute tasks to other agents via mailbox
- [ ] Terminal View shows all agents simultaneously
- [ ] Network View shows agent status and communication flow
- [ ] Agents can be individually restarted or stopped
- [ ] Swarm is fully torn down when workspace closes

### Performance Requirements
- [ ] Swarm deployment < 3 seconds (CLI launches are fast)
- [ ] Terminal output rendering < 16ms per frame (60fps)
- [ ] Mailbox message routing < 200ms
- [ ] Network graph renders < 100ms
- [ ] SQLite queries < 50ms

### UX Requirements
- [ ] Terminal View is intuitive—feels like a tiled terminal multiplexer
- [ ] Network View provides at-a-glance understanding of swarm state
- [ ] Configuration is straightforward—just CLI commands and env vars
- [ ] No learning curve for users familiar with the underlying CLIs
- [ ] View toggle is instant

---

## Risks & Mitigations

### Risk 1: CLI Compatibility Varies Widely
**Mitigation**: Sentinel only manages PTY I/O—it doesn't parse CLI output. Any CLI that runs in a terminal works. Provide a "Custom CLI" option for unlisted tools.

### Risk 2: Inter-Agent Communication Is Unreliable
**Mitigation**: File-based mailbox is simple and debuggable. Sentinel routes messages, not the CLIs themselves. If a CLI supports reading task files natively, great. If not, Sentinel injects task content via stdin.

### Risk 3: CLI Processes Crash
**Mitigation**: PTY health monitoring with configurable auto-restart. Status indicator turns red immediately. User can manually restart from UI or terminal.

### Risk 4: Orchestrator CLI Makes Poor Task Decompositions
**Mitigation**: User can always type directly into any agent's terminal, bypassing the orchestrator. The orchestrator is a suggestion engine, not a gatekeeper.

### Risk 5: Too Many Terminal Panes Are Overwhelming
**Mitigation**: Tabbed layout mode collapses to one pane at a time. Status indicators on tabs show activity. Notification badges for important events.

### Risk 6: File-Based Mailbox Creates Disk Clutter
**Mitigation**: Auto-cleanup old messages on swarm teardown. Configurable retention period. Messages are small markdown files.

---

## Future Enhancements

- [ ] **Custom agent roles**: User-defined roles beyond the 5 built-in ones
- [ ] **Agent templates**: Save/share swarm configurations as templates
- [ ] **Pipeline mode**: Chain agents sequentially (output of one → input of next)
- [ ] **Watch mode**: Agents automatically react to file changes in their domain
- [ ] **Multi-workspace read**: Orchestrator can reference other workspaces (read-only)
- [ ] **Conversation replay**: Scroll back through an agent's full session
- [ ] **Team collaboration**: Share swarm configs with team members
- [ ] **Webhook triggers**: Start swarms from CI/CD or external events
- [ ] **Resource monitoring**: Track CPU/memory per agent process
- [ ] **Agent marketplace**: Community-shared agent configs and prompts

---

## Conclusion

The Swarm Dashboard transforms Sentinel into a CLI-native agent orchestration platform. Rather than reinventing AI providers behind custom abstractions, Sentinel leverages the CLIs that developers already know and trust:

1. **CLI-Native Agents**: Real CLIs (Gemini, Claude Code, Codex, Aider, etc.) in real terminals
2. **Terminal-First UX**: Multi-pane terminal dashboard as the primary view
3. **Zero Provider Lock-in**: Any CLI that runs in a terminal works—no custom integration needed
4. **File-Based Communication**: Simple, debuggable mailbox system for inter-agent coordination
5. **Lean Backend**: PTY management + filesystem watcher + SQLite. No API wrappers.
6. **Full User Control**: Type into any terminal, configure any CLI, override any decision

This positions Sentinel as the **tmux for AI agents**—a coordination layer that makes multi-agent workflows visible, manageable, and powerful without abstracting away the tools developers already use.
