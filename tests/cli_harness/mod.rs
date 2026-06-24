//! Minimal black-box harness (mirrors poc-10 `tests/cli_harness`): build the `lk`
//! binary into an isolated target dir, run it, spawn daemons, parse `field: value`
//! output. Kept small; serial test runs (`.cargo/config.toml`) avoid port races.
#![allow(dead_code)]

use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Output, Stdio};
use std::sync::OnceLock;

pub fn lk_cli(args: &[&str]) -> Output {
    Command::new(lk_bin()).args(args).output().expect("run lk")
}

pub fn assert_success(out: Output) -> String {
    assert!(
        out.status.success(),
        "command failed\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// The value of a `field: value` line.
pub fn line_value(text: &str, field: &str) -> String {
    let prefix = format!("{field}:");
    text.lines()
        .find_map(|l| l.strip_prefix(&prefix))
        .map(|v| v.trim().to_string())
        .unwrap_or_else(|| panic!("field `{field}` not found in:\n{text}"))
}

pub fn temp_db(dir: &tempfile::TempDir, name: &str) -> String {
    dir.path().join(name).to_string_lossy().to_string()
}

pub fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// Build `lk` once into an isolated target dir (so the shared cargo target dir
/// can't hand us a stale/foreign binary), and return its path.
fn lk_bin() -> &'static Path {
    static BIN: OnceLock<PathBuf> = OnceLock::new();
    BIN.get_or_init(|| {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let target = manifest.join("target").join("cli-black-box");
        let status = Command::new("cargo")
            .args(["build", "--quiet", "--bin", "lk", "--manifest-path"])
            .arg(manifest.join("Cargo.toml"))
            .arg("--target-dir")
            .arg(&target)
            .status()
            .expect("build lk");
        assert!(status.success(), "build lk");
        target.join("debug").join("lk")
    })
}

/// A daemon child, killed on drop. We keep its stdout reader alive so the daemon
/// never gets SIGPIPE writing its (single) `listening:` line.
pub struct RunningDaemon {
    child: Child,
    _stdout: BufReader<ChildStdout>,
    pub port: u16,
}

impl Drop for RunningDaemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Spawn `lk --db DB start --listen 127.0.0.1 PORT [--peer 127.0.0.1 P]...` and
/// block until it prints `listening:`.
pub fn spawn_daemon(db: &str, port: u16, peers: &[u16]) -> RunningDaemon {
    let mut args: Vec<String> = vec![
        "--db".into(),
        db.into(),
        "start".into(),
        "--listen".into(),
        "127.0.0.1".into(),
        port.to_string(),
    ];
    for p in peers {
        args.push("--peer".into());
        args.push("127.0.0.1".into());
        args.push(p.to_string());
    }
    let mut child = Command::new(lk_bin())
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn lk daemon");
    let mut reader = BufReader::new(child.stdout.take().expect("daemon stdout"));
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line).expect("read daemon stdout");
        assert!(n > 0, "daemon exited before listening");
        if line.starts_with("listening:") {
            break;
        }
    }
    RunningDaemon {
        child,
        _stdout: reader,
        port,
    }
}
