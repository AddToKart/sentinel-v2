fn write_workspace_file(workspace_path: &Path, relative_path: &str, content: &str) -> Result<(), String> {
    let absolute_path = resolve_workspace_target(workspace_path, relative_path)?;
    if let Some(parent) = absolute_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(absolute_path, content).map_err(|error| error.to_string())
}

fn parse_git_status_output(raw: &str) -> Vec<String> {
    let entries = raw.split('\0').filter(|entry| !entry.is_empty()).collect::<Vec<_>>();
    let mut modified_paths = Vec::new();
    let mut index = 0;
    while index < entries.len() {
        let entry = entries[index];
        if entry.len() < 4 {
            index += 1;
            continue;
        }
        let status = &entry[0..2];
        let primary_path = entry[3..].trim();
        if primary_path.is_empty() {
            index += 1;
            continue;
        }
        if status.contains('R') || status.contains('C') {
            if let Some(next_entry) = entries.get(index + 1) {
                let renamed = next_entry.trim();
                if !renamed.is_empty() {
                    modified_paths.push(renamed.to_string());
                    index += 2;
                    continue;
                }
            }
        }
        modified_paths.push(primary_path.to_string());
        index += 1;
    }
    modified_paths.sort();
    modified_paths.dedup();
    modified_paths
}

fn collect_workspace_diffs_for_record(record: &mut SessionRecord) -> Result<Vec<String>, String> {
    let workspace_path = PathBuf::from(&record.summary.workspace_path);
    if !workspace_path.exists() {
        return Ok(Vec::new());
    }

    if record.summary.workspace_strategy == SessionWorkspaceStrategy::SandboxCopy {
        let sandbox_state = record
            .sandbox_state
            .clone()
            .ok_or_else(|| "Sandbox state is unavailable.".to_string())?;
        let (modified_paths, next_cache) = refresh_sandbox_workspace_diffs(&workspace_path, &sandbox_state)?;
        record.sandbox_state = Some(SandboxWorkspaceState {
            baseline_hashes: sandbox_state.baseline_hashes,
            scan_cache: next_cache,
        });
        return Ok(modified_paths);
    }

    let raw = run_command(
        "git",
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        Some(&workspace_path),
    )?;
    Ok(parse_git_status_output(&raw))
}

