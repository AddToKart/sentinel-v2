impl SentinelManager {
    pub fn read_file(&self, file_path: &str) -> String {
        fs::read_to_string(file_path).unwrap_or_default()
    }

    pub fn read_file_diff(&self, session_id: &str, file_path: &str) -> String {
        let summary = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
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

    pub fn write_session_file(
        &self,
        app: &AppHandle,
        session_id: &str,
        relative_path: &str,
        content: &str,
    ) -> Result<(), String> {
        let summary = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner
                .sessions
                .get(session_id)
                .map(|record| record.summary.clone())
                .ok_or_else(|| "Session not found.".to_string())?
        };
        let workspace_path = PathBuf::from(&summary.workspace_path);
        let absolute_path = resolve_workspace_target(&workspace_path, relative_path)?;
        let before_hash = if absolute_path.is_file() {
            Some(hash_file(&absolute_path)?)
        } else {
            None
        };

        write_workspace_file(&workspace_path, relative_path, content)?;

        let after_hash = if absolute_path.is_file() {
            Some(hash_file(&absolute_path)?)
        } else {
            None
        };
        let file_size = absolute_path
            .metadata()
            .ok()
            .map(|metadata| metadata.len() as i64);

        self.persist_file_change(
            app,
            session_id,
            &summary.workspace_id,
            &normalize_relative_path(relative_path)?,
            before_hash,
            after_hash,
            file_size,
        );

        Ok(())
    }
}
