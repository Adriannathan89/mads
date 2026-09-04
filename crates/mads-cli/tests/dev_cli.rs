//! Black-box CLI coverage for the development command.

use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command as ProcessCommand, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use assert_cmd::{Command as AssertCommand, cargo::CommandCargoExt};
use predicates::prelude::*;

#[test]
fn scripted_dev_command_is_advertised_by_the_cli() {
    let mut command = AssertCommand::cargo_bin("mads").unwrap();

    command
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "dev       Watch, rebuild, and restart a MADS application",
        ));
}

#[test]
fn real_dev_loop() {
    let fixture = DevFixture::copy().expect("test fixture should copy");
    let address =
        available_localhost_address().expect("localhost should provide an available port");
    fixture
        .write_config(address, "one")
        .expect("fixture configuration should write");

    let mut dev = fixture.start_dev().expect("mads dev should start");
    let initial_start = wait_for_start(&fixture.start_log, 1, "initial application start");
    dev.track_application(initial_start.pid);
    wait_for_health(address, "initial application health");
    assert_eq!(log_lines(&fixture.build_log).len(), 1);
    assert!(initial_start.arguments.contains("--seed|42"));

    fixture
        .write_config(address, "two")
        .expect("configuration-only edit should write");
    let config_start = wait_for_start(&fixture.start_log, 2, "configuration restart");
    dev.track_application(config_start.pid);
    wait_for_health(address, "configuration restart health");
    assert_ne!(config_start.pid, initial_start.pid);
    assert_eq!(log_lines(&fixture.build_log).len(), 1);

    let valid_source = fs::read_to_string(&fixture.main).expect("fixture main should read");
    fs::write(
        &fixture.main,
        format!("{valid_source}\n// trigger an incremental rebuild\n"),
    )
    .expect("source edit should write");
    let rebuilt_start = wait_for_start(&fixture.start_log, 3, "source rebuild restart");
    dev.track_application(rebuilt_start.pid);
    wait_for_health(address, "source rebuild health");
    assert_ne!(rebuilt_start.pid, config_start.pid);
    assert_eq!(log_lines(&fixture.build_log).len(), 2);

    fs::write(&fixture.main, "this is intentionally invalid Rust\n")
        .expect("invalid source should write");
    wait_for_output(
        &dev.output,
        "mads dev: build failed; continuing to watch",
        "streamed compiler failure",
    );
    wait_for_health(
        address,
        "last good application health after compiler failure",
    );
    assert_eq!(log_lines(&fixture.start_log).len(), 3);
    assert_eq!(last_start(&fixture.start_log).pid, rebuilt_start.pid);
    assert_eq!(log_lines(&fixture.build_log).len(), 3);

    fs::write(&fixture.main, valid_source).expect("valid source should restore");
    let restored_start = wait_for_start(&fixture.start_log, 4, "restored source restart");
    dev.track_application(restored_start.pid);
    wait_for_health(address, "restored application health");
    assert_ne!(restored_start.pid, rebuilt_start.pid);
    assert_eq!(log_lines(&fixture.build_log).len(), 4);

    assert!(
        dev.shutdown()
            .expect("mads dev should stop cleanly")
            .success()
    );
    wait_until("health endpoint should close", || {
        (!health_is_ready(address)).then_some(())
    });
    wait_until("final application child should exit", || {
        (!process_is_running(restored_start.pid)).then_some(())
    });
    dev.disarm();
}

const PHASE_TIMEOUT: Duration = Duration::from_secs(60);
const POLL_INTERVAL: Duration = Duration::from_millis(25);

struct DevFixture {
    _directory: tempfile::TempDir,
    project: PathBuf,
    main: PathBuf,
    build_log: PathBuf,
    start_log: PathBuf,
}

impl DevFixture {
    fn copy() -> io::Result<Self> {
        let directory = tempfile::tempdir()?;
        let project = directory.path().join("project");
        copy_directory(&fixture_root(), &project)?;

        let template = fs::read_to_string(project.join("Cargo.toml.template"))?;
        let mads_path = workspace_root().join("crates/mads").canonicalize()?;
        let manifest = template.replace("@MADS_PATH@", &toml_path(&mads_path));
        fs::write(project.join("Cargo.toml"), manifest)?;
        fs::remove_file(project.join("Cargo.toml.template"))?;

        Ok(Self {
            main: project.join("src/main.rs"),
            build_log: directory.path().join("build.log"),
            start_log: directory.path().join("start.log"),
            _directory: directory,
            project,
        })
    }

