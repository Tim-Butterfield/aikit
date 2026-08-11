//! The per-instance temp directory that holds generated scripts.
//!
//! The script is **ephemeral, not non-persistent**: it exists on disk while it runs. The OS
//! holds a share lock on an executing `.ps1`/`.cmd`, so deletion is deferred until the
//! process tree exits, and it can still fail while a scanner holds a handle. Deletion is
//! therefore best-effort with retries, and the instance directory is the real backstop.
//!
//! Instance directories are named with a random nonce rather than a PID: PIDs are reused,
//! so a PID-named directory can collide with a live sibling's — and a client may run more
//! than one server process at a time.
//!
//! A lock file inside each directory marks it live. Sweeping *acquires* the lock rather
//! than deleting first, so a directory belonging to a running instance is never removed.

use std::io::Write;
use std::path::{Path, PathBuf};

const INSTANCE_PREFIX: &str = "aikit-mcp-";
const LOCK_NAME: &str = ".lock";

/// A directory owned by this server process for the lifetime of the run.
pub struct InstanceDir {
    path: PathBuf,
    _lock: std::fs::File,
}

impl InstanceDir {
    /// Create a fresh instance directory under the OS temp dir and take its lock.
    pub fn create() -> std::io::Result<Self> {
        let base = std::env::temp_dir();
        let mut last_err = None;
        for _ in 0..8 {
            let dir = base.join(format!("{INSTANCE_PREFIX}{}", nonce()));
            match create_dir_private(&dir) {
                Ok(()) => {
                    let lock = std::fs::OpenOptions::new()
                        .create_new(true)
                        .write(true)
                        .open(dir.join(LOCK_NAME))?;
                    // Creating the file is not enough: liveness is decided by whether the
                    // lock can be TAKEN, so it must actually be held. Without this the
                    // lock is decorative and a sweep would reclaim a live directory.
                    hold_lock(&lock)?;
                    return Ok(InstanceDir {
                        path: dir,
                        _lock: lock,
                    });
                }
                Err(e) => last_err = Some(e),
            }
        }
        Err(last_err.unwrap_or_else(|| std::io::Error::other("could not create instance dir")))
    }

    /// Write a script and return its path. The extension comes from the runner, the line
    /// endings and BOM from the runner's conventions, and `epilogue` — when the runner needs
    /// one — is appended so the interpreter's exit code reflects the last native command.
    ///
    /// The epilogue is appended after a newline, so a body without a trailing newline cannot
    /// splice it onto the final statement.
    pub fn write_script(
        &self,
        runner: &str,
        body: &str,
        extension: &str,
        crlf: bool,
        bom: bool,
        epilogue: Option<&str>,
    ) -> std::io::Result<PathBuf> {
        let name = format!("{}{}", nonce(), extension);
        let path = self.path.join(name);
        let mut file = create_file_private(&path)?;

        if bom {
            file.write_all(&[0xEF, 0xBB, 0xBF])?;
        }

        let mut source = body.to_string();
        if let Some(tail) = epilogue {
            if !source.ends_with('\n') {
                source.push('\n');
            }
            source.push_str(tail);
            source.push('\n');
        }

        let text = normalize_newlines(&source, crlf);
        file.write_all(text.as_bytes())?;
        file.flush()?;
        let _ = runner; // extension/crlf/bom/epilogue already encode the runner's conventions
        Ok(path)
    }
}

impl Drop for InstanceDir {
    fn drop(&mut self) {
        // Best effort: a scanner or a surviving grandchild may still hold a handle.
        for attempt in 0..5 {
            if std::fs::remove_dir_all(&self.path).is_ok() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(50 * (attempt + 1)));
        }
    }
}

/// Normalize every newline form, then apply the target convention.
///
/// Converting bare `\n` to `\r\n` without normalizing first turns an existing `\r\n` into
/// `\r\r\n`, which corrupts here-strings and any hashed content in the script.
pub fn normalize_newlines(text: &str, crlf: bool) -> String {
    let unified = text.replace("\r\n", "\n").replace('\r', "\n");
    if crlf {
        unified.replace('\n', "\r\n")
    } else {
        unified
    }
}

