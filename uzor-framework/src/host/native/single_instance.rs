//! Cross-process single-instance guard (moved from `uzor-desktop`).
//!
//! On Windows, uses a named Win32 mutex. On other platforms this is a no-op
//! that always succeeds.

/// RAII guard that holds the Win32 named mutex for the process lifetime.
pub struct SingleInstanceGuard {
    #[cfg(target_os = "windows")]
    handle: isize,
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        #[cfg(target_os = "windows")]
        {
            if self.handle != 0 {
                extern "system" {
                    fn CloseHandle(h_object: isize) -> i32;
                }
                unsafe {
                    CloseHandle(self.handle);
                }
            }
        }
    }
}

/// Acquire the single-instance guard for `mutex_name`.
///
/// If another process already holds the mutex:
/// - when `--wait-pid <pid>` is present in `std::env::args()`, wait for that
///   pid to exit, then retry once;
/// - otherwise exit the process with status 0.
pub fn single_instance(mutex_name: &str) -> SingleInstanceGuard {
    #[cfg(target_os = "windows")]
    {
        windows_single_instance(mutex_name)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = mutex_name;
        SingleInstanceGuard {}
    }
}

#[cfg(target_os = "windows")]
fn windows_single_instance(mutex_name: &str) -> SingleInstanceGuard {
    extern "system" {
        fn CreateMutexW(
            lp_mutex_attributes: *const std::ffi::c_void,
            b_initial_owner: i32,
            lp_name: *const u16,
        ) -> isize;
        fn GetLastError() -> u32;
        fn OpenProcess(dw_desired_access: u32, b_inherit_handle: i32, dw_process_id: u32) -> isize;
        fn WaitForSingleObject(h_handle: isize, dw_milliseconds: u32) -> u32;
        fn CloseHandle(h_object: isize) -> i32;
    }

    const ERROR_ALREADY_EXISTS: u32 = 183;
    const SYNCHRONIZE: u32 = 0x0010_0000;
    const INFINITE: u32 = 0xFFFF_FFFF;

    let wait_pid: Option<u32> = {
        let args: Vec<String> = std::env::args().collect();
        args.windows(2)
            .find(|w| w[0] == "--wait-pid")
            .and_then(|w| w[1].parse::<u32>().ok())
    };

    let name_utf16: Vec<u16> = {
        let mut v: Vec<u16> = mutex_name.encode_utf16().collect();
        v.push(0);
        v
    };

    let try_acquire = || -> (isize, bool) {
        let handle = unsafe { CreateMutexW(std::ptr::null(), 1, name_utf16.as_ptr()) };
        let already_exists = handle != 0 && unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
        (handle, already_exists)
    };

    let (handle, already_exists) = try_acquire();

    if already_exists {
        if let Some(pid) = wait_pid {
            log::warn!("single-instance: waiting for pid {pid} to exit");
            let proc_handle = unsafe { OpenProcess(SYNCHRONIZE, 0, pid) };
            if proc_handle != 0 {
                unsafe {
                    WaitForSingleObject(proc_handle, INFINITE);
                    CloseHandle(proc_handle);
                }
            }
            if handle != 0 {
                unsafe {
                    CloseHandle(handle);
                }
            }
            let (handle2, already_exists2) = try_acquire();
            if already_exists2 {
                log::warn!("single-instance: still running after wait — exiting");
                std::process::exit(0);
            }
            if handle2 == 0 {
                log::warn!(
                    "single-instance: CreateMutexW failed ({}), continuing without guard",
                    unsafe { GetLastError() }
                );
            }
            return SingleInstanceGuard { handle: handle2 };
        } else {
            log::warn!("single-instance: another instance is already running — exiting");
            std::process::exit(0);
        }
    }

    if handle == 0 {
        log::warn!(
            "single-instance: CreateMutexW failed ({}), continuing without guard",
            unsafe { GetLastError() }
        );
    }

    SingleInstanceGuard { handle }
}
