const EVENT_CHANGES_UPDATED: &str = "sentinel:changes-updated";
const EVENT_UNIFIED_SANDBOX_UPDATED: &str = "sentinel:unified-sandbox-updated";

fn compute_unified_diff(before: &str, after: &str) -> (String, i64, i64) {
    let mut additions = 0i64;
    let mut deletions = 0i64;
    let mut diff_lines = Vec::new();

    let before_lines: Vec<&str> = before.lines().collect();
    let after_lines: Vec<&str> = after.lines().collect();

    let max_len = before_lines.len().max(after_lines.len());

    for i in 0..max_len {
        let before_line = before_lines.get(i).copied().unwrap_or("");
        let after_line = after_lines.get(i).copied().unwrap_or("");

        if before_line == after_line {
            if !diff_lines.is_empty() {
                diff_lines.push(format!(" {}", after_line));
            }
        } else {
            if !before_line.is_empty() {
                diff_lines.push(format!("-{}", before_line));
                deletions += 1;
            }
            if !after_line.is_empty() {
                diff_lines.push(format!("+{}", after_line));
                additions += 1;
            }
        }
    }

    if diff_lines.is_empty() {
        (String::new(), additions, deletions)
    } else {
        (diff_lines.join("\n"), additions, deletions)
    }
}

fn generate_change_id() -> String {
    format!("change-{:x}", now_millis())
}

fn generate_unified_id() -> String {
    format!("unified-{:x}", now_millis())
}

fn is_binary_file(file_path: &Path) -> bool {
    if let Ok(mut file) = fs::File::open(file_path) {
        use std::io::Read;
        let mut buffer = [0u8; 8192];
        if let Ok(bytes_read) = file.read(&mut buffer) {
            return buffer[..bytes_read].iter().any(|&b| b == 0);
        }
    }
    false
}

pub struct ChangesManager {
    sandbox_states: std::sync::Mutex<BTreeMap<String, SandboxTracker>>,
}

struct SandboxTracker {
    #[allow(dead_code)]
    workspace_id: String,
    #[allow(dead_code)]
    agent_id: String,
    sandbox_path: PathBuf,
    file_hashes: BTreeMap<String, String>,
}

impl ChangesManager {
    pub fn new() -> Self {
        Self {
            sandbox_states: std::sync::Mutex::new(BTreeMap::new()),
        }
    }

    #[allow(dead_code)]
    pub fn register_sandbox(
        &self,
        workspace_id: &str,
        agent_id: &str,
        sandbox_path: &Path,
    ) {
        let mut states = self.sandbox_states.lock().unwrap_or_else(|e| e.into_inner());
        let key = format!("{}:{}", workspace_id, agent_id);

        let mut file_hashes = BTreeMap::new();
        if sandbox_path.exists() {
            let _ = Self::scan_sandbox_files(sandbox_path, sandbox_path, &mut file_hashes);
        }

        states.insert(key, SandboxTracker {
            workspace_id: workspace_id.to_string(),
            agent_id: agent_id.to_string(),
            sandbox_path: sandbox_path.to_path_buf(),
            file_hashes,
        });
    }

