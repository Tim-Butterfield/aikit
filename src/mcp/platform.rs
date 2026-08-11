//! Platform-specific process-tree control.
//!
//! Unix: the child is placed in its own process group before exec, so the whole tree can be
//! signalled with `killpg` without the server signalling itself.
//!
//! Windows: the child is created suspended, assigned to a kill-on-close Job Object, then
//! resumed. Without the suspend/assign/resume sequence a child can spawn a grandchild
//! before assignment and escape the job. Assignment can also fail when the server is
//! already inside a job without breakaway — that failure is reported rather than ignored,
//! because silently continuing leaks orphans on every timeout.

#[cfg(unix)]
pub fn pre_spawn_unix(cmd: &mut tokio::process::Command) {
    // Safety: setpgid only touches the child's process group between fork and exec.
    unsafe {
        cmd.pre_exec(|| {
            if libc_setpgid() != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

#[cfg(unix)]
fn libc_setpgid() -> i32 {
    // setpgid(0, 0): the child becomes the leader of a new group whose id is its pid.
    unsafe { setpgid(0, 0) }
}

#[cfg(unix)]
extern "C" {
    fn setpgid(pid: i32, pgid: i32) -> i32;
    fn killpg(pgrp: i32, sig: i32) -> i32;
}

/// Terminate a process group: SIGTERM, a short grace period, then SIGKILL.
#[cfg(unix)]
pub async fn terminate_group_unix(pid: i32) {
    const SIGTERM: i32 = 15;
    const SIGKILL: i32 = 9;
    unsafe {
        killpg(pid, SIGTERM);
    }
    tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
    unsafe {
        killpg(pid, SIGKILL);
    }
}

#[cfg(windows)]
pub fn pre_spawn_windows(cmd: &mut tokio::process::Command) {
    const CREATE_SUSPENDED: u32 = 0x0000_0004;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    cmd.creation_flags(CREATE_SUSPENDED | CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);
}

/// A Job Object owning the child's tree. Dropping it terminates every process in the job.
#[cfg(windows)]
pub struct JobHandle(windows_raw::Handle);

#[cfg(windows)]
impl Drop for JobHandle {
    fn drop(&mut self) {
        windows_raw::close_handle(self.0);
    }
}

/// Assign the (suspended) child to a kill-on-close job, then resume its main thread.
///
/// The order matters: assigning after resume races the child's own spawns.
#[cfg(windows)]
pub fn assign_job_and_resume(child: &tokio::process::Child) -> std::io::Result<JobHandle> {
    let pid = child
        .id()
        .ok_or_else(|| std::io::Error::other("child already reaped"))?;
    let job = windows_raw::create_kill_on_close_job()?;
    // Own the handle immediately: every failure below must close it, and a kill-on-close
    // job that leaks would keep its processes alive with nothing left to terminate them.
    let owned = JobHandle(job);
    windows_raw::assign_process_to_job(job, pid)?;
    windows_raw::resume_process_main_thread(pid)?;
    Ok(owned)
}

#[cfg(windows)]
mod windows_raw {
    //! Thin FFI over the Win32 calls the job-object sequence needs. Kept minimal and local
    //! rather than pulling in a wide dependency for four functions.

    pub type Handle = isize;

    const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION: i32 = 9;
    const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x0000_2000;
    const THREAD_SUSPEND_RESUME: u32 = 0x0002;

    #[repr(C)]
    #[derive(Default)]
    struct IoCounters {
        read_operation_count: u64,
        write_operation_count: u64,
        other_operation_count: u64,
        read_transfer_count: u64,
        write_transfer_count: u64,
        other_transfer_count: u64,
    }

    #[repr(C)]
    #[derive(Default)]
    struct BasicLimitInformation {
        per_process_user_time_limit: i64,
        per_job_user_time_limit: i64,
        limit_flags: u32,
        minimum_working_set_size: usize,
        maximum_working_set_size: usize,
        active_process_limit: u32,
        affinity: usize,
        priority_class: u32,
        scheduling_class: u32,
    }

    #[repr(C)]
    #[derive(Default)]
    struct ExtendedLimitInformation {
        basic_limit_information: BasicLimitInformation,
        io_info: IoCounters,
        process_memory_limit: usize,
        job_memory_limit: usize,
        peak_process_memory_used: usize,
        peak_job_memory_used: usize,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateJobObjectW(attrs: *mut core::ffi::c_void, name: *const u16) -> Handle;
        fn SetInformationJobObject(
            job: Handle,
            class: i32,
            info: *mut core::ffi::c_void,
            len: u32,
        ) -> i32;
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
        fn AssignProcessToJobObject(job: Handle, process: Handle) -> i32;
        fn CloseHandle(h: Handle) -> i32;
        fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> Handle;
        fn Thread32First(snap: Handle, entry: *mut ThreadEntry32) -> i32;
        fn Thread32Next(snap: Handle, entry: *mut ThreadEntry32) -> i32;
        fn OpenThread(access: u32, inherit: i32, tid: u32) -> Handle;
        fn ResumeThread(thread: Handle) -> u32;
    }

    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct ThreadEntry32 {
        dw_size: u32,
        cnt_usage: u32,
        th32_thread_id: u32,
        th32_owner_process_id: u32,
        tp_base_pri: i32,
        tp_delta_pri: i32,
        dw_flags: u32,
    }

    pub fn close_handle(h: Handle) {
        unsafe {
            CloseHandle(h);
        }
    }

    pub fn create_kill_on_close_job() -> std::io::Result<Handle> {
        unsafe {
            let job = CreateJobObjectW(core::ptr::null_mut(), core::ptr::null());
            if job == 0 {
                return Err(std::io::Error::last_os_error());
            }
            let mut info = ExtendedLimitInformation::default();
            info.basic_limit_information.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let ok = SetInformationJobObject(
                job,
                JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
                &mut info as *mut _ as *mut core::ffi::c_void,
                core::mem::size_of::<ExtendedLimitInformation>() as u32,
            );
            if ok == 0 {
                let e = std::io::Error::last_os_error();
                CloseHandle(job);
                return Err(e);
            }
            Ok(job)
        }
    }

    pub fn assign_process_to_job(job: Handle, pid: u32) -> std::io::Result<()> {
        const PROCESS_SET_QUOTA: u32 = 0x0100;
        const PROCESS_TERMINATE: u32 = 0x0001;
        unsafe {
            let proc = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid);
            if proc == 0 {
                return Err(std::io::Error::last_os_error());
            }
            let ok = AssignProcessToJobObject(job, proc);
            CloseHandle(proc);
            if ok == 0 {
                // Commonly ERROR_ACCESS_DENIED when this process is already inside a job
                // without breakaway. Reported, never ignored: silently continuing would
                // leak the whole tree on every timeout.
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        }
    }

    /// Resume every thread of the (suspended) process. A freshly created suspended process
    /// has exactly one, but enumerating is correct and costs nothing here.
    pub fn resume_process_main_thread(pid: u32) -> std::io::Result<()> {
        const TH32CS_SNAPTHREAD: u32 = 0x0000_0004;
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
            if snap == -1 || snap == 0 {
                return Err(std::io::Error::last_os_error());
            }
            let mut entry = ThreadEntry32 {
                dw_size: core::mem::size_of::<ThreadEntry32>() as u32,
                ..Default::default()
            };
            let mut found = false;
            let mut ok = Thread32First(snap, &mut entry);
            while ok != 0 {
                if entry.th32_owner_process_id == pid {
                    let t = OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32_thread_id);
                    if t != 0 {
                        ResumeThread(t);
                        CloseHandle(t);
                        found = true;
                    }
                }
                entry.dw_size = core::mem::size_of::<ThreadEntry32>() as u32;
                ok = Thread32Next(snap, &mut entry);
            }
            CloseHandle(snap);
            if !found {
                return Err(std::io::Error::other(
                    "no thread found to resume for the suspended child",
                ));
            }
            Ok(())
        }
    }
}
