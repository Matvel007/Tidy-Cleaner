use anyhow::{bail, Context, Result};
use std::process::Command;

pub struct PolkitExecutor;

impl PolkitExecutor {
    /// Executes a system command with elevated privileges using Polkit (pkexec).
    /// Safe: Never stores or exposes user passwords.
    pub fn run_with_pkexec(program: &str, args: &[&str]) -> Result<()> {
        let mut cmd = Command::new("pkexec");
        cmd.arg(program);
        for arg in args {
            cmd.arg(arg);
        }

        let output = cmd
            .output()
            .with_context(|| format!("Failed to spawn pkexec for {}", program))?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let code = output.status.code().unwrap_or(-1);
            if code == 126 || code == 127 {
                bail!("Authorization cancelled or denied by user");
            }
            bail!("Command failed (exit code {}): {}", code, stderr.trim())
        }
    }
}
