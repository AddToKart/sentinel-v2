use crate::database::db_models::{AuditLogRow, CommandHistoryRow, FileChangeRow, WorkspaceSnapshotRow};
use crate::database::repositories::WorkspaceSnapshotRepository;
use sqlx::query_as;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredSnapshotFile {
    path: String,
    content_base64: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredWorkspaceSnapshotData {
    workspace: WorkspaceContext,
    files: Vec<StoredSnapshotFile>,
}

fn command_history_entry_from_row(row: CommandHistoryRow) -> CommandHistoryEntry {
    CommandHistoryEntry {
        id: row.id,
        session_id: row.session_id,
        workspace_id: row.workspace_id,
        command: row.command_text,
        timestamp: row.timestamp,
        source: row.source,
        exit_code: row.exit_code.map(|value| value as i32),
        duration_ms: row.duration_ms,
        cwd: row.cwd,
    }
}

fn file_change_entry_from_row(row: FileChangeRow) -> FileChangeEntry {
    FileChangeEntry {
        id: row.id,
        session_id: row.session_id,
        workspace_id: row.workspace_id,
        file_path: row.file_path,
        change_type: row.change_type,
        before_hash: row.before_hash,
        after_hash: row.after_hash,
        timestamp: row.timestamp,
        file_size: row.file_size,
    }
}

fn audit_log_entry_from_row(row: AuditLogRow) -> AuditLogEntry {
    AuditLogEntry {
        id: row.id,
        workspace_id: row.workspace_id,
        session_id: row.session_id,
        tab_id: row.tab_id,
        timestamp: row.timestamp,
        action_type: row.action_type,
        resource_type: row.resource_type,
        resource_id: row.resource_id,
        details: row.details,
        user_id: row.user_id,
    }
}

fn snapshot_summary_from_row(row: WorkspaceSnapshotRow) -> SnapshotSummary {
    SnapshotSummary {
        id: row.id,
        workspace_id: row.workspace_id,
        name: row.name,
        description: row.description,
        created_at: row.created_at,
        file_count: row.file_count,
        session_count: row.session_count,
    }
}

fn csv_escape(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn collect_snapshot_files(project_root: &Path) -> Result<Vec<StoredSnapshotFile>, String> {
    let tracked_files = list_tracked_files(project_root)?;
    let files = parallel_try_map(&tracked_files, |relative_path| {
        let absolute_path = resolve_workspace_target(project_root, relative_path)?;
        let bytes = fs::read(&absolute_path).map_err(|error| error.to_string())?;
        Ok(StoredSnapshotFile {
            path: relative_path.clone(),
            content_base64: BASE64_STANDARD.encode(bytes),
        })
    })?;

    let mut files = files;
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn restore_snapshot_files(
    project_root: &Path,
    snapshot_files: &[StoredSnapshotFile],
) -> Result<(), String> {
    let snapshot_paths = snapshot_files
        .iter()
        .map(|file| file.path.clone())
        .collect::<HashSet<_>>();

    for current_file in list_tracked_files(project_root)? {
        if snapshot_paths.contains(&current_file) {
            continue;
        }

        let absolute_path = resolve_workspace_target(project_root, &current_file)?;
        if absolute_path.exists() {
            fs::remove_file(&absolute_path).map_err(|error| error.to_string())?;
        }
    }

    for file in snapshot_files {
        let absolute_path = resolve_workspace_target(project_root, &file.path)?;
        if let Some(parent) = absolute_path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }

        let bytes = BASE64_STANDARD
            .decode(&file.content_base64)
            .map_err(|error| error.to_string())?;
        fs::write(&absolute_path, bytes).map_err(|error| error.to_string())?;
    }

    Ok(())
}

impl SentinelManager {
    pub fn search_command_history(
        &self,
        app: &AppHandle,
        workspace_id: &str,
        query: &str,
        limit: Option<i64>,
    ) -> Result<Vec<CommandHistoryEntry>, String> {
        let pool = database_pool(app);
        let query = query.trim();
        let rows = if query.is_empty() {
            tauri::async_runtime::block_on(CommandRepository::find_by_workspace(
                &pool,
                workspace_id,
                limit,
            ))
        } else {
            tauri::async_runtime::block_on(CommandRepository::search(
                &pool,
                workspace_id,
                query,
                limit,
            ))
        }
        .map_err(|error| format!("Failed to search command history: {error}"))?;

        Ok(rows.into_iter().map(command_history_entry_from_row).collect())
    }

    pub fn get_file_change_timeline(
        &self,
        app: &AppHandle,
        workspace_id: &str,
        file_path: Option<&str>,
        limit: Option<i64>,
    ) -> Result<Vec<FileChangeEntry>, String> {
        let pool = database_pool(app);
        let rows = match file_path
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(file_path) => tauri::async_runtime::block_on(FileChangeRepository::find_by_file(
                &pool,
                workspace_id,
                file_path,
            )),
            None => tauri::async_runtime::block_on(async {
                let rows = query_as::<_, FileChangeRow>(
                    r#"
                    SELECT * FROM file_changes
                    WHERE workspace_id = ?1
                    ORDER BY timestamp DESC
                    LIMIT ?2
                    "#,
                )
                .bind(workspace_id)
                .bind(limit.unwrap_or(200))
                .fetch_all(&pool)
                .await?;
                Ok::<_, sqlx::Error>(rows)
            }),
        }
        .map_err(|error| format!("Failed to load file change timeline: {error}"))?;

        Ok(rows.into_iter().map(file_change_entry_from_row).collect())
    }

    pub fn get_workspace_analytics(
        &self,
        app: &AppHandle,
        workspace_id: &str,
    ) -> Result<WorkspaceAnalytics, String> {
        let pool = database_pool(app);
        let (
            total_sessions,
            active_sessions,
            total_tabs,
            active_tabs,
            total_commands,
            total_file_changes,
            unique_files_changed,
            total_activity_entries,
            total_snapshots,
            average_session_cpu_percent,
            average_session_memory_mb,
            latest_activity_at,
            latest_snapshot_at,
        ) = tauri::async_runtime::block_on(async {
            let total_sessions = query_as::<_, (i64,)>(
                "SELECT COUNT(*) FROM sessions WHERE workspace_id = ?1",
            )
            .bind(workspace_id)
            .fetch_one(&pool)
            .await?
            .0;

            let active_sessions = query_as::<_, (i64,)>(
                "SELECT COUNT(*) FROM sessions WHERE workspace_id = ?1 AND status NOT IN ('closed', 'error')",
            )
            .bind(workspace_id)
            .fetch_one(&pool)
            .await?
            .0;

            let total_tabs = query_as::<_, (i64,)>(
                "SELECT COUNT(*) FROM tabs WHERE workspace_id = ?1",
            )
            .bind(workspace_id)
            .fetch_one(&pool)
            .await?
            .0;

            let active_tabs = query_as::<_, (i64,)>(
                "SELECT COUNT(*) FROM tabs WHERE workspace_id = ?1 AND status NOT IN ('closed', 'error')",
            )
            .bind(workspace_id)
            .fetch_one(&pool)
            .await?
            .0;

            let total_commands = query_as::<_, (i64,)>(
                "SELECT COUNT(*) FROM command_history WHERE workspace_id = ?1",
            )
            .bind(workspace_id)
            .fetch_one(&pool)
            .await?
            .0;

            let total_file_changes = query_as::<_, (i64,)>(
                "SELECT COUNT(*) FROM file_changes WHERE workspace_id = ?1",
            )
            .bind(workspace_id)
            .fetch_one(&pool)
            .await?
            .0;

            let unique_files_changed = query_as::<_, (i64,)>(
                "SELECT COUNT(DISTINCT file_path) FROM file_changes WHERE workspace_id = ?1",
            )
            .bind(workspace_id)
            .fetch_one(&pool)
            .await?
            .0;

            let total_activity_entries = query_as::<_, (i64,)>(
                "SELECT COUNT(*) FROM activity_log WHERE workspace_id = ?1",
            )
            .bind(workspace_id)
            .fetch_one(&pool)
            .await?
            .0;

            let total_snapshots = query_as::<_, (i64,)>(
                "SELECT COUNT(*) FROM workspace_snapshots WHERE workspace_id = ?1",
            )
            .bind(workspace_id)
            .fetch_one(&pool)
            .await?
            .0;

            let average_session_cpu_percent = query_as::<_, (Option<f64>,)>(
                "SELECT AVG(cpu_percent) FROM sessions WHERE workspace_id = ?1",
            )
            .bind(workspace_id)
            .fetch_one(&pool)
            .await?
            .0
            .unwrap_or(0.0);

            let average_session_memory_mb = query_as::<_, (Option<f64>,)>(
                "SELECT AVG(memory_mb) FROM sessions WHERE workspace_id = ?1",
            )
            .bind(workspace_id)
            .fetch_one(&pool)
            .await?
            .0
            .unwrap_or(0.0);

            let latest_activity_at = query_as::<_, (Option<i64>,)>(
                "SELECT MAX(timestamp) FROM activity_log WHERE workspace_id = ?1",
            )
            .bind(workspace_id)
            .fetch_one(&pool)
            .await?
            .0;

            let latest_snapshot_at = query_as::<_, (Option<i64>,)>(
                "SELECT MAX(created_at) FROM workspace_snapshots WHERE workspace_id = ?1",
            )
            .bind(workspace_id)
            .fetch_one(&pool)
            .await?
            .0;

            Ok::<_, sqlx::Error>((
                total_sessions,
                active_sessions,
                total_tabs,
                active_tabs,
                total_commands,
                total_file_changes,
                unique_files_changed,
                total_activity_entries,
                total_snapshots,
                average_session_cpu_percent,
                average_session_memory_mb,
                latest_activity_at,
                latest_snapshot_at,
            ))
        })
        .map_err(|error| format!("Failed to load workspace analytics: {error}"))?;

        Ok(WorkspaceAnalytics {
            workspace_id: workspace_id.to_string(),
            total_sessions,
            active_sessions,
            total_tabs,
            active_tabs,
            total_commands,
            total_file_changes,
            unique_files_changed,
            total_activity_entries,
            total_snapshots,
            average_session_cpu_percent: round(average_session_cpu_percent, 1),
            average_session_memory_mb: round(average_session_memory_mb, 1),
            latest_activity_at,
            latest_snapshot_at,
        })
    }

    pub fn export_audit_log(
        &self,
        app: &AppHandle,
        workspace_id: &str,
        start_timestamp: Option<i64>,
        end_timestamp: Option<i64>,
        format: Option<&str>,
    ) -> Result<String, String> {
        let pool = database_pool(app);
        let rows = tauri::async_runtime::block_on(async {
            let rows = match (start_timestamp, end_timestamp) {
                (Some(start), Some(end)) => {
                    AuditRepository::find_by_date_range(&pool, workspace_id, start, end).await?
                }
                _ => {
                    query_as::<_, AuditLogRow>(
                        r#"
                        SELECT * FROM audit_log
                        WHERE workspace_id = ?1
                        ORDER BY timestamp DESC
                        "#,
                    )
                    .bind(workspace_id)
                    .fetch_all(&pool)
                    .await?
                }
            };
            Ok::<_, sqlx::Error>(rows)
        })
        .map_err(|error| format!("Failed to export audit log: {error}"))?;

        let entries = rows.into_iter().map(audit_log_entry_from_row).collect::<Vec<_>>();
        let format = format.unwrap_or("json");

        if format.eq_ignore_ascii_case("csv") {
            let mut lines = vec![
                "id,workspace_id,session_id,tab_id,timestamp,action_type,resource_type,resource_id,details,user_id".to_string(),
            ];
            for entry in &entries {
                lines.push(format!(
                    "{},{},{},{},{},{},{},{},{},{}",
                    entry.id,
                    csv_escape(entry.workspace_id.as_deref().unwrap_or("")),
                    csv_escape(entry.session_id.as_deref().unwrap_or("")),
                    csv_escape(entry.tab_id.as_deref().unwrap_or("")),
                    entry.timestamp,
                    csv_escape(&entry.action_type),
                    csv_escape(&entry.resource_type),
                    csv_escape(&entry.resource_id),
                    csv_escape(entry.details.as_deref().unwrap_or("")),
                    csv_escape(entry.user_id.as_deref().unwrap_or("")),
                ));
            }
            return Ok(lines.join("\n"));
        }

        serde_json::to_string(&entries).map_err(|error| error.to_string())
    }

    pub fn create_workspace_snapshot(
        &self,
        app: &AppHandle,
        workspace_id: &str,
        name: &str,
        description: Option<String>,
    ) -> Result<SnapshotSummary, String> {
        let workspace = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner
                .workspaces
                .get(workspace_id)
                .cloned()
                .ok_or_else(|| "Workspace not found.".to_string())?
        };

        let project_root = workspace
            .project
            .path
            .clone()
            .map(PathBuf::from)
            .ok_or_else(|| "Workspace project path is unavailable.".to_string())?;
        let files = collect_snapshot_files(&project_root)?;
        let snapshot = SnapshotSummary {
            id: format!("snapshot-{}", create_token()),
            workspace_id: workspace.id.clone(),
            name: name.trim().to_string(),
            description: description
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            created_at: now_millis(),
            file_count: files.len() as i64,
            session_count: workspace.session_ids.len() as i64,
        };
        let snapshot_data = serde_json::to_string(&StoredWorkspaceSnapshotData {
            workspace: workspace.clone(),
            files,
        })
        .map_err(|error| error.to_string())?;

        let pool = database_pool(app);
        tauri::async_runtime::block_on(WorkspaceSnapshotRepository::create(
            &pool,
            &snapshot,
            &snapshot_data,
        ))
        .map_err(|error| format!("Failed to create workspace snapshot: {error}"))?;

        self.push_activity_log(
            app,
            "workspace",
            "completed",
            "Create workspace snapshot",
            project_root.to_string_lossy().to_string(),
            Some(snapshot.name.clone()),
        );
        self.persist_audit_event(
            app,
            Some(&workspace.id),
            None,
            None,
            "workspace-snapshot-created",
            "workspace-snapshot",
            &snapshot.id,
            Some(serde_json::json!({
                "name": snapshot.name.clone(),
                "description": snapshot.description.clone(),
                "fileCount": snapshot.file_count,
                "sessionCount": snapshot.session_count,
            })),
        );

        Ok(snapshot)
    }

    pub fn restore_workspace_snapshot(
        &self,
        app: &AppHandle,
        snapshot_id: &str,
    ) -> Result<WorkspaceContext, String> {
        let pool = database_pool(app);
        let snapshot_row = tauri::async_runtime::block_on(
            WorkspaceSnapshotRepository::find_by_id(&pool, snapshot_id),
        )
        .map_err(|error| format!("Failed to load workspace snapshot: {error}"))?
        .ok_or_else(|| "Snapshot not found.".to_string())?;
        let snapshot_summary = snapshot_summary_from_row(snapshot_row.clone());
        let snapshot_data: StoredWorkspaceSnapshotData =
            serde_json::from_str(&snapshot_row.snapshot_data).map_err(|error| error.to_string())?;

        let current_workspace = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.workspaces.get(&snapshot_row.workspace_id).cloned()
        }
        .ok_or_else(|| "Workspace not found.".to_string())?;

        let project_root = current_workspace
            .project
            .path
            .clone()
            .or_else(|| snapshot_data.workspace.project.path.clone())
            .map(PathBuf::from)
            .ok_or_else(|| "Workspace project path is unavailable.".to_string())?;

        restore_snapshot_files(&project_root, &snapshot_data.files)?;
        let next_project = inspect_project(&project_root)?;

        let (workspace, should_refresh_project) = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let active_workspace_id = inner.active_workspace_id.clone();
            let workspace = inner
                .workspaces
                .get_mut(&snapshot_row.workspace_id)
                .ok_or_else(|| "Workspace not found.".to_string())?;
            workspace.project = next_project.clone();
            workspace.last_active_at = now_millis();
            let workspace = workspace.clone();
            if active_workspace_id.as_deref() == Some(snapshot_row.workspace_id.as_str()) {
                inner.project = next_project;
                (workspace, true)
            } else {
                (workspace, false)
            }
        };

        self.persist_workspace(app, &workspace)?;
        self.push_activity_log(
            app,
            "workspace",
            "completed",
            "Restore workspace snapshot",
            project_root.to_string_lossy().to_string(),
            Some(snapshot_summary.name.clone()),
        );
        self.persist_audit_event(
            app,
            Some(&workspace.id),
            None,
            None,
            "workspace-snapshot-restored",
            "workspace-snapshot",
            &snapshot_summary.id,
            Some(serde_json::json!({
                "name": snapshot_summary.name,
                "fileCount": snapshot_summary.file_count,
            })),
        );

        self.emit_workspace_updated(app, &workspace.id);
        if should_refresh_project {
            self.emit_project_state(app);
        }
        self.emit_workspace_state(app);

        Ok(workspace)
    }

    pub fn list_workspace_snapshots(
        &self,
        app: &AppHandle,
        workspace_id: &str,
    ) -> Result<Vec<SnapshotSummary>, String> {
        let pool = database_pool(app);
        let rows = tauri::async_runtime::block_on(WorkspaceSnapshotRepository::find_by_workspace(
            &pool,
            workspace_id,
            Some(100),
        ))
        .map_err(|error| format!("Failed to load workspace snapshots: {error}"))?;

        Ok(rows.into_iter().map(snapshot_summary_from_row).collect())
    }
}
