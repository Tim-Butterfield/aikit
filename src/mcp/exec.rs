//! Script execution for the MCP `run` tool.
//!
//! This is deliberately *not* the CLI's runner: nothing is persisted, no run record is
//! written, and the script is generated from call arguments rather than read from an
//! allowed location.
//!
//! Correctness constraints that shape the code:
//!
//! * **stdout, stderr and stdin each get their own task.** Reading only stdout/stderr is
//!   not enough: a stdin write larger than the pipe buffer (4 KB on Windows anonymous
//!   pipes) to a child that fills stdout first deadlocks both sides.
//! * **A broken stdin pipe is normal.** A child that exits early without draining stdin
//!   produces `EPIPE`/`ERROR_BROKEN_PIPE`; that must not replace the process result.
//! * **The capture loop shares the child's deadline.** A surviving grandchild holding the
//!   stdout write end means EOF never arrives, so waiting for EOF alone can outlast the
//!   timeout that exists to prevent exactly that.
//! * **Byte budget is combined across streams, with per-stream truncation flags**, and
//!   `truncate` keeps draining and discarding so the child never blocks on a full pipe.

use std::collections::BTreeMap;
use std::process::Stdio;
use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::sync::mpsc;

/// Why execution stopped. There is deliberately no `cancelled`: the protocol forbids
/// responding to a cancelled request, so a cancelled call produces no result at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    Exited,
    Timeout,
    OutputLimit,
}

impl StopReason {
    pub fn as_str(self) -> &'static str {
        match self {
            StopReason::Exited => "exited",
            StopReason::Timeout => "timeout",
            StopReason::OutputLimit => "output_limit",
        }
    }
}

/// What to do when the byte budget is exhausted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnOutputLimit {
    /// Keep running and keep draining, but stop recording.
    Truncate,
    /// Kill the process tree.
    Kill,
}

pub struct ExecRequest {
    pub program: String,
    pub argv_flags: Vec<String>,
    pub script_path: std::path::PathBuf,
    pub cwd: std::path::PathBuf,
    pub stdin: Option<String>,
    pub env: BTreeMap<String, Option<String>>,
    pub env_minimal: bool,
    pub timeout: Duration,
    pub max_bytes: Option<usize>,
    pub on_output_limit: OnOutputLimit,
}

pub struct ExecOutcome {
    pub stop_reason: StopReason,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub duration_ms: u64,
    /// The mechanism that would contain (and kill) the script's descendants.
    ///
    /// Reported rather than claimed. "Kills the process tree" is not something aikit can
    /// promise unconditionally, and a caller that believes it will not go looking for the
    /// orphan that outlived a timeout. Naming the mechanism says exactly what protection is
    /// in force, and what can still escape it. See [`containment`].
    pub containment: &'static str,
}

/// The process-containment mechanism in force on this platform.
///
/// - `job_object` (Windows): the child is created suspended, assigned to a kill-on-close Job
///   Object, then resumed, so descendants cannot be spawned outside the job. A process
///   created with explicit breakaway can still escape.
/// - `process_group` (Unix): the child leads its own process group and the group is signalled
///   (SIGTERM, then SIGKILL). A descendant that calls `setsid`/`setpgid` leaves the group and
///   survives.
///
/// Establishing the mechanism is a precondition for running at all — a failed `setpgid` fails
/// the spawn and a failed job assignment is reported as `setup_failed` — so a call that
/// executes always has one of these in force, never neither.
pub const fn containment() -> &'static str {
    if cfg!(windows) {
        "job_object"
    } else {
        "process_group"
    }
}

/// Which stream a chunk came from.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stream {
    Out,
    Err,
}

