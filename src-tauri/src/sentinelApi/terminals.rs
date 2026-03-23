impl SentinelManager {
    fn spawn_session_terminal(
        self: &Arc<Self>,
        app: AppHandle,
        session_id: String,
        cwd: PathBuf,
        cols: u16,
        rows: u16,
        workspace_strategy: SessionWorkspaceStrategy,
        branch_name: Option<String>,
    ) -> Result<TerminalHandles, String> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| error.to_string())?;

        let mut cmd = CommandBuilder::new("powershell.exe");
        cmd.arg("-NoLogo");
        cmd.cwd(&cwd);
        cmd.env("FORCE_COLOR", "1");
        cmd.env("SENTINEL_SESSION_ID", &session_id);
        cmd.env("SENTINEL_WORKSPACE_PATH", path_to_string(&cwd));
        cmd.env(
            "SENTINEL_WORKSPACE_MODE",
            match workspace_strategy {
                SessionWorkspaceStrategy::SandboxCopy => "sandbox-copy",
                SessionWorkspaceStrategy::GitWorktree => "git-worktree",
            },
        );
        cmd.env("SENTINEL_BRANCH", branch_name.unwrap_or_default());

        let mut child = pair.slave.spawn_command(cmd).map_err(|error| error.to_string())?;
        let pid = child.process_id();
        let killer = child.clone_killer();
        let mut reader = pair.master.try_clone_reader().map_err(|error| error.to_string())?;
        let writer = pair.master.take_writer().map_err(|error| error.to_string())?;
        let master = Arc::new(Mutex::new(pair.master));
        let writer = Arc::new(Mutex::new(writer));
        let killer = Arc::new(Mutex::new(killer));

        {
            let manager = self.clone();
            let app_handle = app.clone();
            let event_session_id = session_id.clone();
            thread::spawn(move || {
                let mut buffer = [0_u8; 4096];
                loop {
                    match reader.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(size) => {
                            let chunk = String::from_utf8_lossy(&buffer[..size]).to_string();
                            manager.handle_session_output(&app_handle, &event_session_id, chunk);
                        }
                        Err(_) => break,
                    }
                }
            });
        }

        {
            let manager = self.clone();
            let app_handle = app.clone();
            thread::spawn(move || {
                let exit_code = child.wait().ok().map(|status| status.exit_code() as i32);
                manager.finalize_session(app_handle, session_id, exit_code, None);
            });
        }

        Ok(TerminalHandles {
            master,
            writer,
            killer,
            pid,
        })
    }

    fn spawn_ide_terminal(
        self: &Arc<Self>,
        app: AppHandle,
        cwd: PathBuf,
        cols: u16,
        rows: u16,
        project_root: PathBuf,
    ) -> Result<TerminalHandles, String> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| error.to_string())?;

        let mut cmd = CommandBuilder::new("powershell.exe");
        cmd.arg("-NoLogo");
        cmd.cwd(&cwd);
        cmd.env("FORCE_COLOR", "1");
        cmd.env("SENTINEL_IDE_TERMINAL", "1");
        cmd.env("SENTINEL_PROJECT_ROOT", path_to_string(&project_root));
        cmd.env("SENTINEL_WORKSPACE_PATH", path_to_string(&cwd));
        cmd.env("SENTINEL_WORKSPACE_MODE", "sandbox-copy");

        let mut child = pair.slave.spawn_command(cmd).map_err(|error| error.to_string())?;
        let pid = child.process_id();
        let killer = child.clone_killer();
        let mut reader = pair.master.try_clone_reader().map_err(|error| error.to_string())?;
        let writer = pair.master.take_writer().map_err(|error| error.to_string())?;
        let master = Arc::new(Mutex::new(pair.master));
        let writer = Arc::new(Mutex::new(writer));
        let killer = Arc::new(Mutex::new(killer));

        {
            let manager = self.clone();
            let app_handle = app.clone();
            thread::spawn(move || {
                let mut buffer = [0_u8; 4096];
                loop {
                    match reader.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(size) => {
                            let chunk = String::from_utf8_lossy(&buffer[..size]).to_string();
                            manager.handle_ide_output(&app_handle, chunk);
                        }
                        Err(_) => break,
                    }
                }
            });
        }

        {
            let manager = self.clone();
            let app_handle = app.clone();
            thread::spawn(move || {
                let exit_code = child.wait().ok().map(|status| status.exit_code() as i32);
                manager.finalize_ide_terminal(app_handle, exit_code, None);
            });
        }

        Ok(TerminalHandles {
            master,
            writer,
            killer,
            pid,
        })
    }
}
