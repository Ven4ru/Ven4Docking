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

/// COM for the current thread (MTA). S_OK / S_FALSE are balanced by CoUninitialize on drop;
/// RPC_E_CHANGED_MODE (the thread already has COM) is left alone. The sampler keeps one for its whole life.
#[cfg(windows)]
pub struct Com(bool);

#[cfg(windows)]
impl Com {
    pub fn mta() -> Com {
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
        Com(unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok())
    }
}

#[cfg(windows)]
impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            unsafe { windows::Win32::System::Com::CoUninitialize() };
        }
    }
}

/// Lower-case exe names of all running processes (Toolhelp snapshot; nothing is kept between calls).
#[cfg(windows)]
pub fn process_names() -> std::collections::HashSet<String> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS};
    let mut out = std::collections::HashSet::new();
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return out };
        let mut e = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut ok = Process32FirstW(snap, &mut e).is_ok();
        while ok {
            let len = e.szExeFile.iter().position(|c| *c == 0).unwrap_or(e.szExeFile.len());
            out.insert(String::from_utf16_lossy(&e.szExeFile[..len]).to_lowercase());
            ok = Process32NextW(snap, &mut e).is_ok();
        }
        let _ = CloseHandle(snap);
    }
    out
}

#[cfg(not(windows))]
pub fn process_names() -> std::collections::HashSet<String> {
    std::collections::HashSet::new()
}

/// Full path of a process image (None without access, e.g. a protected process).
#[cfg(windows)]
pub fn exe_path(pid: u32) -> Option<String> {
    use windows::core::PWSTR;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION};
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let r = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(h);
        r.ok()?;
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }
}

#[cfg(not(windows))]
pub fn exe_path(_pid: u32) -> Option<String> {
    None
}

/// "C:\Program Files\obs-studio\bin\64bit\obs64.exe" -> "obs64.exe".
pub fn file_name(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

/// Programs with a visible top-level window: (exe name, window title), one entry per exe, sorted by name.
#[cfg(windows)]
pub fn windowed_processes() -> Vec<(String, String)> {
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindow, GetWindowLongW, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible, GWL_EXSTYLE,
        GW_OWNER, WS_EX_TOOLWINDOW,
    };
    unsafe extern "system" fn collect(hwnd: HWND, lp: LPARAM) -> BOOL {
        let list = &mut *(lp.0 as *mut Vec<(u32, String)>);
        if !IsWindowVisible(hwnd).as_bool() || GetWindow(hwnd, GW_OWNER).is_ok_and(|o| !o.is_invalid()) {
            return BOOL(1);
        }
        if GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0 != 0 {
            return BOOL(1);
        }
        let n = GetWindowTextLengthW(hwnd);
        if n <= 0 {
            return BOOL(1);
        }
        let mut buf = vec![0u16; n as usize + 1];
        let got = GetWindowTextW(hwnd, &mut buf);
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if got > 0 && pid != 0 {
            list.push((pid, String::from_utf16_lossy(&buf[..got as usize])));
        }
        BOOL(1)
    }
    let mut wins: Vec<(u32, String)> = Vec::new();
    unsafe {
        let _ = EnumWindows(Some(collect), LPARAM(&mut wins as *mut _ as isize));
    }
    let me = std::process::id();
    let mut out: Vec<(String, String)> = Vec::new();
    for (pid, title) in wins {
        if pid == me {
            continue;
        }
        let Some(path) = exe_path(pid) else { continue };
        let name = file_name(&path).to_string();
        if !out.iter().any(|(n, _)| n.eq_ignore_ascii_case(&name)) {
            out.push((name, title));
        }
    }
    out.sort_by_key(|(n, _)| n.to_lowercase());
    out
}

#[cfg(not(windows))]
pub fn windowed_processes() -> Vec<(String, String)> {
    Vec::new()
}

/// DPAPI, current user scope, no entropy (as `ProtectedData.Unprotect(data, null, CurrentUser)` in the editor).
#[cfg(windows)]
pub fn dpapi_unprotect(data: &[u8]) -> Result<Vec<u8>, String> {
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB};
    let inp = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
    let mut out = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptUnprotectData(&inp, None, None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut out).map_err(|e| e.to_string())?;
        let v = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(out.pbData as *mut core::ffi::c_void)));
        Ok(v)
    }
}

/// The editor's side of `dpapi_unprotect` (used by tests).
#[cfg(windows)]
pub fn dpapi_protect(data: &[u8]) -> Result<Vec<u8>, String> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB};
    let inp = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
    let mut out = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptProtectData(&inp, PCWSTR::null(), None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut out).map_err(|e| e.to_string())?;
        let v = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(out.pbData as *mut core::ffi::c_void)));
        Ok(v)
    }
}

/// System32 console tool without a window: (exit code, stdout). Used for shutdown.exe and powercfg.
#[cfg(windows)]
pub fn run_hidden(exe: &str, args: &[&str]) -> Result<(i32, String), String> {
    use std::os::windows::process::CommandExt;
    let sys = std::env::var_os("SystemRoot").map(std::path::PathBuf::from).unwrap_or_else(|| std::path::PathBuf::from(r"C:\Windows"));
    let path = sys.join("System32").join(exe);
    let out = std::process::Command::new(&path)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(0x0800_0000)
        .output()
        .map_err(|e| format!("{exe}: {e}"))?;
    Ok((out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned()))
}
