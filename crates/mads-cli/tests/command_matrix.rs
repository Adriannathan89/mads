//! Black-box coverage for the complete v0.7 command surface.

#![cfg(unix)]

use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command as ProcessCommand, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

use tempfile::{TempDir, tempdir};

struct CommandCase {
    arguments: &'static [&'static str],
    expected_code: i32,
    stdout_contains: &'static [&'static str],
    stderr_contains: &'static [&'static str],
}

const USAGE_CASES: &[CommandCase] = &[
    CommandCase {
        arguments: &["foundation"],
        expected_code: 2,
        stdout_contains: &[],
        stderr_contains: &["unknown command"],
    },
    CommandCase {
        arguments: &["db", "generate", "named"],
        expected_code: 2,
        stdout_contains: &[],
        stderr_contains: &["unknown argument"],
    },
    CommandCase {
        arguments: &["db", "generate", "--diff-schema"],
        expected_code: 2,
        stdout_contains: &[],
        stderr_contains: &["unknown argument"],
    },
    CommandCase {
        arguments: &["routes", "--", "extra"],
        expected_code: 2,
        stdout_contains: &[],
        stderr_contains: &["does not accept application arguments"],
    },
];

#[test]
fn complete_command_matrix_has_stable_usage_and_exit_classes() {
    for case in USAGE_CASES {
        let output = cli_command(&single_fixture(), case.arguments)
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(case.expected_code),
            "{:?}",
            case.arguments
        );
        assert_contains_all(&output, case.stdout_contains, case.stderr_contains);
    }

    let success_cases = [
        &["--help"][..],
        &["--version"][..],
        &["routes"][..],
        &["graph"][..],
        &["doctor"][..],
    ];
    for arguments in success_cases {
        let output = cli_command(&single_fixture(), arguments).output().unwrap();
        assert_eq!(output.status.code(), Some(0), "{arguments:?}");
    }

    let output = cli_command(
        &workspace_fixture(),
        &["run", "-p", "api", "--bin", "server", "--", "--matrix-arg"],
    )
    .output()
    .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_contains_all(&output, &["matrix server args=--matrix-arg"], &[]);

    let output = cli_command(
        &workspace_fixture(),
        &["run", "-p", "api", "--", "--default-bin"],
    )
    .output()
    .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_contains_all(&output, &["matrix server args=--default-bin"], &[]);
}

#[test]
fn operational_database_failures_are_redacted_and_exit_one() {
    for arguments in [["db", "generate"].as_slice(), ["db", "status"].as_slice()] {
        let output = cli_command(&single_fixture(), arguments)
            .env_remove("DATABASE_URL")
            .env_remove("MADS_DATABASE__URL")
            .env(
                "MADS_DATABASE__URL",
                "postgres://matrix-env-secret@127.0.0.1:1/matrix",
            )
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1), "{arguments:?}");
        assert_contains_all(&output, &[], &[]);
        assert_redacted(&output);
    }
}

#[test]
fn workflow_has_cli_platform_matrix() {
    let workflow = fs::read_to_string(workspace_root().join(".github/workflows/ci.yml")).unwrap();
    for runner in ["macos-latest", "windows-latest"] {
        assert!(workflow.contains(runner), "missing runner {runner}");
    }
    assert!(workflow.contains(
        "cargo test --locked -p mads-cli --test cli --test command_matrix -- --test-threads=1"
    ));
}

#[test]
fn publish_workflows_use_linux_for_full_tests_and_platform_smoke_only() {
    for workflow_path in [
        ".github/workflows/ci.yml",
        ".github/workflows/beta-publish.yml",
        ".github/workflows/stable-publish.yml",
    ] {
        let workflow = fs::read_to_string(workspace_root().join(workflow_path)).unwrap();
        let smoke_command =
            "cargo test --locked -p mads-cli --test cli --test command_matrix -- --test-threads=1";
        assert!(
            workflow.contains("os: [macos-latest, windows-latest]"),
            "{workflow_path}"
        );
        assert!(workflow.contains(smoke_command), "{workflow_path}");
        assert!(
            !workflow.contains("cargo test --locked -p mads-cli -- --test-threads=1"),
            "{workflow_path}"
        );
    }
}

