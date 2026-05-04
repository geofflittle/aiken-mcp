use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::Stdio;

use async_trait::async_trait;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tracing::debug;

use aiken_mcp_core::{
    AikenRunner, BuildOutcome, CheckOutcome, CoreError, CoreResult, FmtOutcome,
    NewProjectOutcome, Project, TestOutcome, UplcOutcome,
};

use crate::parse;

/// `AikenRunner` implementation that shells out to the `aiken` binary.
///
/// Binary path resolves once at construction. Default lookup uses `PATH`;
/// tests + alternative installs can override via `with_binary`.
#[derive(Debug, Clone)]
pub struct AikenCliRunner {
    binary: PathBuf,
}

impl AikenCliRunner {
    pub fn new() -> Self {
        Self {
            binary: PathBuf::from("aiken"),
        }
    }

    pub fn with_binary(binary: impl Into<PathBuf>) -> Self {
        Self {
            binary: binary.into(),
        }
    }

    pub fn binary(&self) -> &PathBuf {
        &self.binary
    }

    async fn run<I, S>(&self, project: Option<&Project>, args: I) -> CoreResult<RawCommand>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut cmd = Command::new(&self.binary);
        cmd.args(args);
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        if let Some(project) = project {
            cmd.current_dir(project.root.as_path());
        }

        debug!(?cmd, "spawning aiken process");

        let output = cmd.output().await.map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => CoreError::AikenNotInstalled,
            _ => CoreError::Io(err),
        })?;

        Ok(RawCommand {
            success: output.status.success(),
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    async fn run_in_dir<I, S>(&self, dir: &str, args: I) -> CoreResult<RawCommand>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut cmd = Command::new(&self.binary);
        cmd.args(args);
        cmd.current_dir(dir);
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        let output = cmd.output().await.map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => CoreError::AikenNotInstalled,
            _ => CoreError::Io(err),
        })?;
        Ok(RawCommand {
            success: output.status.success(),
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    async fn run_with_stdin<I, S>(&self, args: I, stdin_payload: &str) -> CoreResult<RawCommand>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut cmd = Command::new(&self.binary);
        cmd.args(args);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => CoreError::AikenNotInstalled,
            _ => CoreError::Io(err),
        })?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(stdin_payload.as_bytes()).await?;
            stdin.shutdown().await?;
        }

        let output = child.wait_with_output().await?;
        Ok(RawCommand {
            success: output.status.success(),
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

impl Default for AikenCliRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
struct RawCommand {
    success: bool,
    #[allow(dead_code)]
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
}

#[async_trait]
impl AikenRunner for AikenCliRunner {
    async fn check(&self, project: &Project, filter: Option<&str>) -> CoreResult<CheckOutcome> {
        let mut args = vec!["check".to_string()];
        if let Some(f) = filter {
            args.push("-m".to_string());
            args.push(f.to_string());
        }
        let raw = self.run(Some(project), args).await?;
        let diagnostics = parse::parse_check(&raw.stdout, &raw.stderr);
        Ok(CheckOutcome {
            success: raw.success,
            diagnostics,
            raw_stdout: raw.stdout,
            raw_stderr: raw.stderr,
        })
    }

    async fn build(&self, project: &Project) -> CoreResult<BuildOutcome> {
        let raw = self.run(Some(project), ["build"]).await?;
        let diagnostics = parse::parse_check(&raw.stdout, &raw.stderr);
        Ok(BuildOutcome {
            success: raw.success,
            diagnostics,
            artifacts: parse::parse_artifacts(&raw.stdout),
            raw_stdout: raw.stdout,
            raw_stderr: raw.stderr,
        })
    }

    async fn test(&self, project: &Project, filter: Option<&str>) -> CoreResult<TestOutcome> {
        let mut args = vec!["check".to_string()];
        if let Some(f) = filter {
            args.push("-m".to_string());
            args.push(f.to_string());
        }
        let raw = self.run(Some(project), args).await?;
        let tests = parse::parse_test(&raw.stdout);
        Ok(TestOutcome {
            success: raw.success,
            tests,
            raw_stdout: raw.stdout,
            raw_stderr: raw.stderr,
        })
    }

    async fn fmt(&self, source: &str) -> CoreResult<FmtOutcome> {
        let raw = self.run_with_stdin(["fmt", "--stdin"], source).await?;
        Ok(FmtOutcome {
            success: raw.success,
            formatted_source: if raw.success { Some(raw.stdout) } else { None },
            raw_stderr: raw.stderr,
        })
    }

    async fn uplc_decode(&self, project: &Project, target: &str) -> CoreResult<UplcOutcome> {
        let raw = self
            .run(Some(project), ["uplc", "decode", target])
            .await?;
        Ok(UplcOutcome {
            success: raw.success,
            uplc: raw.stdout,
            raw_stderr: raw.stderr,
        })
    }

    async fn new_project(
        &self,
        parent_dir: &str,
        name: &str,
    ) -> CoreResult<NewProjectOutcome> {
        let raw = self.run_in_dir(parent_dir, ["new", name]).await?;
        let created_path = if raw.success {
            Some(format!("{}/{}", parent_dir.trim_end_matches('/'), name))
        } else {
            None
        };
        Ok(NewProjectOutcome {
            success: raw.success,
            created_path,
            raw_stdout: raw.stdout,
            raw_stderr: raw.stderr,
        })
    }

    async fn version(&self) -> CoreResult<String> {
        let raw = self.run(None, ["--version"]).await?;
        if !raw.success {
            return Err(CoreError::AikenProcessFailed {
                exit_code: raw.exit_code,
                stderr: raw.stderr,
            });
        }
        Ok(raw.stdout.trim().to_string())
    }
}