fn collect_process_tree_snapshots(root_ids: &[u32]) -> Result<HashMap<u32, ProcessTreeSnapshot>, String> {
    if root_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let root_values = root_ids
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let script = format!(
        "$ErrorActionPreference='SilentlyContinue'; \
         $rootIds=@({root_values}); \
         $children=@{{}}; \
         Get-CimInstance Win32_Process | ForEach-Object {{ \
           $parent=[string]$_.ParentProcessId; \
           if (-not $children.ContainsKey($parent)) {{ $children[$parent]=New-Object System.Collections.Generic.List[int] }}; \
           $children[$parent].Add([int]$_.ProcessId) | Out-Null; \
         }}; \
         $result=@(); \
         foreach ($rootId in $rootIds) {{ \
           $queue=New-Object 'System.Collections.Generic.Queue[int]'; \
           $seen=New-Object 'System.Collections.Generic.HashSet[int]'; \
           $queue.Enqueue([int]$rootId); \
           while ($queue.Count -gt 0) {{ \
             $current=$queue.Dequeue(); \
             if ($seen.Add($current)) {{ \
               $key=[string]$current; \
               if ($children.ContainsKey($key)) {{ \
                 foreach ($child in $children[$key]) {{ $queue.Enqueue([int]$child) }} \
               }} \
             }} \
           }}; \
           $ids=@($seen); \
           $stats=@(); \
           if ($ids.Count -gt 0) {{ $stats=Get-Process -Id $ids -ErrorAction SilentlyContinue }}; \
           $cpu=0.0; $workingSet=0; $handles=0; $threads=0; \
           foreach ($proc in $stats) {{ \
             if ($null -ne $proc.CPU) {{ $cpu += [double]$proc.CPU }}; \
             if ($null -ne $proc.WorkingSet64) {{ $workingSet += [int64]$proc.WorkingSet64 }}; \
             if ($null -ne $proc.HandleCount) {{ $handles += [int]$proc.HandleCount }}; \
             if ($null -ne $proc.Threads) {{ $threads += $proc.Threads.Count }}; \
           }}; \
           $result += [pscustomobject]@{{ \
             RootId=[int]$rootId; \
             CpuTotalSeconds=[double]$cpu; \
             WorkingSetBytes=[int64]$workingSet; \
             HandleCount=[int]$handles; \
             ThreadCount=[int]$threads; \
             ProcessCount=[int]$ids.Count; \
             ProcessIds=@($ids); \
           }}; \
         }}; \
         $result | ConvertTo-Json -Compress"
    );

    let raw = run_powershell(&script)?;
    if raw.trim().is_empty() {
        return Ok(HashMap::new());
    }

    let parsed = serde_json::from_str::<serde_json::Value>(&raw).map_err(|error| error.to_string())?;
    let snapshots = if parsed.is_array() {
        serde_json::from_value::<Vec<RawProcessTreeSnapshot>>(parsed).map_err(|error| error.to_string())?
    } else {
        vec![serde_json::from_value::<RawProcessTreeSnapshot>(parsed).map_err(|error| error.to_string())?]
    };

    Ok(snapshots
        .into_iter()
        .map(|snapshot| {
            (
                snapshot.root_id,
                ProcessTreeSnapshot {
                    cpu_total_seconds: snapshot.cpu_total_seconds,
                    working_set_bytes: snapshot.working_set_bytes,
                    handle_count: snapshot.handle_count,
                    thread_count: snapshot.thread_count,
                    process_count: snapshot.process_count,
                    process_ids: snapshot.process_ids,
                },
            )
        })
        .collect())
}

fn append_history_entry(history: &mut Vec<SessionCommandEntry>, command: &str, source: &str) {
    let normalized = command.trim();
    if normalized.is_empty() {
        return;
    }
    history.insert(
        0,
        SessionCommandEntry {
            id: format!("{}-{}", create_timestamp(), create_token()),
            command: normalized.to_string(),
            timestamp: now_millis(),
            source: source.to_string(),
        },
    );
    if history.len() > 250 {
        history.truncate(250);
    }
}

fn track_command_input(command_buffer: &mut String, history: &mut Vec<SessionCommandEntry>, data: &str) {
    for character in data.chars() {
        match character {
            '\r' | '\n' => {
                append_history_entry(history, command_buffer, "interactive");
                command_buffer.clear();
            }
            '\u{0003}' | '\u{0015}' => {
                command_buffer.clear();
            }
            '\u{0008}' | '\u{007f}' => {
                command_buffer.pop();
            }
            '\t' => command_buffer.push(character),
            value if value >= ' ' => command_buffer.push(value),
            _ => {}
        }
    }
}

fn resize_terminal(master: &SharedMaster, cols: u16, rows: u16) -> Result<(), String> {
    let master = master.lock().expect("pty poisoned");
    master
        .resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|error| error.to_string())
}

fn write_terminal(writer: &SharedWriter, data: &[u8]) -> Result<(), String> {
    let mut writer = writer.lock().expect("writer poisoned");
    writer.write_all(data).map_err(|error| error.to_string())?;
    writer.flush().map_err(|error| error.to_string())
}

fn kill_with_killer(killer: &SharedKiller) -> Result<(), String> {
    let mut killer = killer.lock().expect("killer poisoned");
    killer.kill().map_err(|error| error.to_string())
}

fn terminate_process_id(pid: Option<u32>) -> Result<(), String> {
    let Some(pid) = pid else {
        return Ok(());
    };
    #[cfg(windows)]
    {
        let _ = run_command("taskkill", &["/PID", &pid.to_string(), "/T", "/F"], None);
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let status = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status()
            .map_err(|error| error.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err("Failed to terminate process.".to_string())
        }
    }
}

