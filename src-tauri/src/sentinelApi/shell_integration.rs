impl SentinelManager {
    pub fn reveal_in_file_explorer(&self, file_path: &str) -> Result<(), String> {
        #[cfg(windows)]
        {
            let status = Command::new("explorer.exe")
                .args(["/select,", file_path])
                .status()
                .map_err(|error| error.to_string())?;
            if status.success() {
                Ok(())
            } else {
                Err("Failed to reveal file in File Explorer.".to_string())
            }
        }
        #[cfg(not(windows))]
        {
            let path = Path::new(file_path);
            let parent = path.parent().unwrap_or(path);
            self.open_in_system_editor(parent.to_string_lossy().as_ref())
        }
    }

    pub fn open_in_system_editor(&self, file_path: &str) -> Result<(), String> {
        #[cfg(windows)]
        {
            let status = Command::new("cmd")
                .args(["/C", "start", "", file_path])
                .status()
                .map_err(|error| error.to_string())?;
            if status.success() {
                Ok(())
            } else {
                Err("Failed to open file.".to_string())
            }
        }
        #[cfg(target_os = "macos")]
        {
            let status = Command::new("open")
                .arg(file_path)
                .status()
                .map_err(|error| error.to_string())?;
            if status.success() {
                Ok(())
            } else {
                Err("Failed to open file.".to_string())
            }
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            let status = Command::new("xdg-open")
                .arg(file_path)
                .status()
                .map_err(|error| error.to_string())?;
            if status.success() {
                Ok(())
            } else {
                Err("Failed to open file.".to_string())
            }
        }
    }

    pub fn dispose(&self, _app: &AppHandle) {
        let (session_pids, tab_pids, ide_pid, ide_workspace) = {
            let inner = self.inner.lock().expect("state poisoned");
            (
                inner
                    .sessions
                    .values()
                    .map(|record| record.summary.pid)
                    .collect::<Vec<_>>(),
                inner
                    .tabs
                    .values()
                    .map(|record| record.summary.pid)
                    .collect::<Vec<_>>(),
                inner.ide.record.as_ref().and_then(|record| record.state.pid),
                inner.ide.workspace_path.clone(),
            )
        };

        for pid in session_pids {
            let _ = terminate_process_id(pid);
        }
        for pid in tab_pids {
            let _ = terminate_process_id(pid);
        }
        let _ = terminate_process_id(ide_pid);
        if let Some(workspace_path) = ide_workspace {
            let _ = fs::remove_dir_all(workspace_path);
        }
    }
}
