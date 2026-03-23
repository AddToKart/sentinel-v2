fn cleanup_session_workspace(
    app: &AppHandle,
    project_root: &Path,
    workspace_path: &Path,
    workspace_strategy: SessionWorkspaceStrategy,
    branch_name: Option<&str>,
) -> Result<CleanupState, String> {
    match workspace_strategy {
        SessionWorkspaceStrategy::SandboxCopy => {
            if !workspace_path.exists() {
                return Ok(CleanupState::Removed);
            }
            fs::remove_dir_all(workspace_path)
                .map(|_| CleanupState::Removed)
                .map_err(|error| error.to_string())
        }
        SessionWorkspaceStrategy::GitWorktree => {
            let mut errors = Vec::new();
            if workspace_path.exists() {
                if let Err(error) =
                    run_git_command(Some(app), project_root, ["worktree", "remove", "--force", &path_to_string(workspace_path)])
                {
                    errors.push(error);
                }
            }
            if let Some(branch_name) = branch_name {
                if let Err(error) = run_git_command(Some(app), project_root, ["branch", "-D", branch_name]) {
                    errors.push(error);
                }
            }
            if workspace_path.exists() {
                if let Err(error) = fs::remove_dir_all(workspace_path) {
                    errors.push(error.to_string());
                }
            }
            if errors.is_empty() || !workspace_path.exists() {
                Ok(CleanupState::Removed)
            } else {
                Err(errors.join(" "))
            }
        }
    }
}

fn update_workspace_summary(inner: &mut SentinelState) {
    let active_sessions = inner
        .sessions
        .values()
        .filter(|record| {
            matches!(
                record.summary.status,
                SessionStatus::Starting | SessionStatus::Ready | SessionStatus::Closing
            )
        })
        .collect::<Vec<_>>();

    inner.workspace_summary = WorkspaceSummary {
        active_sessions: active_sessions.len(),
        total_cpu_percent: round(
            active_sessions
                .iter()
                .map(|record| record.summary.metrics.cpu_percent)
                .sum::<f64>(),
            1,
        ),
        total_memory_mb: round(
            active_sessions
                .iter()
                .map(|record| record.summary.metrics.memory_mb)
                .sum::<f64>(),
            1,
        ),
        total_processes: active_sessions
            .iter()
            .map(|record| record.summary.metrics.process_count)
            .sum(),
        last_updated: now_millis(),
        default_session_strategy: inner.preferences.default_session_strategy,
        project_path: inner.project.path.clone(),
        project_name: inner.project.name.clone(),
        branch: inner.project.branch.clone(),
    };
}