/// Run the script to completion, a timeout, or cancellation.
///
/// `cancel` is the request's own token from `RequestContext`. It resolves when the client
/// cancels the request; the process tree is then killed and `None` is returned — the caller
/// sends no response.
pub async fn execute(
    req: ExecRequest,
    cancel: tokio_util::sync::CancellationToken,
) -> std::io::Result<Option<ExecOutcome>> {
    let started = Instant::now();

    let mut cmd = Command::new(&req.program);
    cmd.args(&req.argv_flags)
        .arg(&req.script_path)
        .current_dir(&req.cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    super::envmap::apply(&mut cmd, &req.env, req.env_minimal);
    #[cfg(unix)]
    super::platform::pre_spawn_unix(&mut cmd);
    #[cfg(windows)]
    super::platform::pre_spawn_windows(&mut cmd);

    let mut child = cmd.spawn()?;

    #[cfg(windows)]
    let _job = super::platform::assign_job_and_resume(&child)?;

    // stdin on its own task; a broken pipe is a normal early-exit signal.
    if let Some(mut sink) = child.stdin.take() {
        let data = req.stdin.clone().unwrap_or_default();
        tokio::spawn(async move {
            let _ = sink.write_all(data.as_bytes()).await;
            let _ = sink.shutdown().await;
        });
    }

    let (tx, mut rx) = mpsc::channel::<(Stream, Vec<u8>)>(64);
    if let Some(mut out) = child.stdout.take() {
        let tx = tx.clone();
        tokio::spawn(async move { pump(&mut out, Stream::Out, tx).await });
    }
    if let Some(mut err) = child.stderr.take() {
        let tx = tx.clone();
        tokio::spawn(async move { pump(&mut err, Stream::Err, tx).await });
    }
    drop(tx);

    let mut stdout_buf: Vec<u8> = Vec::new();
    let mut stderr_buf: Vec<u8> = Vec::new();
    let mut used: usize = 0;
    let mut out_trunc = false;
    let mut err_trunc = false;
    let mut limit_hit = false;

    let deadline = tokio::time::sleep(req.timeout);
    tokio::pin!(deadline);

    let mut exit_code: Option<i32> = None;
    let mut stop = StopReason::Exited;

    loop {
        tokio::select! {
            // Capture is bounded by the same deadline as the child: a grandchild holding
            // the write end means EOF may never arrive.
            _ = &mut deadline => {
                stop = StopReason::Timeout;
                let _ = kill_tree(&mut child).await;
                break;
            }
            _ = cancel.cancelled() => {
                let _ = kill_tree(&mut child).await;
                return Ok(None);
            }
            chunk = rx.recv() => {
                match chunk {
                    None => {
                        // Both pipes closed, but that does not mean the child exited: it
                        // may have closed its streams and kept running. Reaping must stay
                        // inside the same deadline, or such a child outlives the timeout
                        // that exists to stop it.
                        let remaining = deadline.deadline().saturating_duration_since(
                            tokio::time::Instant::now(),
                        );
                        match tokio::time::timeout(remaining, child.wait()).await {
                            Ok(Ok(status)) => exit_code = status.code(),
                            Ok(Err(_)) => exit_code = None,
                            Err(_) => {
                                stop = StopReason::Timeout;
                                let _ = kill_tree(&mut child).await;
                            }
                        }
                        break;
                    }
                    Some((which, bytes)) => {
                        let (buf, trunc) = match which {
                            Stream::Out => (&mut stdout_buf, &mut out_trunc),
                            Stream::Err => (&mut stderr_buf, &mut err_trunc),
                        };
                        match req.max_bytes {
                            None => buf.extend_from_slice(&bytes),
                            Some(cap) => {
                                if used >= cap {
                                    // Already over budget: keep draining, record nothing.
                                    *trunc = true;
                                    limit_hit = true;
                                } else {
                                    let room = cap - used;
                                    if bytes.len() <= room {
                                        used += bytes.len();
                                        buf.extend_from_slice(&bytes);
                                    } else {
                                        buf.extend_from_slice(&bytes[..room]);
                                        used = cap;
                                        *trunc = true;
                                        limit_hit = true;
                                    }
                                }
                                if limit_hit && req.on_output_limit == OnOutputLimit::Kill {
                                    stop = StopReason::OutputLimit;
                                    let _ = kill_tree(&mut child).await;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // A capture limit is reported even when the process exited cleanly: the exit code stays
    // meaningful, so both facts survive.
    if limit_hit && stop == StopReason::Exited {
        stop = StopReason::OutputLimit;
    }

    Ok(Some(ExecOutcome {
        stop_reason: stop,
        exit_code: if stop == StopReason::Timeout {
            None
        } else {
            exit_code
        },
        stdout: String::from_utf8_lossy(&stdout_buf).into_owned(),
        stderr: String::from_utf8_lossy(&stderr_buf).into_owned(),
        stdout_truncated: out_trunc,
        stderr_truncated: err_trunc,
        duration_ms: started.elapsed().as_millis() as u64,
        containment: containment(),
    }))
}

async fn pump<R>(reader: &mut R, which: Stream, tx: mpsc::Sender<(Stream, Vec<u8>)>)
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut buf = [0u8; 8192];
    loop {
        match reader.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if tx.send((which, buf[..n].to_vec())).await.is_err() {
                    break;
                }
            }
        }
    }
}

/// Kill the whole process tree. On Unix this signals the child's own process group; on
/// Windows the Job Object handle does it when dropped.
async fn kill_tree(child: &mut tokio::process::Child) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        if let Some(pid) = child.id() {
            super::platform::terminate_group_unix(pid as i32).await;
        }
    }
    let _ = child.start_kill();
    let _ = child.wait().await;
    Ok(())
}

// Cancellation uses `tokio_util::sync::CancellationToken` — the same type rmcp puts in
// `RequestContext::ct`. A hand-rolled "check an AtomicBool, then await a Notify" signal has
// a lost-wakeup window: a cancel landing between the load and the waiter's registration is
// missed by `notify_waiters`, and the call then stays parked until its timeout. Reusing the
// token also means the request's own signal reaches the executor with nothing in between.
