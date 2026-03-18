use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

pub fn copy_path_to_clipboard(path: &Path) -> Result<(), String> {
    let mut child = Command::new("pbcopy")
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to start pbcopy: {e}"))?;

    if let Some(stdin) = &mut child.stdin {
        stdin
            .write_all(path.to_string_lossy().as_bytes())
            .map_err(|e| format!("failed to write clipboard content: {e}"))?;
    }

    let status = child
        .wait()
        .map_err(|e| format!("failed to wait pbcopy: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("pbcopy failed".to_string())
    }
}

pub fn prompt_sudo_auth() -> Result<(), String> {
    let status = Command::new("sudo")
        .arg("-v")
        .status()
        .map_err(|e| format!("failed to invoke sudo: {e}"))?;

    if status.success() {
        Ok(())
    } else {
        Err("sudo authentication failed".to_string())
    }
}