#[test]
fn cli_documentation_lists_the_exact_surface() {
    let documentation = fs::read_to_string(workspace_root().join("docs/CLI.md")).unwrap();
    for command in [
        "mads run",
        "mads dev",
        "mads routes",
        "mads graph",
        "mads doctor",
        "mads db generate",
        "mads db migrate",
        "mads db rollback",
        "mads db status",
    ] {
        assert!(documentation.contains(command), "missing {command}");
    }
    assert!(!documentation.contains("mads db generate <name>"));
    assert!(!documentation.contains("mads foundation"));
}

#[test]
fn dev_starts_an_application_and_can_be_terminated() {
    let fixture = copied_single_fixture();
    let address = available_localhost_address().unwrap();
    fs::write(
        fixture.path().join("mads.toml"),
        format!(
            "[server]\nhost = \"{}\"\nport = {}\n",
            address.ip(),
            address.port()
        ),
    )
    .unwrap();

    let output_path = fixture.path().join("dev-output.log");
    let output = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&output_path)
        .unwrap();
    let mut child = ChildGuard::new(
        cli_command(fixture.path(), &["dev"])
            .stdout(Stdio::from(output.try_clone().unwrap()))
            .stderr(Stdio::from(output))
            .spawn()
            .unwrap(),
    );
    wait_for_output(&output_path, "mads dev: starting");
    wait_for_health(address);
    child.kill();
}

fn cli_command(root: &Path, arguments: &[&str]) -> ProcessCommand {
    let mut command = ProcessCommand::new(env!("CARGO_BIN_EXE_mads"));
    command.current_dir(root).args(arguments);
    command
}

fn assert_contains_all(output: &Output, stdout_contains: &[&str], stderr_contains: &[&str]) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    for expected in stdout_contains {
        assert!(
            stdout.contains(expected),
            "stdout missing {expected}: {stdout}"
        );
    }
    for expected in stderr_contains {
        assert!(
            stderr.contains(expected),
            "stderr missing {expected}: {stderr}"
        );
    }
}

fn assert_redacted(output: &Output) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    for secret in ["matrix-config-secret", "matrix-env-secret"] {
        assert!(!stdout.contains(secret), "stdout leaked {secret}: {stdout}");
        assert!(!stderr.contains(secret), "stderr leaked {secret}: {stderr}");
    }
}

fn single_fixture() -> PathBuf {
    workspace_root().join("crates/mads-cli/tests/fixtures/matrix/single")
}

fn workspace_fixture() -> PathBuf {
    workspace_root().join("crates/mads-cli/tests/fixtures/matrix/workspace")
}

fn copied_single_fixture() -> TempDir {
    let destination = tempdir().unwrap();
    copy_directory(&single_fixture(), destination.path()).unwrap();
    let manifest_path = destination.path().join("Cargo.toml");
    let manifest = fs::read_to_string(&manifest_path).unwrap();
    let mads_path = workspace_root().join("crates/mads").canonicalize().unwrap();
    fs::write(
        manifest_path,
        manifest.replace("../../../../../mads", &mads_path.display().to_string()),
    )
    .unwrap();
    destination
}

fn copy_directory(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let destination_path = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_directory(&entry.path(), &destination_path)?;
        } else {
            fs::copy(entry.path(), destination_path)?;
        }
    }
    Ok(())
}

fn available_localhost_address() -> std::io::Result<SocketAddr> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    listener.local_addr()
}

fn wait_for_output(path: &Path, expected: &str) {
    wait_until(|| {
        fs::read_to_string(path)
            .unwrap_or_default()
            .contains(expected)
    });
}

fn wait_for_health(address: SocketAddr) {
    wait_until(|| {
        let Ok(mut stream) = TcpStream::connect_timeout(&address, Duration::from_millis(100))
        else {
            return false;
        };
        let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
        let _ = stream
            .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
        let mut response = String::new();
        stream.read_to_string(&mut response).is_ok() && response.contains("healthy")
    });
}

fn wait_until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(60);
    while !condition() {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for CLI fixture"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

struct ChildGuard(Option<Child>);

impl ChildGuard {
    fn new(child: Child) -> Self {
        Self(Some(child))
    }

    fn kill(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        self.kill();
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .to_path_buf()
}