    fn scan_sandbox_files(
        root: &Path,
        current: &Path,
        hashes: &mut BTreeMap<String, String>,
    ) -> Result<(), String> {
        if !current.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(current).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            if path.is_dir() {
                if name_str == ".git" || name_str == "node_modules" || name_str == ".sentinel" {
                    continue;
                }
                let _ = Self::scan_sandbox_files(root, &path, hashes);
            } else if path.is_file() {
                if let Ok(hash) = hash_file(&path) {
                    let rel = path.strip_prefix(root)
                        .map(|p| path_to_string(p))
                        .unwrap_or_else(|_| path_to_string(&path));
                    hashes.insert(rel, hash);
                }
            }
        }
        Ok(())
    }

    pub fn scan_sandbox_changes(
        &self,
        app: &AppHandle,
        workspace_id: &str,
        agent_id: &str,
    ) -> Result<Vec<AgentFileChange>, String> {
        let key = format!("{}:{}", workspace_id, agent_id);
        let mut states = self.sandbox_states.lock().unwrap_or_else(|e| e.into_inner());
        let tracker = states.get_mut(&key)
            .ok_or_else(|| format!("No sandbox registered for agent {}", agent_id))?;

        let sandbox_path = tracker.sandbox_path.clone();
        let old_hashes = tracker.file_hashes.clone();
        let mut new_hashes = BTreeMap::new();

        let _ = Self::scan_sandbox_files(&sandbox_path, &sandbox_path, &mut new_hashes);

        let mut changes = Vec::new();
        let all_paths: HashSet<_> = old_hashes.keys()
            .chain(new_hashes.keys())
            .cloned()
            .collect();

        for file_path in &all_paths {
            let old_hash = old_hashes.get(file_path);
            let new_hash = new_hashes.get(file_path);

            let (operation, diff_content, additions, deletions, is_binary, file_size) = match (old_hash, new_hash) {
                (None, Some(_new_h)) => {
                    let abs_path = sandbox_path.join(file_path);
                    let is_bin = is_binary_file(&abs_path);
                    let content = if is_bin {
                        String::new()
                    } else {
                        fs::read_to_string(&abs_path).unwrap_or_default()
                    };
                    let fsize = fs::metadata(&abs_path).ok().map(|m| m.len() as i64);
                    let line_count = content.lines().count() as i64;
                    ("created", content, line_count, 0i64, is_bin, fsize)
                }
                (Some(_old_h), None) => ("deleted", String::new(), 0i64, 0i64, false, None),
                (Some(old_h), Some(new_h)) if old_h != new_h => {
                    let abs_path = sandbox_path.join(file_path);
                    let is_bin = is_binary_file(&abs_path);
                    let new_content = if is_bin {
                        String::new()
                    } else {
                        fs::read_to_string(&abs_path).unwrap_or_default()
                    };
                    let fsize = fs::metadata(&abs_path).ok().map(|m| m.len() as i64);
                    if is_bin {
                        ("modified", String::new(), 0i64, 0i64, true, fsize)
                    } else {
                        let (diff, add, del) = compute_unified_diff("", &new_content);
                        ("modified", diff, add, del, false, fsize)
                    }
                }
                _ => continue,
            };

            let change_id = generate_change_id();
            let change = AgentFileChange {
                id: change_id.clone(),
                workspace_id: workspace_id.to_string(),
                agent_id: agent_id.to_string(),
                sandbox_id: key.clone(),
                file_path: file_path.clone(),
                operation: operation.to_string(),
                diff_content: if diff_content.is_empty() { None } else { Some(diff_content) },
                additions,
                deletions,
                timestamp: now_millis(),
                unified_status: "pending".to_string(),
                file_size,
                is_binary,
            };

            changes.push(change.clone());

            let pool = database_pool(app);
            let _ = tauri::async_runtime::block_on(async {
                AgentFileChangeRepository::upsert(
                    &pool,
                    &change_id,
                    workspace_id,
                    agent_id,
                    &key,
                    file_path,
                    operation,
                    change.diff_content.as_deref(),
                    additions,
                    deletions,
                    change.timestamp,
                    "pending",
                    change.file_size,
                    if is_binary { 1 } else { 0 },
                ).await
            });

            tracker.file_hashes = new_hashes.clone();
        }

        if !changes.is_empty() {
            let _ = self.update_unified_sandbox(app, workspace_id);
            emit_event(app, EVENT_CHANGES_UPDATED, &serde_json::json!({
                "workspaceId": workspace_id,
                "agentId": agent_id,
                "changeCount": changes.len()
            }));
        }

        Ok(changes)
    }

    fn update_unified_sandbox(
        &self,
        app: &AppHandle,
        workspace_id: &str,
    ) -> Result<(), String> {
        let pool = database_pool(app);
        let changes = tauri::async_runtime::block_on(
            AgentFileChangeRepository::find_pending_by_workspace(&pool, workspace_id)
        ).map_err(|e| e.to_string())?;

        let mut file_sources: BTreeMap<String, Vec<&AgentFileChangeRow>> = BTreeMap::new();
        for change in &changes {
            file_sources.entry(change.file_path.clone())
                .or_default()
                .push(change);
        }

        for (file_path, agent_changes) in &file_sources {
            let (source_agent, conflict_agents, status) = if agent_changes.len() == 1 {
                (agent_changes[0].agent_id.clone(), Vec::new(), "clean".to_string())
            } else {
                let source = &agent_changes[0];
                let conflicts: Vec<_> = agent_changes.iter()
                    .skip(1)
                    .map(|c| c.agent_id.clone())
                    .collect();
                (source.agent_id.clone(), conflicts, "conflicted".to_string())
            };

            let entry_id = generate_unified_id();
            let conflict_json = if conflict_agents.is_empty() {
                None
            } else {
                Some(serde_json::to_string(&conflict_agents).unwrap_or_default())
            };

            let _ = tauri::async_runtime::block_on(async {
                UnifiedSandboxRepository::upsert(
                    &pool,
                    &entry_id,
                    workspace_id,
                    file_path,
                    &source_agent,
                    conflict_json.as_deref(),
                    &status,
                    now_millis(),
                ).await
            });
        }

        if !file_sources.is_empty() {
            emit_event(app, EVENT_UNIFIED_SANDBOX_UPDATED, &serde_json::json!({
                "workspaceId": workspace_id,
                "entryCount": file_sources.len()
            }));
        }

        Ok(())
    }

    pub fn get_changes_state(
        &self,
        app: &AppHandle,
        workspace_id: &str,
    ) -> Result<ChangesManagerState, String> {
        let pool = database_pool(app);

        let (change_rows, unified_rows) = tauri::async_runtime::block_on(async {
            tokio::try_join!(
                AgentFileChangeRepository::find_by_workspace(&pool, workspace_id),
                UnifiedSandboxRepository::find_by_workspace(&pool, workspace_id),
            )
        }).map_err(|e| e.to_string())?;

        let agent_changes: Vec<AgentFileChange> = change_rows.into_iter().map(|row| {
            AgentFileChange {
                id: row.id,
                workspace_id: row.workspace_id,
                agent_id: row.agent_id,
                sandbox_id: row.sandbox_id,
                file_path: row.file_path,
                operation: row.operation,
                diff_content: row.diff_content,
                additions: row.additions,
                deletions: row.deletions,
                timestamp: row.timestamp,
                unified_status: row.unified_status,
                file_size: row.file_size,
                is_binary: row.is_binary != 0,
            }
        }).collect();

        let unified_entries: Vec<UnifiedSandboxEntry> = unified_rows.into_iter().map(|row| {
            let conflict_agent_ids = row.conflict_agent_ids.and_then(|json| {
                serde_json::from_str::<Vec<String>>(&json).ok()
            });
            UnifiedSandboxEntry {
                id: row.id,
                workspace_id: row.workspace_id,
                file_path: row.file_path,
                source_agent_id: row.source_agent_id,
                conflict_agent_ids,
                status: row.status,
                last_updated_at: row.last_updated_at,
            }
        }).collect();

        let total_changed_files = unified_entries.len();
        let conflict_count = unified_entries.iter().filter(|e| e.status == "conflicted").count();
        let pending_push_count = agent_changes.iter().filter(|c| c.unified_status == "pending").count();

        Ok(ChangesManagerState {
            agent_changes,
            unified_entries,
            total_changed_files,
            conflict_count,
            pending_push_count,
        })
    }

    pub fn push_unified_sandbox(
        &self,
        app: &AppHandle,
        workspace_id: &str,
        project_root: &Path,
    ) -> Result<Vec<String>, String> {
        let pool = database_pool(app);
        let unified_rows = tauri::async_runtime::block_on(
            UnifiedSandboxRepository::find_by_workspace(&pool, workspace_id)
        ).map_err(|e| e.to_string())?;

        let mut pushed_paths = Vec::new();

        for entry in &unified_rows {
            if entry.status == "conflicted" {
                continue;
            }

            let change_rows = tauri::async_runtime::block_on(
                AgentFileChangeRepository::find_by_agent(&pool, workspace_id, &entry.source_agent_id)
            ).map_err(|e| e.to_string())?;

            if let Some(change) = change_rows.iter().find(|c| c.file_path == entry.file_path && c.unified_status == "pending") {
                let target_path = project_root.join(&change.file_path);

                if change.operation == "deleted" {
                    let _ = fs::remove_file(&target_path);
                } else {
                    if let Some(parent) = target_path.parent() {
                        let _ = fs::create_dir_all(parent);
                    }

                    let sandbox_path = {
                        let states = self.sandbox_states.lock().unwrap_or_else(|e| e.into_inner());
                        let key = format!("{}:{}", workspace_id, change.agent_id);
                        states.get(&key).map(|s| s.sandbox_path.join(&change.file_path))
                    };

                    if let Some(sandbox_path) = sandbox_path {
                        if sandbox_path.exists() {
                            let _ = fs::copy(&sandbox_path, &target_path);
                        }
                    }
                }

                let _ = tauri::async_runtime::block_on(async {
                    AgentFileChangeRepository::update_unified_status(&pool, &change.id, "pushed").await
                });

                pushed_paths.push(change.file_path.clone());
            }
        }

        if !pushed_paths.is_empty() {
            emit_event(app, EVENT_CHANGES_UPDATED, &serde_json::json!({
                "workspaceId": workspace_id,
                "action": "pushed",
                "paths": pushed_paths
            }));
        }

        Ok(pushed_paths)
    }

    pub fn discard_changes(
        &self,
        app: &AppHandle,
        workspace_id: &str,
        agent_id: Option<&str>,
    ) -> Result<(), String> {
        let pool = database_pool(app);

        tauri::async_runtime::block_on(async {
            if let Some(aid) = agent_id {
                AgentFileChangeRepository::delete_by_agent(&pool, workspace_id, aid).await?;
            } else {
                AgentFileChangeRepository::delete_by_workspace(&pool, workspace_id).await?;
            }
            UnifiedSandboxRepository::delete_by_workspace(&pool, workspace_id).await?;
            Ok::<_, sqlx::Error>(())
        }).map_err(|e| e.to_string())?;

        emit_event(app, EVENT_CHANGES_UPDATED, &serde_json::json!({
            "workspaceId": workspace_id,
            "action": "discarded",
            "agentId": agent_id
        }));

        Ok(())
    }

    pub fn resolve_conflict(
        &self,
        app: &AppHandle,
        workspace_id: &str,
        file_path: &str,
        winning_agent_id: &str,
    ) -> Result<(), String> {
        let pool = database_pool(app);

        let change_rows = tauri::async_runtime::block_on(
            AgentFileChangeRepository::find_by_workspace(&pool, workspace_id)
        ).map_err(|e| e.to_string())?;

        let file_changes: Vec<_> = change_rows.iter()
            .filter(|c| c.file_path == file_path && c.unified_status == "pending")
            .collect();

        for change in &file_changes {
            let new_status = if change.agent_id == winning_agent_id {
                "merged"
            } else {
                "discarded"
            };
            let _ = tauri::async_runtime::block_on(async {
                AgentFileChangeRepository::update_unified_status(&pool, &change.id, new_status).await
            });
        }

        let _ = tauri::async_runtime::block_on(async {
            UnifiedSandboxRepository::update_status(&pool, workspace_id, file_path, "clean").await
        });

        emit_event(app, EVENT_CHANGES_UPDATED, &serde_json::json!({
            "workspaceId": workspace_id,
            "action": "conflict_resolved",
            "filePath": file_path,
            "winningAgent": winning_agent_id
        }));

        Ok(())
    }
}

impl SentinelManager {
    pub fn push_unified_sandbox(
        &self,
        app: &AppHandle,
        workspace_id: &str,
    ) -> Result<Vec<String>, String> {
        let project_root = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.project.path.clone().map(PathBuf::from)
        };
        let project_root = project_root.ok_or("No project directory available")?;
        self.changes_manager.push_unified_sandbox(app, workspace_id, &project_root)
    }
}