    fn write_config(&self, address: SocketAddr, reload: &str) -> io::Result<()> {
        fs::write(
            self.project.join("mads.toml"),
            format!(
                "[server]\nhost = \"{}\"\nport = \"{}\"\n\n[test]\nreload = \"{reload}\"\n",
                address.ip(),
                address.port()
            ),
        )
    }

    fn start_dev(&self) -> io::Result<DevProcess> {
        let output = self.project.parent().unwrap().join("dev-output.log");
        let output_file = OpenOptions::new().create(true).append(true).open(&output)?;
        let mut command = ProcessCommand::cargo_bin("mads").map_err(io::Error::other)?;
        #[cfg(windows)]
        command.creation_flags(CREATE_NEW_CONSOLE);
        let child = command
            .current_dir(&self.project)
            .args(["dev", "--", "--seed", "42"])
            .env("MADS_TEST_BUILD_LOG", &self.build_log)
            .env("MADS_TEST_START_LOG", &self.start_log)
            .stdin(Stdio::null())
            .stdout(Stdio::from(output_file.try_clone()?))
            .stderr(Stdio::from(output_file))
            .spawn()?;
        Ok(DevProcess {
            child: Some(child),
            output,
            start_log: self.start_log.clone(),
            application_pids: Vec::new(),
        })
    }
}

struct DevProcess {
    child: Option<Child>,
    output: PathBuf,
    start_log: PathBuf,
    application_pids: Vec<u32>,
}

impl DevProcess {
    fn track_application(&mut self, pid: u32) {
        self.application_pids.push(pid);
    }

    fn shutdown(&mut self) -> io::Result<ExitStatus> {
        let pid = self
            .child
            .as_ref()
            .expect("dev process should be live")
            .id();
        request_dev_shutdown(pid)?;
        let status = wait_until("mads dev process should exit", || {
            self.child
                .as_mut()
                .expect("dev process should be live")
                .try_wait()
                .expect("mads dev exit status should be readable")
        });
        self.child.take();
        Ok(status)
    }

    fn disarm(&mut self) {
        self.application_pids.clear();
    }

    fn force_cleanup(&mut self) {
        let mut application_pids = std::mem::take(&mut self.application_pids);
        application_pids.extend(log_lines(&self.start_log).into_iter().filter_map(|line| {
            line.split_once('|')
                .and_then(|(pid, _)| pid.parse::<u32>().ok())
        }));
        application_pids.sort_unstable();
        application_pids.dedup();
        for pid in application_pids {
            let _ = force_kill_process(pid);
        }
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for DevProcess {
    fn drop(&mut self) {
        self.force_cleanup();
    }
}

#[derive(Debug)]
struct StartRecord {
    pid: u32,
    arguments: String,
}

fn wait_for_start(path: &Path, count: usize, phase: &str) -> StartRecord {
    wait_until(phase, || {
        let lines = log_lines(path);
        (lines.len() >= count).then(|| parse_start(&lines[count - 1]))
    })
}

fn last_start(path: &Path) -> StartRecord {
    let lines = log_lines(path);
    parse_start(lines.last().expect("start log should contain an entry"))
}

fn parse_start(line: &str) -> StartRecord {
    let (pid, arguments) = line
        .split_once('|')
        .expect("start log entries should contain a pid and arguments");
    StartRecord {
        pid: pid.parse().expect("start log pid should be numeric"),
        arguments: arguments.to_owned(),
    }
}

fn wait_for_health(address: SocketAddr, phase: &str) {
    wait_until(phase, || health_is_ready(address).then_some(()));
}

fn wait_for_output(path: &Path, needle: &str, phase: &str) {
    wait_until(phase, || {
        fs::read_to_string(path)
            .unwrap_or_default()
            .contains(needle)
            .then_some(())
    });
}

fn wait_until<T>(phase: &str, mut check: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + PHASE_TIMEOUT;
    loop {
        if let Some(value) = check() {
            return value;
        }
        assert!(
            Instant::now() < deadline,
            "timed out after {} seconds waiting for {phase}",
            PHASE_TIMEOUT.as_secs()
        );
        thread::sleep(POLL_INTERVAL);
    }
}

fn health_is_ready(address: SocketAddr) -> bool {
    let Ok(mut stream) = TcpStream::connect_timeout(&address, POLL_INTERVAL) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(POLL_INTERVAL));
    let _ = stream.set_write_timeout(Some(POLL_INTERVAL));
    if stream
        .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .is_err()
    {
        return false;
    }
    let mut response = String::new();
    stream.read_to_string(&mut response).is_ok()
        && response.starts_with("HTTP/1.1 200")
        && response.ends_with("healthy")
}

fn log_lines(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(ToOwned::to_owned)
        .collect()
}

fn available_localhost_address() -> io::Result<SocketAddr> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    listener.local_addr()
}