/// A directory younger than this is never swept, whatever its lock says.
///
/// Creating the directory and taking its lock cannot be one atomic step, so there is a
/// window in which a live instance's directory has no lock file yet. Without this guard a
/// second server starting in that window sees "no lock" and deletes a directory that is
/// about to be used — which shows up as a script vanishing mid-run.
const CREATION_GRACE: std::time::Duration = std::time::Duration::from_secs(60);

/// Remove instance directories left behind by dead servers.
///
/// Liveness is decided by *acquiring the lock*, never by inspecting a PID: PIDs are reused,
/// so a PID-named directory can belong to an unrelated live process. A directory whose lock
/// cannot be taken belongs to a running instance and is left alone.
pub fn sweep_orphans() {
    let base = std::env::temp_dir();
    let Ok(entries) = std::fs::read_dir(&base) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let is_ours = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with(INSTANCE_PREFIX));
        if !is_ours || is_recent(&path) {
            continue;
        }
        if can_acquire_lock(&path.join(LOCK_NAME)) {
            let _ = std::fs::remove_dir_all(&path);
        }
    }
}

/// Whether a directory was created or touched inside the creation grace period.
fn is_recent(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        // Unreadable: leave it alone rather than guess.
        return true;
    };
    let stamp = meta.created().or_else(|_| meta.modified());
    match stamp {
        Ok(t) => t.elapsed().map(|age| age < CREATION_GRACE).unwrap_or(true),
        Err(_) => true,
    }
}

/// Take an exclusive lock on the instance's lock file and keep it for the file's lifetime.
///
/// The kernel releases it when the process exits, however it exits, which is what makes a
/// crashed instance's directory reclaimable while a live one's is not.
fn hold_lock(file: &std::fs::File) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        // Safety: flock on a valid, owned descriptor.
        if unsafe { flock_raw(file.as_raw_fd(), LOCK_EX | LOCK_NB) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    #[cfg(windows)]
    {
        // On Windows the exclusive open in `create` already denies sharing, so holding the
        // handle is the lock.
        let _ = file;
    }
    Ok(())
}

#[cfg(unix)]
const LOCK_EX: i32 = 2;
#[cfg(unix)]
const LOCK_NB: i32 = 4;
#[cfg(unix)]
const LOCK_UN: i32 = 8;

#[cfg(unix)]
extern "C" {
    #[link_name = "flock"]
    fn flock_raw(fd: i32, op: i32) -> i32;
}

/// Whether the lock file can be taken, i.e. no live instance owns the directory.
fn can_acquire_lock(lock: &Path) -> bool {
    if !lock.exists() {
        // No lock at all: a crashed instance from before the lock was written.
        return true;
    }
    #[cfg(unix)]
    {
        // An advisory lock is released when the owning process dies, so a successful
        // non-blocking exclusive lock means nobody is using it.
        use std::os::unix::io::AsRawFd;
        let Ok(file) = std::fs::OpenOptions::new().write(true).open(lock) else {
            return false;
        };
        unsafe {
            if flock_raw(file.as_raw_fd(), LOCK_EX | LOCK_NB) == 0 {
                flock_raw(file.as_raw_fd(), LOCK_UN);
                return true;
            }
        }
        false
    }
    #[cfg(windows)]
    {
        // Windows keeps a mandatory lock on an open file: an exclusive open succeeds only
        // when no live instance holds it.
        std::fs::OpenOptions::new()
            .write(true)
            .share_mode(0)
            .open(lock)
            .is_ok()
    }
}

/// Create a directory that is private to the current user *from the moment it exists*.
///
/// Creating it and then narrowing the mode leaves a window in which the directory is
/// world-readable, which matters because this one is created in a world-writable location
/// (the shared system temp directory) and is about to hold a script. On Unix the mode is
/// therefore passed to `mkdir` itself. Elsewhere the per-user temp directory is the only
/// protection there is — see `create_file_private`.
fn create_dir_private(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new().mode(0o700).create(path)
    }
    #[cfg(not(unix))]
    {
        std::fs::create_dir(path)
    }
}

/// Create a file that is private to the current user from the moment it exists.
///
/// `create_new` rather than `create`: the name carries a nonce, so an existing file would
/// mean a collision worth failing on rather than a file worth truncating.
///
/// On Windows no ACL is applied. The containing directory sits under the per-user temp
/// location, which is what limits access there; that is a weaker guarantee than the Unix
/// mode, and it is stated rather than implied wherever this behaviour is documented.
fn create_file_private(path: &Path) -> std::io::Result<std::fs::File> {
    let mut opts = std::fs::OpenOptions::new();
    opts.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    opts.open(path)
}

