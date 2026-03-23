impl SentinelManager {
    pub fn read_file(&self, file_path: &str) -> String {
        fs::read_to_string(file_path).unwrap_or_default()
    }

    pub fn read_file_diff(&self, session_id: &str, file_path: &str) -> String {
        let summary = {
            let inner = self.inner.lock().expect("state poisoned");
            inner.sessions.get(session_id).map(|record| record.summary.clone())
        };
        let Some(summary) = summary else {
            return String::new();
        };
        if summary.workspace_strategy != SessionWorkspaceStrategy::GitWorktree {
            return String::new();
        }
        run_git_command(None, Path::new(&summary.workspace_path), ["diff", "HEAD", "--", file_path])
            .unwrap_or_default()
    }

    pub fn write_session_file(&self, session_id: &str, relative_path: &str, content: &str) -> Result<(), String> {
        let workspace_path = {
            let inner = self.inner.lock().expect("state poisoned");
            let record = inner
                .sessions
                .get(session_id)
                .ok_or_else(|| "Session not found.".to_string())?;
            PathBuf::from(&record.summary.workspace_path)
        };
        write_workspace_file(&workspace_path, relative_path, content)
    }
}