fn copy_directory(source: &Path, destination: &Path) -> io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let destination = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_directory(&entry.path(), &destination)?;
        } else {
            fs::copy(entry.path(), destination)?;
        }
    }
    Ok(())
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dev")
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("mads-cli crate should be in the workspace")
        .to_path_buf()
}

fn toml_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "\\\\")
}

#[cfg(unix)]
fn request_dev_shutdown(pid: u32) -> io::Result<()> {
    let status = ProcessCommand::new("kill")
        .args(["-INT", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other("could not signal mads dev"))
    }
}

#[cfg(windows)]
fn request_dev_shutdown(pid: u32) -> io::Result<()> {
    let status = ProcessCommand::new(std::env::current_exe()?)
        .args(["--exact", "windows_dev_shutdown_helper", "--nocapture"])
        .env("MADS_TEST_DEV_PID", pid.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other("could not send Ctrl-C to mads dev"))
    }
}

// Windows does not let a Ctrl-C event target one process group. The test starts
// `mads dev` in a dedicated console, then this helper attaches to that console
// and sends Ctrl-C only to its processes. Keeping the P/Invoke inside PowerShell
// makes this test-only path work without unsafe Rust or production signal code.
#[cfg(windows)]
#[test]
fn windows_dev_shutdown_helper() {
    let Some(pid) = std::env::var_os("MADS_TEST_DEV_PID") else {
        return;
    };
    let script = r#"
$signature = @'
using System;
using System.Runtime.InteropServices;

public static class MadsDevTestConsole {
    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool FreeConsole();

    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool AttachConsole(uint processId);

    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool SetConsoleCtrlHandler(IntPtr handler, bool add);

    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool GenerateConsoleCtrlEvent(uint eventType, uint processGroupId);
}
'@
Add-Type -TypeDefinition $signature
$devPid = [uint32]$env:MADS_TEST_DEV_PID
[MadsDevTestConsole]::FreeConsole() | Out-Null
if (-not [MadsDevTestConsole]::AttachConsole($devPid)) {
    throw "could not attach to mads dev console: $([Runtime.InteropServices.Marshal]::GetLastWin32Error())"
}
[MadsDevTestConsole]::SetConsoleCtrlHandler([IntPtr]::Zero, $true) | Out-Null
if (-not [MadsDevTestConsole]::GenerateConsoleCtrlEvent(0, 0)) {
    throw "could not generate Ctrl-C: $([Runtime.InteropServices.Marshal]::GetLastWin32Error())"
}
"#;
    let status = ProcessCommand::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("MADS_TEST_DEV_PID", pid)
        .status()
        .expect("Windows Ctrl-C helper should run");
    assert!(status.success(), "Windows Ctrl-C helper should succeed");
}

#[cfg(windows)]
const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;

#[cfg(not(any(unix, windows)))]
fn request_dev_shutdown(pid: u32) -> io::Result<()> {
    force_kill_process(pid)
}

#[cfg(unix)]
fn force_kill_process(pid: u32) -> io::Result<()> {
    let _ = ProcessCommand::new("kill")
        .args(["-KILL", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    Ok(())
}

#[cfg(windows)]
fn force_kill_process(pid: u32) -> io::Result<()> {
    let _ = ProcessCommand::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn force_kill_process(_pid: u32) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn process_is_running(pid: u32) -> bool {
    ProcessCommand::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(windows)]
fn process_is_running(pid: u32) -> bool {
    ProcessCommand::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH"])
        .output()
        .is_ok_and(|output| {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout.contains(&pid.to_string()) && !stdout.contains("No tasks are running")
        })
}

#[cfg(not(any(unix, windows)))]
fn process_is_running(_pid: u32) -> bool {
    false
}
