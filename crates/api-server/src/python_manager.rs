use crate::embedded_frontend::FrontendAssets;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::process::{Child, Command};
use tokio::sync::watch;

const MAX_RESTARTS: u32 = 5;
const RESTART_DELAY_SECS: u64 = 3;

#[derive(Debug, Clone, PartialEq)]
pub enum FrontendStatus {
    NotStarted,
    ExtractingFiles,
    InstallingDependencies,
    Starting,
    Running,
    Restarting(u32),
    Failed(String),
    Stopped,
}

impl std::fmt::Display for FrontendStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotStarted => write!(f, "Not started"),
            Self::ExtractingFiles => write!(f, "Extracting files"),
            Self::InstallingDependencies => write!(f, "Installing dependencies"),
            Self::Starting => write!(f, "Starting"),
            Self::Running => write!(f, "Running"),
            Self::Restarting(n) => write!(f, "Restarting (attempt {}/{})", n, MAX_RESTARTS),
            Self::Failed(msg) => write!(f, "Failed: {}", msg),
            Self::Stopped => write!(f, "Stopped"),
        }
    }
}

pub struct PythonManager {
    base_dir: PathBuf,
    status_tx: watch::Sender<FrontendStatus>,
    status_rx: watch::Receiver<FrontendStatus>,
    api_base_url: String,
    api_key: Option<String>,
    dash_port: u16,
}

