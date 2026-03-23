impl SentinelManager {
    pub fn apply_session(
        &self,
        app: &AppHandle,
        session_id: &str,
    ) -> Result<SessionApplyResult, String> {
        let session_info = {
            let inner = self.inner.lock().expect("state poisoned");
            let record = inner
                .sessions
                .get(session_id)
                .ok_or_else(|| "Session or project not found.".to_string())?;
            (
                record.summary.clone(),
                record.modified_paths.clone(),
                record.sandbox_state.clone(),
            )
        };

        let (summary, modified_paths, sandbox_state) = session_info;

        if summary.workspace_strategy == SessionWorkspaceStrategy::GitWorktree {
            if !modified_paths.is_empty() {
                return Err(
                    "This worktree still has uncommitted changes. Commit the worktree before merging it into the main project."
                        .to_string(),
                );
            }

            run_git_command(
                Some(app),
                Path::new(&summary.project_root),
                ["merge", summary.branch_name.as_deref().unwrap_or("")],
            )?;

            let _ = self.refresh_project_snapshot(app);
            return Ok(SessionApplyResult {
                session_id: summary.id,
                workspace_strategy: SessionWorkspaceStrategy::GitWorktree,
                applied_paths: Vec::new(),
                remaining_paths: Vec::new(),
                conflicts: Vec::new(),
            });
        }

        let sandbox_state =
            sandbox_state.ok_or_else(|| "Sandbox session state is unavailable.".to_string())?;
        self.apply_sandbox_session_changes(app, session_id, &summary, sandbox_state)
    }

    pub fn commit_session(
        &self,
        app: &AppHandle,
        session_id: &str,
        message: &str,
    ) -> Result<SessionCommitResult, String> {
        let session_info = {
            let inner = self.inner.lock().expect("state poisoned");
            let record = inner
                .sessions
                .get(session_id)
                .ok_or_else(|| "Session or project not found.".to_string())?;
            (
                record.summary.clone(),
                record.modified_paths.clone(),
                record.sandbox_state.clone(),
            )
        };

        let (summary, modified_paths, _sandbox_state) = session_info;
        let commit_message = message.trim();
        let commit_message = if commit_message.is_empty() {
            "Sentinel update"
        } else {
            commit_message
        };

        if summary.workspace_strategy != SessionWorkspaceStrategy::GitWorktree {
            return Err(
                "Sandbox sessions are local-only. Use Sync to Main Project Files instead of Commit."
                    .to_string(),
            );
        }

        if modified_paths.is_empty() {
            return Ok(SessionCommitResult {
                session_id: summary.id,
                workspace_strategy: SessionWorkspaceStrategy::GitWorktree,
                applied_paths: Vec::new(),
                committed_paths: Vec::new(),
                remaining_paths: Vec::new(),
                conflicts: Vec::new(),
                created_commit: false,
                commit_message: commit_message.to_string(),
                commit_hash: None,
            });
        }

        run_git_command(Some(app), Path::new(&summary.workspace_path), ["add", "."])?;
        run_git_command(
            Some(app),
            Path::new(&summary.workspace_path),
            ["commit", "-m", commit_message],
        )?;

        self.refresh_runtime_state(app);
        let remaining_paths = {
            let inner = self.inner.lock().expect("state poisoned");
            inner
                .sessions
                .get(session_id)
                .map(|record| record.modified_paths.clone())
                .unwrap_or_default()
        };

        Ok(SessionCommitResult {
            session_id: summary.id,
            workspace_strategy: SessionWorkspaceStrategy::GitWorktree,
            applied_paths: Vec::new(),
            committed_paths: modified_paths,
            remaining_paths,
            conflicts: Vec::new(),
            created_commit: true,
            commit_message: commit_message.to_string(),
            commit_hash: run_git_command(
                None,
                Path::new(&summary.workspace_path),
                ["rev-parse", "--short", "HEAD"],
            )
            .ok()
            .filter(|hash| !hash.is_empty()),
        })
    }

    pub fn discard_session_changes(
        &self,
        app: &AppHandle,
        session_id: &str,
    ) -> Result<(), String> {
        let session_info = {
            let inner = self.inner.lock().expect("state poisoned");
            let record = inner
                .sessions
                .get(session_id)
                .ok_or_else(|| "Session or project not found.".to_string())?;
            (record.summary.clone(), record.sandbox_state.clone())
        };

        let (summary, sandbox_state) = session_info;
        if summary.workspace_strategy == SessionWorkspaceStrategy::GitWorktree {
            run_git_command(Some(app), Path::new(&summary.workspace_path), ["reset", "--hard"])?;
            run_git_command(Some(app), Path::new(&summary.workspace_path), ["clean", "-fd"])?;
            self.refresh_runtime_state(app);
            return Ok(());
        }

        let _ = sandbox_state.ok_or_else(|| "Sandbox session state is unavailable.".to_string())?;
        self.push_activity_log(
            app,
            "workspace",
            "started",
            "Discard sandbox changes",
            summary.workspace_path.clone(),
            None,
        );

        match discard_sandbox_workspace(
            Path::new(&summary.project_root),
            Path::new(&summary.workspace_path),
        ) {
            Ok(next_state) => {
                {
                    let mut inner = self.inner.lock().expect("state poisoned");
                    if let Some(record) = inner.sessions.get_mut(session_id) {
                        record.sandbox_state = Some(next_state);
                        record.modified_paths.clear();
                    }
                }
                self.push_activity_log(
                    app,
                    "workspace",
                    "completed",
                    "Discard sandbox changes",
                    summary.workspace_path,
                    None,
                );
                self.emit_session_diff(app, session_id);
                Ok(())
            }
            Err(error) => {
                self.push_activity_log(
                    app,
                    "workspace",
                    "failed",
                    "Discard sandbox changes",
                    summary.workspace_path,
                    Some(error.clone()),
                );
                Err(error)
            }
        }
    }

    fn apply_sandbox_session_changes(
        &self,
        app: &AppHandle,
        session_id: &str,
        summary: &SessionSummary,
        sandbox_state: SandboxWorkspaceState,
    ) -> Result<SessionApplyResult, String> {
        self.push_activity_log(
            app,
            "workspace",
            "started",
            "Sync sandbox changes to main project files",
            summary.workspace_path.clone(),
            None,
        );

        match apply_sandbox_workspace(
            &summary.id,
            Path::new(&summary.project_root),
            Path::new(&summary.workspace_path),
            sandbox_state,
        ) {
            Ok(applied) => {
                let refreshed = refresh_sandbox_workspace_diffs(
                    Path::new(&summary.workspace_path),
                    &SandboxWorkspaceState {
                        baseline_hashes: applied.next_baseline_hashes.clone(),
                        scan_cache: applied.next_cache.clone(),
                    },
                )?;
                let mut result = applied.result;
                result.remaining_paths = refreshed.0.clone();

                {
                    let mut inner = self.inner.lock().expect("state poisoned");
                    if let Some(record) = inner.sessions.get_mut(session_id) {
                        record.sandbox_state = Some(SandboxWorkspaceState {
                            baseline_hashes: applied.next_baseline_hashes.clone(),
                            scan_cache: refreshed.1.clone(),
                        });
                        record.modified_paths = refreshed.0.clone();
                    }
                }

                self.push_activity_log(
                    app,
                    "workspace",
                    if result.conflicts.is_empty() { "completed" } else { "failed" },
                    "Sync sandbox changes to main project files",
                    summary.workspace_path.clone(),
                    Some(if result.conflicts.is_empty() {
                        format!(
                            "{} file(s) synced, {} remaining",
                            result.applied_paths.len(),
                            result.remaining_paths.len()
                        )
                    } else {
                        format!(
                            "{} synced, {} conflicts, {} remaining",
                            result.applied_paths.len(),
                            result.conflicts.len(),
                            result.remaining_paths.len()
                        )
                    }),
                );

                self.emit_session_diff(app, session_id);
                if !result.applied_paths.is_empty() {
                    let _ = self.refresh_project_snapshot(app);
                }

                Ok(result)
            }
            Err(error) => {
                self.push_activity_log(
                    app,
                    "workspace",
                    "failed",
                    "Sync sandbox changes to main project files",
                    summary.workspace_path.clone(),
                    Some(error.clone()),
                );
                Err(error)
            }
        }
    }
}
