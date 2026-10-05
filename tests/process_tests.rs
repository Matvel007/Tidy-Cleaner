use std::io::ErrorKind;
use std::process::Command;
use std::time::{Duration, Instant};
use tidy_cleaner::process::output;

#[test]
fn captures_both_pipes_without_deadlock_and_preserves_status() {
    let result = output(Command::new("/bin/sh").args(["-c", "i=0; while [ $i -lt 20000 ]; do printf 'stdout\n'; printf 'stderr\n' >&2; i=$((i+1)); done; exit 7"]), Duration::from_secs(5)).unwrap();
    assert_eq!(result.status.code(), Some(7));
    assert_eq!(result.stdout.len(), 140000);
    assert_eq!(result.stderr.len(), 140000);
}

#[test]
fn sleeping_command_is_killed_and_reaped() {
    let directory =
        std::path::PathBuf::from(format!("/tmp/opencode/process-reap-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("child.pid");
    let start = Instant::now();
    let result = output(
        Command::new("/bin/sh")
            .args([
                "-c",
                "printf '%s' \"$$\" > \"$1\"; exec /bin/sleep 30",
                "fixture",
            ])
            .arg(&path),
        Duration::from_millis(200),
    );
    assert_eq!(result.unwrap_err().kind(), ErrorKind::TimedOut);
    assert!(start.elapsed() < Duration::from_secs(3));
    let pid = std::fs::read_to_string(&path).unwrap();
    assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn exited_parent_with_sleeping_pipe_owner_does_not_join_hang() {
    let start = Instant::now();
    let result = output(
        Command::new("/bin/sh").args(["-c", "/bin/sleep 30 & exit 0"]),
        Duration::from_millis(200),
    );
    assert_eq!(result.unwrap_err().kind(), ErrorKind::TimedOut);
    assert!(start.elapsed() < Duration::from_secs(3));
}

#[test]
fn escaped_pipe_owner_cannot_hold_up_timeout() {
    // Escaping the process group must not make the pipe drain block. This
    // deliberately short escaped child self-terminates; no long-lived fixture.
    let start = Instant::now();
    let result = output(
        Command::new("/bin/sh").args(["-c", "/usr/bin/setsid /bin/sleep 2 & exit 0"]),
        Duration::from_millis(100),
    );
    assert_eq!(result.unwrap_err().kind(), ErrorKind::TimedOut);
    assert!(start.elapsed() < Duration::from_secs(1));
}

#[test]
fn continuous_output_is_bounded_in_memory_and_time() {
    let start = Instant::now();
    let result = output(
        Command::new("/bin/sh").args([
            "-c",
            "while :; do printf '0123456789012345678901234567890123456789'; done",
        ]),
        Duration::from_millis(100),
    );
    assert_eq!(result.unwrap_err().kind(), ErrorKind::TimedOut);
    assert!(start.elapsed() < Duration::from_secs(3));
}

#[test]
fn missing_command_is_a_spawn_error() {
    let result = output(
        &mut Command::new("/tmp/opencode/no-such-provider-command"),
        Duration::from_secs(1),
    );
    assert_eq!(result.unwrap_err().kind(), ErrorKind::NotFound);
}
