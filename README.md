# Sentinel

A powerful multi-agent workspace for managing AI coding agents in parallel. Built with Tauri v2, React 19, and Rust, Sentinel provides isolated sandboxed environments where you can spawn multiple AI agent sessions to work on code simultaneously.

![Sentinel Dashboard](https://via.placeholder.com/1200x600/0b1219/70a5c9?text=Sentinel+Dashboard)

## ✨ Features

### Core Capabilities

#### 🖥️ Multi-Tab Workspace System
- **Agents Dashboard** - Primary persistent tab showing all active AI agent sessions
- **Standalone Terminals** - Open unlimited independent terminal tabs initialized at your system root directory
- **Tab Management** - Switch between tabs seamlessly while maintaining process persistence
- **Smart Navigation** - Closing a tab automatically navigates to the next available terminal

#### 🤖 AI Agent Session Management
- **Multiple Concurrent Agents** - Run multiple AI coding agents in isolated environments
- **Workspace Strategies**:
  - **Sandbox Copy** - Create isolated copies of your project for safe experimentation
  - **Git Worktree** - Use Git worktrees for parallel development branches
- **Process Monitoring** - Real-time CPU, RAM, and process metrics for each session
- **Session Lifecycle** - Start, monitor, and close agent sessions with full control

#### 📝 Code Review & Integration
- **File Diff Tracking** - Automatically track all file changes made by agents
- **Apply Changes** - Sync agent modifications back to your main project with conflict detection
- **Commit Workflow** - Create Git commits directly from agent sessions
- **Discard Changes** - Revert agent modifications cleanly

#### 💻 IDE Mode
- **Monaco Editor Integration** - Built-in code editor with syntax highlighting
- **Integrated Terminal** - Persistent IDE terminal for your own development work
- **File Browser** - Navigate project structure with modification indicators
- **Split View** - Resizable panels for editor and terminal

#### 🎯 Terminal Experience
- **xterm.js Integration** - Full-featured terminal emulation with portable-pty backend
- **Auto-Resizing** - Terminals automatically fit their containers using ResizeObserver
- **Scrollback & History** - Full terminal history with mouse wheel support
- **Shell Integration** - Tracks command history and working directory changes

#### 📊 Real-Time Telemetry
- **Per-Tab Metrics** - View PID, CPU%, and RAM usage for the focused tab
- **Workspace Overview** - Aggregate metrics for all active sessions
- **Process Tree Tracking** - Monitor child processes spawned by agents
- **Live Updates** - Metrics refresh every second using sysinfo crate

#### 🎨 Industrial Design
- **High-Density UI** - Slim 24px tab bar with efficient space utilization
- **Rounded-None Aesthetic** - Sharp, industrial design language
- **Dark Theme** - Optimized for long coding sessions
- **Responsive Layout** - Strict 100vh with zero global scrolling

## 🏗️ Architecture

### Tech Stack

| Layer | Technology |
|-------|------------|
| **Desktop Framework** | Tauri v2 (Rust backend + Web frontend) |
| **Frontend** | React 19 + TypeScript |
| **Build Tool** | Vite |
| **Styling** | Tailwind CSS 3 |
| **Package Manager** | Bun |
| **Terminal** | xterm.js + portable-pty (Rust) |
| **Code Editor** | Monaco Editor |
| **Process Monitoring** | sysinfo crate |

### Project Structure

```
sentinel-tauri/
├── src/
│   ├── renderer/              # React frontend
│   │   ├── src/
│   │   │   ├── components/    # UI components
│   │   │   │   ├── AgentDashboard.tsx
│   │   │   │   ├── SessionTile.tsx
│   │   │   │   ├── WorkspaceTabs.tsx
│   │   │   │   ├── StandaloneTerminalTile.tsx
│   │   │   │   ├── IdeTerminalPanel.tsx
│   │   │   │   ├── CodePreview.tsx
│   │   │   │   ├── StatusBar.tsx
│   │   │   │   └── ...
│   │   │   ├── App.tsx        # Main application
│   │   │   ├── tab-stream.ts  # Terminal output stream
│   │   │   └── ...
│   │   └── ...
│   └── shared/                # Shared TypeScript types
│       └── types.ts
├── src-tauri/                 # Rust backend
│   ├── src/
│   │   ├── sentinelApi/       # API modules
│   │   │   ├── app.rs         # Application lifecycle
│   │   │   ├── sessions.rs    # Session management
│   │   │   ├── tabs.rs        # Tab management
│   │   │   ├── terminals.rs   # PTY terminal spawning
│   │   │   ├── ide.rs         # IDE mode logic
│   │   │   ├── files.rs       # File operations
│   │   │   ├── sync.rs        # File synchronization
│   │   │   ├── tracking.rs    # Process & diff tracking
│   │   │   ├── workspace.rs   # Workspace strategies
│   │   │   └── ...
│   │   ├── lib.rs             # Tauri commands
│   │   ├── models.rs          # Data structures
│   │   └── main.rs
│   ├── Cargo.toml
│   └── ...
├── package.json
├── tsconfig.json
└── ...
```

## 🚀 Getting Started

### Prerequisites

- **Node.js** 18+ 
- **Bun** (recommended) or npm
- **Rust** 1.70+
- **Git**

### Installation

1. **Clone the repository**
   ```bash
   git clone https://github.com/your-org/sentinel-tauri.git
   cd sentinel-tauri
   ```

2. **Install dependencies**
   ```bash
   bun install
   ```

3. **Start development mode**
   ```bash
   bun tauri dev
   ```

4. **Build for production**
   ```bash
   bun tauri build
   ```

## 📖 Usage Guide

### Opening a Project

1. Click **Open Repository** in the header or use the Global Action Bar (`Ctrl+K`)
2. Select your Git repository or project folder
3. Sentinel will analyze the project structure and initialize

### Creating Agent Sessions

1. Click **+ New Agent** in the header
2. Choose workspace strategy (Sandbox Copy or Git Worktree)
3. Agent session spawns in isolated environment
4. Monitor metrics and file changes in real-time

### Managing Terminals

- **New Terminal**: Click the terminal icon in the header
- **Switch Tabs**: Click any tab in the tab bar
- **Close Tab**: Hover over tab and click × (or middle-click)
- **Navigation**: Closing a tab auto-navigates to next available tab

### Reviewing Agent Changes

1. Modified files appear in sidebar with indicators
2. Click file to open in IDE Mode
3. Review diff in **Diff** tab
4. Click **Apply** to sync changes to main project
5. Resolve conflicts if detected

### IDE Mode

- Toggle with `Ctrl+K` → "Switch to IDE Mode"
- Edit files directly with Monaco Editor
- Use integrated terminal for commands
- Switch back to Multiplex Mode anytime

## 🔧 Configuration

### Workspace Strategies

#### Sandbox Copy
- Creates isolated copy of project
- Safe for experimental changes
- Changes must be explicitly applied
- Best for: Testing, refactoring, high-risk changes

#### Git Worktree
- Uses Git worktrees for isolation
- Each session gets separate branch
- Native Git integration
- Best for: Feature development, parallel workflows

### Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+K` | Open Global Action Bar |
| `Ctrl+\`` | Toggle Console Drawer |
| `Ctrl+Shift+P` | Open Project |
| `Ctrl+Shift+N` | New Agent Session |

## 🛠️ Development

### Running Tests

```bash
# TypeScript type checking
bun run build

# Rust compilation check
cd src-tauri && cargo check

# Full build
bun tauri build
```

### Code Style

- **TypeScript**: Strict mode with no implicit any
- **Rust**: Follows Rustfmt defaults
- **Components**: Functional components with hooks
- **Naming**: PascalCase for components, camelCase for functions

### Adding New Features

1. **Define types** in `src/shared/types.ts`
2. **Add Rust models** in `src-tauri/src/models.rs`
3. **Implement backend logic** in appropriate `sentinelApi/*.rs` module
4. **Create Tauri command** in `src-tauri/src/lib.rs`
5. **Build frontend component** in `src/renderer/src/components/`
6. **Wire up events** in `App.tsx`

## 🎯 Roadmap

### Planned Features

#### Q2 2026
- [ ] **Session Templates** - Pre-configured agent setups for common tasks
- [ ] **Multi-Project Support** - Open multiple repositories simultaneously
- [ ] **Custom Shell Selection** - Choose between PowerShell, CMD, WSL, bash
- [ ] **Terminal Themes** - Customizable color schemes for terminals
- [ ] **Search Across Sessions** - Full-text search in terminal history

#### Q3 2026
- [ ] **Agent Collaboration** - Multiple agents working on same session
- [ ] **Session Recording** - Record and playback agent sessions
- [ ] **Plugin System** - Extend Sentinel with custom plugins
- [ ] **AI Integration** - Built-in LLM chat for quick questions
- [ ] **Workspace Snapshots** - Save and restore workspace states

#### Q4 2026
- [ ] **Remote Sessions** - Connect to remote development environments
- [ ] **Team Collaboration** - Share sessions with team members
- [ ] **Advanced Git Tools** - Interactive rebase, cherry-pick, bisect
- [ ] **Performance Profiling** - Deep dive into agent resource usage
- [ ] **Mobile Companion App** - Monitor sessions on the go

#### Future Considerations
- [ ] **VS Code Extension** - Integrate Sentinel into VS Code workflow
- [ ] **CI/CD Integration** - Trigger agent sessions from pipeline events
- [ ] **Custom Agent Scripts** - Define automated agent workflows
- [ ] **Cloud Sync** - Sync workspace state across devices
- [ ] **Extension Marketplace** - Community-built plugins and themes

## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

### Ways to Help

- 🐛 Report bugs and issues
- 💡 Suggest new features
- 📝 Improve documentation
- 🔧 Submit pull requests
- 🎨 Design UI improvements

## 📄 License

Sentinel is licensed under the [MIT License](LICENSE).

## 🙏 Acknowledgments

Built with amazing open-source projects:

- [Tauri](https://tauri.app/) - Desktop application framework
- [xterm.js](https://xtermjs.org/) - Terminal emulator
- [Monaco Editor](https://microsoft.github.io/monaco-editor/) - Code editor
- [portable-pty](https://docs.rs/portable-pty/latest/portable_pty/) - PTY library
- [sysinfo](https://docs.rs/sysinfo/latest/sysinfo/) - System information
- [React](https://react.dev/) - UI library
- [Tailwind CSS](https://tailwindcss.com/) - Utility-first CSS

---

**Sentinel** - Manage AI agents. Master your workflow.

Made with ❤️ by the Sentinel Team
