//! Bounded read-only subprocesses. Do not use deadlines for package mutations.
use std::io::{self, Read};
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

pub const SCAN_TIMEOUT: Duration = Duration::from_secs(10);

struct OwnedChild(Child, bool);

impl Drop for OwnedChild {
    fn drop(&mut self) {
        // The group includes descendants holding the capture pipes open. Never
        // join blocking pipe-reader threads: an escaped descendant could hang them.
        if self.1 {
            unsafe {
                libc::kill(-(self.0.id() as i32), libc::SIGKILL);
            }
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}

pub fn output(command: &mut Command, timeout: Duration) -> io::Result<Output> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command.process_group(0);
    let start = Instant::now();
    let mut child = OwnedChild(command.spawn()?, true);
    let mut stdout = child.0.stdout.take().unwrap();
    let mut stderr = child.0.stderr.take().unwrap();
    for fd in [stdout.as_raw_fd(), stderr.as_raw_fd()] {
        unsafe {
            let flags = libc::fcntl(fd, libc::F_GETFL);
            if flags < 0 || libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
                return Err(io::Error::last_os_error());
            }
        }
    }
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut eof = [false; 2];
    let mut status = None;
    loop {
        // Limit work per iteration so a continuously writing child cannot starve
        // the deadline. Bound captured memory as well as child lifetime.
        for (index, (reader, bytes)) in [
            (&mut stdout as &mut dyn Read, &mut out),
            (&mut stderr as &mut dyn Read, &mut err),
        ]
        .into_iter()
        .enumerate()
        {
            if eof[index] {
                continue;
            }
            for _ in 0..16 {
                let mut buffer = [0u8; 8192];
                match reader.read(&mut buffer) {
                    Ok(0) => {
                        eof[index] = true;
                        break;
                    }
                    Ok(n) => {
                        if bytes.len() + n > 16 * 1024 * 1024 {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "subprocess capture exceeds 16 MiB",
                            ));
                        }
                        bytes.extend_from_slice(&buffer[..n]);
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e),
                }
            }
        }
        if status.is_none() {
            status = child.0.try_wait()?;
        }
        if let Some(status) = status {
            if eof.iter().all(|v| *v) {
                child.1 = false;
                return Ok(Output {
                    status,
                    stdout: out,
                    stderr: err,
                });
            }
        }
        let remaining = timeout
            .checked_sub(start.elapsed())
            .filter(|v| !v.is_zero())
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::TimedOut, "subprocess deadline exceeded")
            })?;
        let mut fds = [
            libc::pollfd {
                fd: if eof[0] { -1 } else { stdout.as_raw_fd() },
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: if eof[1] { -1 } else { stderr.as_raw_fd() },
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        let ms = remaining.as_millis().clamp(1, 10) as i32;
        if unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, ms) } < 0 {
            let e = io::Error::last_os_error();
            if e.kind() != io::ErrorKind::Interrupted {
                return Err(e);
            }
        }
    }
}

/// Adapter for existing provider command chains. Only read-only calls use this.
pub trait ReadOnlyCommand {
    fn scan_output(&mut self) -> io::Result<Output>;
}

impl ReadOnlyCommand for Command {
    fn scan_output(&mut self) -> io::Result<Output> {
        output(self.env("LC_ALL", "C"), SCAN_TIMEOUT)
    }
}