impl PythonManager {
    pub fn new() -> anyhow::Result<Self> {
        let home =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
        let base_dir = home.join(".investiq").join("frontend");
        let (status_tx, status_rx) = watch::channel(FrontendStatus::NotStarted);

        let api_base_url =
            std::env::var("API_BASE_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());
        let api_key = std::env::var("API_KEY").ok();
        let dash_port: u16 = std::env::var("DASH_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(8050);

        Ok(Self {
            base_dir,
            status_tx,
            status_rx,
            api_base_url,
            api_key,
            dash_port,
        })
    }

    /// Extract embedded frontend files to ~/.investiq/frontend/.
    /// Skips files whose SHA256 already matches (fast on subsequent runs).
    fn extract_files(&self) -> anyhow::Result<bool> {
        let _ = self.status_tx.send(FrontendStatus::ExtractingFiles);
        let mut any_changed = false;

        std::fs::create_dir_all(&self.base_dir)?;

        for filename in FrontendAssets::iter() {
            let file_data = FrontendAssets::get(&filename)
                .ok_or_else(|| anyhow::anyhow!("Missing embedded file: {}", filename))?;

            let dest = self.base_dir.join(filename.as_ref());

            // Check if existing file matches by hash
            if dest.exists() {
                if let Ok(existing) = std::fs::read(&dest) {
                    let existing_hash = hex::encode(Sha256::digest(&existing));
                    let new_hash = hex::encode(Sha256::digest(&file_data.data));
                    if existing_hash == new_hash {
                        continue;
                    }
                }
            }

            // Write the file
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&dest, &file_data.data)?;
            any_changed = true;
        }

        if any_changed {
            tracing::info!("Frontend files extracted to {}", self.base_dir.display());
        } else {
            tracing::info!("Frontend files up-to-date, skipping extraction");
        }

        Ok(any_changed)
    }

    /// Locate python3 on the system.
    fn find_python(&self) -> anyhow::Result<String> {
        // Check common names
        for name in &["python3", "python"] {
            let result = std::process::Command::new(name)
                .arg("--version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            if let Ok(status) = result {
                if status.success() {
                    return Ok(name.to_string());
                }
            }
        }
        Err(anyhow::anyhow!(
            "Python 3 not found. Please install Python 3.8+ and ensure 'python3' is in PATH."
        ))
    }

    /// Path to the venv's python executable.
    fn venv_python(&self) -> PathBuf {
        let venv_dir = self.base_dir.join("venv");
        if cfg!(windows) {
            venv_dir.join("Scripts").join("python.exe")
        } else {
            venv_dir.join("bin").join("python")
        }
    }

    /// Path to the venv's pip executable.
    fn venv_pip(&self) -> PathBuf {
        let venv_dir = self.base_dir.join("venv");
        if cfg!(windows) {
            venv_dir.join("Scripts").join("pip.exe")
        } else {
            venv_dir.join("bin").join("pip")
        }
    }

    /// Path to the venv's gunicorn executable.
    fn venv_gunicorn(&self) -> PathBuf {
        let venv_dir = self.base_dir.join("venv");
        if cfg!(windows) {
            venv_dir.join("Scripts").join("gunicorn.exe")
        } else {
            venv_dir.join("bin").join("gunicorn")
        }
    }

    /// Hash file that tracks the last-installed requirements hash.
    fn requirements_hash_path(&self) -> PathBuf {
        self.base_dir.join("venv").join(".requirements_hash")
    }

    /// Create venv if missing, install deps if requirements hash changed.
    fn ensure_venv(&self) -> anyhow::Result<()> {
        let _ = self.status_tx.send(FrontendStatus::InstallingDependencies);
        let venv_dir = self.base_dir.join("venv");
        let python = self.find_python()?;

        // Create venv if it doesn't exist
        if !self.venv_python().exists() {
            tracing::info!("Creating virtual environment...");
            let status = std::process::Command::new(&python)
                .args(["-m", "venv", &venv_dir.to_string_lossy()])
                .status()?;
            if !status.success() {
                return Err(anyhow::anyhow!("Failed to create virtual environment"));
            }
        }

        // Check if requirements have changed
        let req_path = self.base_dir.join("requirements.txt");
        if !req_path.exists() {
            tracing::warn!("No requirements.txt found, skipping pip install");
            return Ok(());
        }

        let req_contents = std::fs::read(&req_path)?;
        let current_hash = hex::encode(Sha256::digest(&req_contents));

        let needs_install = match std::fs::read_to_string(self.requirements_hash_path()) {
            Ok(saved_hash) => saved_hash.trim() != current_hash,
            Err(_) => true,
        };

        if needs_install {
            tracing::info!("Installing Python dependencies...");
            let status = std::process::Command::new(self.venv_pip())
                .args(["install", "-r", &req_path.to_string_lossy(), "--quiet"])
                .status()?;
            if !status.success() {
                return Err(anyhow::anyhow!("pip install failed"));
            }
            std::fs::write(self.requirements_hash_path(), &current_hash)?;
            tracing::info!("Python dependencies installed successfully");
        } else {
            tracing::info!("Python dependencies up-to-date");
        }

        Ok(())
    }

    /// Kill any process currently listening on the given port.
    #[cfg(unix)]
    fn kill_port(port: u16) {
        let my_pid = std::process::id() as i32;
        let output = std::process::Command::new("lsof")
            .args(["-ti", &format!(":{}", port)])
            .output();

        if let Ok(out) = output {
            let pids: Vec<i32> = String::from_utf8_lossy(&out.stdout)
                .split_whitespace()
                .filter_map(|s| s.parse::<i32>().ok())
                .filter(|&pid| pid != my_pid)
                .collect();

            if pids.is_empty() {
                return;
            }

            for &pid in &pids {
                tracing::info!("Killing stale process on port {} (PID: {})", port, pid);
                unsafe {
                    libc::kill(pid, libc::SIGTERM);
                }
            }

            // Wait up to 2s for processes to die, then SIGKILL
            for _ in 0..20 {
                std::thread::sleep(std::time::Duration::from_millis(100));
                let check = std::process::Command::new("lsof")
                    .args(["-ti", &format!(":{}", port)])
                    .output();
                let still_alive = check
                    .map(|o| {
                        String::from_utf8_lossy(&o.stdout)
                            .split_whitespace()
                            .any(|s| s.parse::<i32>().ok().is_some_and(|p| p != my_pid))
                    })
                    .unwrap_or(false);
                if !still_alive {
                    return;
                }
            }

            // Escalate to SIGKILL
            for &pid in &pids {
                tracing::warn!("SIGKILL on PID {} (port {} still occupied)", pid, port);
                unsafe {
                    libc::kill(pid, libc::SIGKILL);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    }

    /// Spawn the Dash app child process using gunicorn for production.
    fn spawn_child(&self) -> anyhow::Result<Child> {
        #[cfg(unix)]
        Self::kill_port(self.dash_port);
        let workers = std::env::var("DASH_WORKERS")
            .ok()
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(4);
        let bind = format!("0.0.0.0:{}", self.dash_port);

        let gunicorn = self.venv_gunicorn();
        let use_gunicorn = gunicorn.exists();

        let mut cmd = if use_gunicorn {
            let mut c = Command::new(&gunicorn);
            c.args([
                "--bind",
                &bind,
                "--workers",
                &workers.to_string(),
                "--timeout",
                "120",
                "app:server",
            ]);
            c
        } else {
            tracing::warn!("gunicorn not found in venv, falling back to dev server");
            let mut c = Command::new(self.venv_python());
            c.arg("app.py");
            c.env("DASH_HOST", "0.0.0.0");
            c.env("DASH_PORT", self.dash_port.to_string());
            c
        };

        cmd.current_dir(&self.base_dir)
            .env("API_BASE_URL", &self.api_base_url)
            .env("DASH_DEBUG", "false")
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        if let Some(ref key) = self.api_key {
            cmd.env("API_KEY", key);
        }

        // On Unix, set the child as its own process group leader
        // so we can signal the whole group on shutdown
        #[cfg(unix)]
        unsafe {
            cmd.pre_exec(|| {
                libc::setpgid(0, 0);
                Ok(())
            });
        }

        let child = cmd.spawn()?;
        Ok(child)
    }

    /// Start the frontend: extract, venv, spawn, and supervise.
    /// Returns a JoinHandle for the supervisor task.
    /// `child_pid` is written with the current child PID so the shutdown path
    /// can kill the process group directly.
    pub fn start(
        self,
        child_pid: Arc<AtomicU32>,
    ) -> anyhow::Result<(tokio::task::JoinHandle<()>, watch::Receiver<FrontendStatus>)> {
        // Extract and install synchronously before spawning
        self.extract_files()?;
        self.ensure_venv()?;

        let status_rx = self.status_rx.clone();
        let handle = tokio::spawn(async move {
            self.supervise_loop(child_pid).await;
        });

        Ok((handle, status_rx))
    }

    /// Supervisor loop: spawn child, wait for exit, restart if crashed.
    async fn supervise_loop(self, child_pid: Arc<AtomicU32>) {
        let mut restarts: u32 = 0;

        loop {
            let _ = self.status_tx.send(FrontendStatus::Starting);

            let mut child = match self.spawn_child() {
                Ok(c) => c,
                Err(e) => {
                    let msg = format!("Failed to spawn frontend: {}", e);
                    tracing::error!("{}", msg);
                    let _ = self.status_tx.send(FrontendStatus::Failed(msg));
                    return;
                }
            };

            let pid = child.id().unwrap_or(0);
            child_pid.store(pid, Ordering::Release);
            tracing::info!(
                "Dashboard started (PID: {}) at http://localhost:{}",
                pid,
                self.dash_port,
            );
            let _ = self.status_tx.send(FrontendStatus::Running);

            // Wait for child to exit
            match child.wait().await {
                Ok(status) if status.success() => {
                    // Clean exit (e.g. we sent SIGTERM)
                    tracing::info!("Frontend process exited cleanly");
                    let _ = self.status_tx.send(FrontendStatus::Stopped);
                    return;
                }
                Ok(status) => {
                    tracing::warn!("Frontend process exited with status: {}", status);
                }
                Err(e) => {
                    tracing::warn!("Error waiting on frontend process: {}", e);
                }
            }

            // Restart logic
            restarts += 1;
            if restarts > MAX_RESTARTS {
                let msg = format!("Frontend crashed {} times, giving up", restarts - 1);
                tracing::error!("{}", msg);
                let _ = self.status_tx.send(FrontendStatus::Failed(msg));
                return;
            }

            tracing::info!(
                "Restarting frontend in {}s (attempt {}/{})",
                RESTART_DELAY_SECS,
                restarts,
                MAX_RESTARTS,
            );
            let _ = self.status_tx.send(FrontendStatus::Restarting(restarts));
            tokio::time::sleep(std::time::Duration::from_secs(RESTART_DELAY_SECS)).await;
        }
    }
}