/// A random-enough nonce for a filesystem name. Uniqueness is enforced by `create_new`, so
/// this only needs to avoid collisions, not be cryptographic.
fn nonce() -> String {
    use std::hash::{BuildHasher, Hasher, RandomState};
    let mut h = RandomState::new().build_hasher();
    h.write_usize(std::process::id() as usize);
    h.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default(),
    );
    format!("{:016x}", h.finish())
}

#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crlf_conversion_is_idempotent() {
        // Blind \n -> \r\n on text that already has CRLF produces CRCRLF and corrupts
        // here-strings; normalizing first is what prevents it.
        assert_eq!(normalize_newlines("a\r\nb", true), "a\r\nb");
        assert_eq!(normalize_newlines("a\nb", true), "a\r\nb");
        assert_eq!(normalize_newlines("a\rb", true), "a\r\nb");
        assert_eq!(normalize_newlines("a\r\nb", false), "a\nb");
    }

    #[test]
    fn instance_dir_is_created_and_removed() {
        let dir_path = {
            let dir = InstanceDir::create().expect("create instance dir");
            let script = dir
                .write_script("sh", "echo hi\n", ".sh", false, false, None)
                .expect("write script");
            assert!(script.is_file());
            let parent = script.parent().expect("script has a parent").to_path_buf();
            assert!(parent.is_dir());
            parent
        };
        assert!(
            !dir_path.exists(),
            "instance dir should be removed when the guard drops"
        );
    }

    #[test]
    fn each_instance_dir_is_distinct() {
        // PID-named directories collide when a PID is reused, and a client may run more
        // than one server process at once.
        let a = InstanceDir::create().expect("first");
        let b = InstanceDir::create().expect("second");
        let pa = a.write_script("sh", "", ".sh", false, false, None).unwrap();
        let pb = b.write_script("sh", "", ".sh", false, false, None).unwrap();
        assert_ne!(pa.parent().unwrap(), pb.parent().unwrap());
    }

    #[test]
    fn bom_and_crlf_are_written_per_runner_convention() {
        let dir = InstanceDir::create().expect("dir");
        let ps = dir
            .write_script("powershell", "echo hi\n", ".ps1", true, true, None)
            .expect("ps1");
        let bytes = std::fs::read(&ps).expect("read ps1");
        assert_eq!(&bytes[..3], &[0xEF, 0xBB, 0xBF], "5.1 needs a UTF-8 BOM");
        assert!(bytes.ends_with(b"\r\n"));

        // cmd.exe parses a BOM as part of the first command, so it must never get one.
        let cmd = dir
            .write_script("cmd", "echo hi\n", ".cmd", true, false, None)
            .expect("cmd");
        let bytes = std::fs::read(&cmd).expect("read cmd");
        assert_ne!(&bytes[..3.min(bytes.len())], &[0xEF, 0xBB, 0xBF]);
    }

    #[test]
    fn the_exit_code_epilogue_is_appended_on_its_own_line() {
        // Measured on PowerShell 5.1: a script ending in a failing native command exits 0
        // through `-File`. The epilogue recovers the real code — but only if it lands as a
        // statement of its own, so a body with no trailing newline must not splice it onto
        // the final line.
        let dir = InstanceDir::create().expect("dir");

        let p = dir
            .write_script(
                "powershell",
                "cmd /d /c \"exit 7\"", // deliberately no trailing newline
                ".ps1",
                true,
                true,
                Some("exit $LASTEXITCODE"),
            )
            .expect("ps1");
        let text = std::fs::read_to_string(&p).expect("read");
        assert!(
            text.contains("\"exit 7\"\r\nexit $LASTEXITCODE"),
            "the epilogue must start a new line; got {text:?}"
        );

        // POSIX shells already exit with the last command's status, so nothing is appended
        // and the body is byte-identical to what the caller sent.
        let p = dir
            .write_script("sh", "echo hi\n", ".sh", false, false, None)
            .expect("sh");
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "echo hi\n");
    }
}
