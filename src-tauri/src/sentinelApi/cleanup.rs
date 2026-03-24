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

            // Try to remove the worktree
            if workspace_path.exists() {
                match run_git_command(Some(app), project_root, ["worktree", "remove", "--force", &path_to_string(workspace_path)]) {
                    Ok(_output) => {
                        eprintln!("[sentinel] Worktree removed successfully: {}", path_to_string(workspace_path));
                    }
                    Err(error) => {
                        eprintln!("[sentinel] Failed to remove worktree: {}", error);
                        errors.push(format!("worktree remove failed: {}", error));
                    }
                }
            }

            // Try to delete the branch
            if let Some(branch_name) = branch_name {
                match run_git_command(Some(app), project_root, ["branch", "-D", branch_name]) {
                    Ok(_output) => {
                        eprintln!("[sentinel] Branch deleted successfully: {}", branch_name);
                    }
                    Err(error) => {
                        eprintln!("[sentinel] Failed to delete branch {}: {}", branch_name, error);
                        errors.push(format!("branch delete failed ({}): {}", branch_name, error));
                    }
                }
            }

            // Final fallback: try filesystem removal
            if workspace_path.exists() {
                match fs::remove_dir_all(workspace_path) {
                    Ok(()) => {
                        eprintln!("[sentinel] Workspace dir removed via filesystem");
                    }
                    Err(error) => {
                        eprintln!("[sentinel] Failed to remove workspace dir: {}", error);
                        errors.push(format!("filesystem removal failed: {}", error.to_string()));
                    }
                }
            }

            // Return success if path is gone, otherwise report structured errors
            if !workspace_path.exists() {
                if !errors.is_empty() {
                    eprintln!("[sentinel] Workspace cleanup had errors but path was removed: {:?}", errors);
                }
                Ok(CleanupState::Removed)
            } else {
                eprintln!("[sentinel] Workspace cleanup failed - path still exists: {:?}", errors);
                Err(errors.join("; "))
            }
        }
    }
}

fn update_workspace_summary(inner: &mut SentinelState) {
    sync_active_project_to_workspace(inner);

    let active_session_records = inner
        .sessions
        .values()
        .filter(|record| {
            matches!(
                record.summary.status,
                SessionStatus::Starting | SessionStatus::Ready | SessionStatus::Closing
            )
        })
        .collect::<Vec<_>>();

    let active_workspace_id = inner.active_workspace_id.clone();
    let active_workspace = active_workspace_clone(inner);

    let active_workspace_session_count = active_workspace_id
        .as_deref()
        .map(|workspace_id| {
            active_session_records
                .iter()
                .filter(|record| record.summary.workspace_id == workspace_id)
                .count()
        })
        .unwrap_or(0);

    let active_workspace_tab_count = active_workspace
        .as_ref()
        .map(|workspace| workspace.tab_ids.len())
        .unwrap_or(0);

    inner.workspace_summary = WorkspaceSummary {
        active_sessions: active_session_records.len(),
        active_workspace_session_count,
        active_workspace_tab_count,
        workspace_count: inner.workspaces.len(),
        total_sessions: inner.sessions.len(),
        total_tabs: inner.tabs.len(),
        total_cpu_percent: round(
            active_session_records
                .iter()
                .map(|record| record.summary.metrics.cpu_percent)
                .sum::<f64>(),
            1,
        ),
        total_memory_mb: round(
            active_session_records
                .iter()
                .map(|record| record.summary.metrics.memory_mb)
                .sum::<f64>(),
            1,
        ),
        total_processes: active_session_records
            .iter()
            .map(|record| record.summary.metrics.process_count)
            .sum(),
        last_updated: now_millis(),
        default_session_strategy: inner.preferences.default_session_strategy,
        active_workspace_id,
        active_workspace_name: active_workspace.as_ref().map(|workspace| workspace.name.clone()),
        project_path: inner.project.path.clone(),
        project_name: inner.project.name.clone(),
        branch: inner.project.branch.clone(),
    };
}

