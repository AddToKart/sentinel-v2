fn hash_file(file_path: &Path) -> Result<String, String> {
    let content = fs::read(file_path).map_err(|error| error.to_string())?;
    let mut hasher = Sha1::new();
    hasher.update(content);
    Ok(format!("{:x}", hasher.finalize()))
}

fn create_signature(metadata: &fs::Metadata) -> String {
    let modified = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|value| value.as_millis())
        .unwrap_or_default();
    format!("{}:{}", metadata.len(), modified)
}

fn copy_project_tree(
    project_root: &Path,
    workspace_path: &Path,
    relative_root: Option<&Path>,
) -> Result<(), String> {
    let source_root = relative_root
        .map(|value| project_root.join(value))
        .unwrap_or_else(|| project_root.to_path_buf());
    let target_root = relative_root
        .map(|value| workspace_path.join(value))
        .unwrap_or_else(|| workspace_path.to_path_buf());
    fs::create_dir_all(&target_root).map_err(|error| error.to_string())?;

    for entry in fs::read_dir(&source_root).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let entry_path = entry.path();
        let entry_name = entry.file_name();
        let name = entry_name.to_string_lossy().to_string();
        let relative_path = relative_root
            .map(|root| root.join(&entry_name))
            .unwrap_or_else(|| PathBuf::from(&entry_name));
        let target_path = workspace_path.join(&relative_path);
        let file_type = entry.file_type().map_err(|error| error.to_string())?;

        if file_type.is_dir() {
            if should_skip_directory(&name) {
                continue;
            }
            copy_project_tree(project_root, workspace_path, Some(&relative_path))?;
            continue;
        }

        if file_type.is_file() {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            fs::copy(&entry_path, &target_path).map_err(|error| error.to_string())?;
        }
    }

    Ok(())
}

fn list_tracked_files(root_path: &Path) -> Result<Vec<String>, String> {
    if !root_path.exists() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    list_tracked_files_recursive(root_path, root_path, &mut files)?;
    files.sort();
    Ok(files)
}

fn list_tracked_files_recursive(
    root_path: &Path,
    current_path: &Path,
    files: &mut Vec<String>,
) -> Result<(), String> {
    for entry in fs::read_dir(current_path).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let file_type = entry.file_type().map_err(|error| error.to_string())?;

        if file_type.is_dir() {
            if should_skip_directory(&name) || should_link_directory(&name) {
                continue;
            }
            list_tracked_files_recursive(root_path, &path, files)?;
            continue;
        }

        if file_type.is_file() {
            let relative_path = path
                .strip_prefix(root_path)
                .map_err(|error| error.to_string())?;
            files.push(path_to_string(relative_path));
        }
    }
    Ok(())
}

fn snapshot_project_hashes(root_path: &Path) -> Result<BTreeMap<String, String>, String> {
    let mut hashes = BTreeMap::new();
    for relative_path in list_tracked_files(root_path)? {
        hashes.insert(
            relative_path.clone(),
            hash_file(&resolve_workspace_target(root_path, &relative_path)?)?,
        );
    }
    Ok(hashes)
}

fn snapshot_workspace_files(
    workspace_path: &Path,
    previous_cache: Option<&BTreeMap<String, FileFingerprint>>,
) -> Result<BTreeMap<String, FileFingerprint>, String> {
    let mut snapshots = BTreeMap::new();
    for relative_path in list_tracked_files(workspace_path)? {
        let absolute_path = resolve_workspace_target(workspace_path, &relative_path)?;
        let metadata = fs::metadata(&absolute_path).map_err(|error| error.to_string())?;
        let signature = create_signature(&metadata);
        let previous = previous_cache.and_then(|cache| cache.get(&relative_path));
        let hash = match previous {
            Some(previous) if previous.signature == signature => previous.hash.clone(),
            _ => hash_file(&absolute_path)?,
        };
        snapshots.insert(relative_path, FileFingerprint { signature, hash });
    }
    Ok(snapshots)
}

