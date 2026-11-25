//! Windows bits: own CPU time / working set, single-instance guard.

/// (CPU seconds used by this process, working set bytes).
#[cfg(windows)]
pub fn process_times() -> (f64, u64) {
    use windows::Win32::Foundation::FILETIME;
    use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
    unsafe {
        let h = GetCurrentProcess();
        let (mut c, mut e, mut k, mut u) = (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
        let ft = |f: FILETIME| ((f.dwHighDateTime as u64) << 32 | f.dwLowDateTime as u64) as f64 / 1e7;
        let cpu = if GetProcessTimes(h, &mut c, &mut e, &mut k, &mut u).is_ok() { ft(k) + ft(u) } else { 0.0 };
        let mut m = PROCESS_MEMORY_COUNTERS { cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32, ..Default::default() };
        let ws = if GetProcessMemoryInfo(h, &mut m, m.cb).is_ok() { m.WorkingSetSize as u64 } else { 0 };
        (cpu, ws)
    }
}

#[cfg(not(windows))]
pub fn process_times() -> (f64, u64) {
    (0.0, 0)
}

/// Drop cold pages (start-up code, DLL init, one-off WinRT/PDH set-up) from the working set.
/// The hot set of an active panel is ~8–9 MB; pages that are needed again come back as soft faults.
#[cfg(windows)]
pub fn trim_working_set() {
    use windows::Win32::System::ProcessStatus::EmptyWorkingSet;
    use windows::Win32::System::Threading::GetCurrentProcess;
    unsafe {
        let _ = EmptyWorkingSet(GetCurrentProcess());
    }
}

#[cfg(not(windows))]
pub fn trim_working_set() {}

/// Named mutex per port: a second agent on the same port exits. The handle lives until the process ends.
#[cfg(windows)]
pub fn single_instance(port: u16) -> bool {
    use windows::core::HSTRING;
    use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows::Win32::System::Threading::CreateMutexW;
    let name = HSTRING::from(format!("Local\\Ven4Docking-agent-{port}"));
    unsafe {
        match CreateMutexW(None, false, &name) {
            Ok(h) => {
                let dup = GetLastError() == ERROR_ALREADY_EXISTS;
                let _ = h; // HANDLE is Copy; never closed, lives until exit
                !dup
            }
            Err(_) => true,
        }
    }
}

#[cfg(not(windows))]
pub fn single_instance(_port: u16) -> bool {
    true
}
