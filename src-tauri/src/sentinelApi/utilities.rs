fn emit_event<T: serde::Serialize>(app: &AppHandle, event: &str, payload: &T) {
    let _ = app.emit(event, payload);
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn create_token() -> String {
    format!(
        "{:06x}",
        TOKEN_COUNTER.fetch_add(1, Ordering::Relaxed) & 0x00ff_ffff
    )
}

fn generate_id() -> String {
    format!(
        "tab-{:06x}",
        TOKEN_COUNTER.fetch_add(1, Ordering::Relaxed) & 0x00ff_ffff
    )
}

fn create_timestamp() -> String {
    now_millis().to_string()
}

fn round(value: f64, decimals: i32) -> f64 {
    let factor = 10_f64.powi(decimals);
    (value * factor).round() / factor
}

fn path_to_string(path: &Path) -> String {
    let value = path.to_string_lossy().to_string();
    value.strip_prefix(r"\\?\").unwrap_or(&value).to_string()
}

fn sanitize_segment(input: &str) -> String {
    let mut output = String::new();
    let mut previous_dash = false;
    for character in input.trim().to_lowercase().chars() {
        let keep = character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-');
        if keep {
            output.push(character);
            previous_dash = false;
        } else if !previous_dash {
            output.push('-');
            previous_dash = true;
        }
    }
    let trimmed = output.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "agent".to_string()
    } else {
        trimmed
    }
}

fn normalize_relative_path(relative_path: &str) -> Result<String, String> {
    let replaced = relative_path
        .trim()
        .replace('/', std::path::MAIN_SEPARATOR_STR);
    let path = Path::new(&replaced);
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Normal(segment) => normalized.push(segment),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(format!(
                        "Refusing to access a path outside the workspace: {relative_path}"
                    ));
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(format!(
                    "Refusing to access a path outside the workspace: {relative_path}"
                ));
            }
        }
    }

    Ok(path_to_string(&normalized))
}

fn resolve_workspace_target(root: &Path, relative_path: &str) -> Result<PathBuf, String> {
    let normalized_relative = normalize_relative_path(relative_path)?;
    let resolved = root.join(&normalized_relative);
    if !resolved.starts_with(root) {
        return Err(format!(
            "Refusing to access a path outside the workspace: {relative_path}"
        ));
    }
    Ok(resolved)
}

fn should_skip_directory(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | ".next"
            | ".turbo"
            | ".venv"
            | "venv"
            | ".tox"
            | ".yarn"
            | ".pnpm-store"
            | "node_modules"
            | "dist"
            | "out"
            | "build"
            | "coverage"
            | "__pycache__"
    )
}

fn should_link_directory(name: &str) -> bool {
    matches!(
        name,
        "node_modules" | ".venv" | "venv" | ".tox" | ".yarn" | ".pnpm-store"
    )
}

fn should_traverse_directory(path: &Path, file_type: &fs::FileType) -> bool {
    if !file_type.is_dir() || file_type.is_symlink() {
        return false;
    }

    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;

        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;

        if let Ok(metadata) = fs::symlink_metadata(path) {
            if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                return false;
            }
        }
    }

    true
}

fn inspect_project(candidate_path: &Path) -> Result<ProjectState, String> {
    let requested_path = candidate_path
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let mut project_root = requested_path.clone();
    let mut branch = None;
    let mut is_git_repo = false;

    if let Ok(root) = run_git_command(None, &requested_path, ["rev-parse", "--show-toplevel"]) {
        project_root = PathBuf::from(root);
        branch = run_git_command(None, &project_root, ["branch", "--show-current"]).ok();
        is_git_repo = true;
    }

    Ok(ProjectState {
        path: Some(path_to_string(&project_root)),
        name: project_root
            .file_name()
            .and_then(OsStr::to_str)
            .map(str::to_string),
        branch: branch.filter(|value| !value.is_empty()),
        is_git_repo,
        tree: build_project_tree(&project_root, TREE_DEPTH)?,
    })
}

fn detect_repo_url(candidate_path: &Path) -> Option<String> {
    let project_root = run_git_command(None, candidate_path, ["rev-parse", "--show-toplevel"]).ok()?;
    let repo_url = run_git_command(
        None,
        Path::new(&project_root),
        ["config", "--get", "remote.origin.url"],
    )
    .ok()?;
    let repo_url = repo_url.trim().to_string();
    if repo_url.is_empty() {
        None
    } else {
        Some(repo_url)
    }
}

fn build_project_tree(root_path: &Path, depth: usize) -> Result<Vec<ProjectNode>, String> {
    let mut entries = fs::read_dir(root_path)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();

    entries.retain(|entry| {
        let name = entry.file_name().to_string_lossy().to_string();
        !should_skip_directory(&name)
    });

    entries.sort_by(|left, right| {
        let left_is_dir = left
            .file_type()
            .map(|value| value.is_dir())
            .unwrap_or(false);
        let right_is_dir = right
            .file_type()
            .map(|value| value.is_dir())
            .unwrap_or(false);
        match (left_is_dir, right_is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => left.file_name().cmp(&right.file_name()),
        }
    });
    entries.truncate(TREE_ENTRY_LIMIT);

    let mut nodes = Vec::with_capacity(entries.len());
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        let is_dir = file_type.is_dir();
        let children = if depth > 0 && should_traverse_directory(&path, &file_type) {
            build_project_tree(&path, depth - 1).ok()
        } else {
            None
        };

        nodes.push(ProjectNode {
            name,
            path: path_to_string(&path),
            kind: if is_dir { "directory" } else { "file" }.to_string(),
            children,
        });
    }

    Ok(nodes)
}

fn run_command(file: &str, args: &[&str], cwd: Option<&Path>) -> Result<String, String> {
    let mut command = Command::new(file);
    command.args(args);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    command.stdin(std::process::Stdio::null());
    let output = command.output().map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Err(if !stderr.is_empty() { stderr } else { stdout })
    }
}

fn run_powershell(script: &str) -> Result<String, String> {
    run_command(
        "powershell.exe",
        &[
            "-NoLogo",
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ],
        None,
    )
}

fn run_git_command<I, S>(app: Option<&AppHandle>, cwd: &Path, args: I) -> Result<String, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let args_vec = args
        .into_iter()
        .map(|value| value.as_ref().to_string())
        .collect::<Vec<_>>();
    if let Some(app) = app {
        emit_event(
            app,
            EVENT_ACTIVITY_LOG,
            &ActivityLogEntry {
                id: format!("{}-{}", create_timestamp(), create_token()),
                workspace_id: None,
                timestamp: now_millis(),
                scope: "git".to_string(),
                status: "started".to_string(),
                command: format!("git -C {} {}", path_to_string(cwd), args_vec.join(" ")),
                cwd: path_to_string(cwd),
                detail: None,
            },
        );
    }
    let borrowed = args_vec
        .iter()
        .map(|value| value.as_str())
        .collect::<Vec<_>>();
    run_command("git", &borrowed, Some(cwd))
}