fn initialize_sandbox_repository(workspace_path: &Path) -> Result<(), String> {
    let _ = run_command(
        "git",
        &["init", "-b", "sentinel-sandbox"],
        Some(workspace_path),
    )
    .or_else(|_| run_command("git", &["init"], Some(workspace_path)))
    .and_then(|_| {
        run_command(
            "git",
            &["checkout", "-B", "sentinel-sandbox"],
            Some(workspace_path),
        )
    });

    let exclude_path = workspace_path.join(".git").join("info").join("exclude");
    if let Some(parent) = exclude_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let content = [
        ".git",
        ".next",
        ".turbo",
        ".venv",
        "node_modules",
        "dist",
        "out",
        "build",
        "coverage",
        "__pycache__",
        "venv",
        ".tox",
        ".yarn",
        ".pnpm-store",
    ]
    .iter()
    .map(|entry| format!("{entry}/"))
    .collect::<Vec<_>>()
    .join("\n");
    fs::write(&exclude_path, format!("{content}\n")).map_err(|error| error.to_string())?;

    let _ = run_command(
        "git",
        &["config", "user.name", "Sentinel"],
        Some(workspace_path),
    );
    let _ = run_command(
        "git",
        &["config", "user.email", "sentinel@local.invalid"],
        Some(workspace_path),
    );
    let _ = run_command("git", &["add", "-A"], Some(workspace_path));
    let _ = run_command(
        "git",
        &["commit", "-m", "Sentinel sandbox baseline"],
        Some(workspace_path),
    )
    .or_else(|_| {
        run_command(
            "git",
            &["commit", "--allow-empty", "-m", "Sentinel sandbox baseline"],
            Some(workspace_path),
        )
    });
    Ok(())
}

fn ensure_shared_directories(project_root: &Path, workspace_path: &Path) -> Result<(), String> {
    for directory_name in [
        "node_modules",
        ".venv",
        "venv",
        ".tox",
        ".yarn",
        ".pnpm-store",
    ] {
        let source_path = project_root.join(directory_name);
        let destination_path = workspace_path.join(directory_name);
        if !source_path.exists() {
            continue;
        }
        let _ = fs::remove_dir_all(&destination_path);
        create_directory_link(&source_path, &destination_path)?;
    }
    Ok(())
}

fn create_directory_link(source: &Path, destination: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        let status = Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                &path_to_string(destination),
                &path_to_string(source),
            ])
            .status()
            .map_err(|error| error.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err("Failed to create directory junction.".to_string())
        }
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(source, destination).map_err(|error| error.to_string())
    }
}

fn collect_modified_paths(
    baseline_hashes: &BTreeMap<String, String>,
    workspace_snapshot: &BTreeMap<String, FileFingerprint>,
) -> Vec<String> {
    let all_paths = baseline_hashes
        .keys()
        .chain(workspace_snapshot.keys())
        .cloned()
        .collect::<HashSet<_>>();

    let mut modified = all_paths
        .into_iter()
        .filter(|relative_path| {
            baseline_hashes.get(relative_path)
                != workspace_snapshot
                    .get(relative_path)
                    .map(|fingerprint| &fingerprint.hash)
        })
        .collect::<Vec<_>>();
    modified.sort();
    modified
}

fn create_sandbox_workspace(
    project_root: &Path,
    workspace_path: &Path,
) -> Result<SandboxWorkspaceState, String> {
    // PERFORMANCE OPTIMIZATION: Skip expensive hash operations during creation
    // They will be computed lazily when first needed for diff comparison

    // Clean up any existing workspace
    let _ = fs::remove_dir_all(workspace_path);
    fs::create_dir_all(workspace_path).map_err(|error| error.to_string())?;

    // Copy files (this is the unavoidable part, but we can optimize it)
    copy_project_tree(project_root, workspace_path, None)?;

    // Initialize git repo (needed for change tracking, but can be optimized)
    let _ = initialize_sandbox_repository(workspace_path);

    // Link shared directories (node_modules, etc.) instead of copying
    let _ = ensure_shared_directories(project_root, workspace_path);

    // PERFORMANCE OPTIMIZATION: Create empty hashes/cache initially
    // They will be populated on first diff check
    Ok(SandboxWorkspaceState {
        baseline_hashes: BTreeMap::new(),
        scan_cache: BTreeMap::new(),
        project_root: Some(path_to_string(project_root)),
    })
}

