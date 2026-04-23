use std::path::{Path, PathBuf};
use std::process::{Child, Command};

use tempfile::TempDir;

/// Test harness that spawns a real `rallyd` process on an ephemeral socket.
///
/// The daemon is killed on `Drop` — tests will not leak processes.
pub struct DaemonHarness {
    child: Option<Child>,
    _temp_dir: TempDir,
    socket_path: PathBuf,
}

impl DaemonHarness {
    /// Start a daemon. `rallyd_bin` is the path to the compiled `rallyd` binary.
    pub fn start(rallyd_bin: &Path) -> Self {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let socket_path = temp_dir.path().join("rally-test.sock");
        let _db_path = temp_dir.path().join("test-state.db");

        let child = Command::new(rallyd_bin)
            .env("RALLY_DAEMON_SOCKET_PATH", &socket_path)
            .env("XDG_DATA_HOME", temp_dir.path())
            .env("RALLY_LOG", "rally=debug")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("failed to spawn rallyd");

        let harness = Self {
            child: Some(child),
            _temp_dir: temp_dir,
            socket_path: socket_path.clone(),
        };

        // Wait for socket to appear
        for _ in 0..50 {
            if socket_path.exists() {
                std::thread::sleep(std::time::Duration::from_millis(50));
                return harness;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        panic!("rallyd did not start within 5 seconds (socket: {})", socket_path.display());
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Find the rallyd binary in the cargo target directory.
    pub fn find_rallyd() -> PathBuf {
        let mut path = std::env::current_exe()
            .expect("cannot determine test binary path");
        // test binary is in target/debug/deps/<name>
        // rallyd is in target/debug/rallyd
        path.pop(); // remove binary name
        if path.ends_with("deps") {
            path.pop(); // remove "deps"
        }
        path.push("rallyd");
        assert!(path.exists(), "rallyd not found at {}. Build it first: cargo build -p rally-daemon", path.display());
        path
    }
}

impl Drop for DaemonHarness {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