fn refresh_sandbox_workspace_diffs(
    workspace_path: &Path,
    sandbox_state: &mut SandboxWorkspaceState,
) -> Result<(Vec<String>, BTreeMap<String, FileFingerprint>), String> {
    // PERFORMANCE: Lazily populate baseline hashes if empty (first time only)
    if sandbox_state.baseline_hashes.is_empty() {
        if let Some(ref project_root_str) = sandbox_state.project_root {
            let project_root = Path::new(project_root_str);
            sandbox_state.baseline_hashes = snapshot_project_hashes(project_root)?;
        }
    }

    let next_cache = snapshot_workspace_files(workspace_path, Some(&sandbox_state.scan_cache))?;
    Ok((
        collect_modified_paths(&sandbox_state.baseline_hashes, &next_cache),
        next_cache,
    ))
}

fn apply_sandbox_workspace(
    session_id: &str,
    project_root: &Path,
    workspace_path: &Path,
    sandbox_state: SandboxWorkspaceState,
) -> Result<ApplySandboxOutcome, String> {
    let workspace_snapshot =
        snapshot_workspace_files(workspace_path, Some(&sandbox_state.scan_cache))?;
    let modified_paths =
        collect_modified_paths(&sandbox_state.baseline_hashes, &workspace_snapshot);
    let mut conflicts = Vec::new();
    let mut applied_paths = Vec::new();
    let mut next_baseline_hashes = sandbox_state.baseline_hashes.clone();

    for relative_path in modified_paths {
        let project_file_path = resolve_workspace_target(project_root, &relative_path)?;
        let workspace_file_path = resolve_workspace_target(workspace_path, &relative_path)?;
        let baseline_hash = sandbox_state.baseline_hashes.get(&relative_path).cloned();
        let workspace_hash = workspace_snapshot
            .get(&relative_path)
            .map(|fingerprint| fingerprint.hash.clone());
        let current_project_hash = if project_file_path.exists() {
            Some(hash_file(&project_file_path)?)
        } else {
            None
        };

        if current_project_hash != baseline_hash {
            conflicts.push(SessionSyncConflict {
                path: relative_path,
                reason: "project-changed".to_string(),
                detail: Some(
                    "The file changed in the main project after this sandbox session started."
                        .to_string(),
                ),
            });
            continue;
        }

        if let Some(workspace_hash) = workspace_hash {
            if let Some(parent) = project_file_path.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            fs::copy(&workspace_file_path, &project_file_path)
                .map_err(|error| error.to_string())?;
            next_baseline_hashes.insert(relative_path.clone(), workspace_hash);
        } else {
            let _ = fs::remove_file(&project_file_path);
            next_baseline_hashes.remove(&relative_path);
        }
        applied_paths.push(relative_path);
    }

    let refreshed_cache = snapshot_workspace_files(workspace_path, Some(&workspace_snapshot))?;
    Ok(ApplySandboxOutcome {
        result: SessionApplyResult {
            session_id: session_id.to_string(),
            workspace_strategy: SessionWorkspaceStrategy::SandboxCopy,
            applied_paths,
            remaining_paths: Vec::new(),
            conflicts,
        },
        next_baseline_hashes,
        next_cache: refreshed_cache,
    })
}

fn apply_ide_workspace_impl(
    project_root: &Path,
    workspace_path: &Path,
    sandbox_state: SandboxWorkspaceState,
) -> Result<IdeApplyOutcome, String> {
    let applied =
        apply_sandbox_workspace("ide-workspace", project_root, workspace_path, sandbox_state)?;
    let refreshed = refresh_sandbox_workspace_diffs(
        workspace_path,
        &mut SandboxWorkspaceState {
            baseline_hashes: applied.next_baseline_hashes.clone(),
            scan_cache: applied.next_cache.clone(),
            project_root: Some(path_to_string(project_root)),
        },
    )?;
    Ok(IdeApplyOutcome {
        result: applied.result,
        sandbox_state: SandboxWorkspaceState {
            baseline_hashes: applied.next_baseline_hashes,
            scan_cache: refreshed.1.clone(),
            project_root: Some(path_to_string(project_root)),
        },
        modified_paths: refreshed.0,
    })
}

fn discard_sandbox_workspace(
    project_root: &Path,
    workspace_path: &Path,
) -> Result<SandboxWorkspaceState, String> {
    create_sandbox_workspace(project_root, workspace_path)
}

fn discard_ide_workspace_impl(
    project_root: &Path,
    workspace_path: &Path,
) -> Result<IdeDiscardOutcome, String> {
    Ok(IdeDiscardOutcome {
        sandbox_state: discard_sandbox_workspace(project_root, workspace_path)?,
        modified_paths: Vec::new(),
    })
}
