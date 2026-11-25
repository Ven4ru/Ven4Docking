//! Windows Firewall rules «Ven4Docking» for the agent program.
//!
//! `ven4docking-agent.exe --firewall add|remove` (run elevated by the installer and the editor, so UAC
//! shows the agent's own name instead of "Windows Command Processor") changes the rules with netsh.
//! The read-only check for `GET /admin/status` uses COM `HNetCfg.FwPolicy2` (INetFwPolicy2): it works
//! without administrator rights, unlike Get-NetFirewallRule.

use std::path::Path;

pub const RULE_NAME: &str = "Ven4Docking";
/// Fixed like in the installer: the rules are made before any config exists (and possibly by another
/// administrator account), so they cannot follow the port setting.
pub const TCP_PORT: u16 = 8765;
pub const UDP_PORT: u16 = 8766;

const PROTO_TCP: i32 = 6;
const PROTO_UDP: i32 = 17;

/// What a rule looks like, only the fields the check needs.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleInfo {
    pub name: String,
    pub app: String,
    pub allow: bool,
    pub inbound: bool,
    pub enabled: bool,
    /// 6 = TCP, 17 = UDP, 256 = any
    pub protocol: i32,
}

/// Same file name, ignoring case (Windows paths; Unicode user names included).
pub fn same_program(a: &str, b: &str) -> bool {
    !a.is_empty() && a.to_lowercase() == b.to_lowercase()
}

/// `"ok"`: enabled inbound allow rules «Ven4Docking» for the program, one TCP and one UDP, and no block rule;
/// `"blocked"`: a block rule for the program exists (Windows makes them after «Allow access?» is cancelled);
/// `"missing"`: no allow rules (or only one of the two).
pub fn classify(rules: &[RuleInfo], program: &str) -> &'static str {
    let mine: Vec<&RuleInfo> = rules.iter().filter(|r| same_program(&r.app, program)).collect();
    if mine.iter().any(|r| !r.allow) {
        return "blocked";
    }
    let allowed = |proto: i32| mine.iter().any(|r| r.allow && r.inbound && r.enabled && r.name == RULE_NAME && r.protocol == proto);
    if allowed(PROTO_TCP) && allowed(PROTO_UDP) {
        "ok"
    } else {
        "missing"
    }
}

/// Full path of this exe (what the rules are bound to).
pub fn own_program() -> Option<String> {
    std::env::current_exe().ok().map(|p| p.display().to_string())
}

/// State for /admin/status: `ok | missing | blocked | unknown`.
pub fn check_self() -> &'static str {
    match own_program() {
        Some(p) => check(&p),
        None => "unknown",
    }
}

pub fn check(program: &str) -> &'static str {
    match read_rules(program) {
        Ok(rules) => classify(&rules, program),
        Err(e) => {
            tracing::warn!("firewall check: {e}");
            "unknown"
        }
    }
}

/// `--firewall add`: delete rules named «Ven4Docking» and every rule for this program (Windows' own
/// block rules too), then add the two allow rules. `--firewall remove`: only delete. Needs elevation.
pub fn run_cli(action: &str) -> i32 {
    let Some(program) = own_program() else {
        eprintln!("cannot get own path");
        return 1;
    };
    let result = match action {
        "add" => add(&program),
        "remove" => remove(&program),
        _ => {
            eprintln!("usage: ven4docking-agent.exe --firewall add|remove");
            return 2;
        }
    };
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("firewall {action}: {e}");
            1
        }
    }
}

fn delete_all(program: &str) {
    // "no rules match" is not an error here
    let _ = netsh(&format!("advfirewall firewall delete rule name=\"{RULE_NAME}\""));
    let _ = netsh(&format!("advfirewall firewall delete rule name=all program=\"{program}\""));
}

pub fn add(program: &str) -> Result<(), String> {
    check_path(program)?;
    delete_all(program);
    for (proto, port) in [("TCP", TCP_PORT), ("UDP", UDP_PORT)] {
        netsh(&format!(
            "advfirewall firewall add rule name=\"{RULE_NAME}\" dir=in action=allow program=\"{program}\" protocol={proto} localport={port} remoteip=localsubnet profile=any"
        ))?;
    }
    match check(program) {
        "ok" => Ok(()),
        s => Err(format!("rules added, but the check says {s}")),
    }
}

pub fn remove(program: &str) -> Result<(), String> {
    check_path(program)?;
    delete_all(program);
    let left = read_rules(program).map_err(|e| e.to_string())?;
    if left.iter().any(|r| r.name == RULE_NAME || same_program(&r.app, program)) {
        return Err("some rules are still there".into());
    }
    Ok(())
}

/// netsh takes the whole line itself; a Windows path cannot contain `"`, so quoting is safe.
fn check_path(program: &str) -> Result<(), String> {
    if program.contains('"') || !Path::new(program).is_absolute() {
        return Err(format!("bad program path: {program}"));
    }
    Ok(())
}

#[cfg(windows)]
fn netsh(args: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let sys = std::env::var_os("SystemRoot").map(std::path::PathBuf::from).unwrap_or_else(|| "C:\\Windows".into());
    let status = std::process::Command::new(sys.join("System32").join("netsh.exe"))
        .raw_arg(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .status()
        .map_err(|e| format!("netsh: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("netsh {args}: exit {}", status.code().unwrap_or(-1)))
    }
}

#[cfg(not(windows))]
fn netsh(_args: &str) -> Result<(), String> {
    Err("not supported".into())
}

/// Rules for `program` plus rules named «Ven4Docking», read through INetFwPolicy2 (no elevation needed).
#[cfg(windows)]
pub fn read_rules(program: &str) -> windows::core::Result<Vec<RuleInfo>> {
    use windows::core::Interface;
    use windows::Win32::NetworkManagement::WindowsFirewall::{INetFwPolicy2, INetFwRule, NetFwPolicy2, NET_FW_ACTION_ALLOW, NET_FW_RULE_DIR_IN};
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, IDispatch, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
    use windows::Win32::System::Ole::IEnumVARIANT;
    use windows::Win32::System::Variant::VARIANT;

    struct ComGuard(bool);
    impl Drop for ComGuard {
        fn drop(&mut self) {
            if self.0 {
                unsafe { CoUninitialize() };
            }
        }
    }
    const BATCH: usize = 64;
    unsafe {
        // S_OK / S_FALSE need a matching CoUninitialize; RPC_E_CHANGED_MODE: the thread already has COM
        let _com = ComGuard(CoInitializeEx(None, COINIT_MULTITHREADED).is_ok());
        let policy: INetFwPolicy2 = CoCreateInstance(&NetFwPolicy2, None, CLSCTX_INPROC_SERVER)?;
        let en: IEnumVARIANT = policy.Rules()?._NewEnum()?.cast()?;
        let mut out = Vec::new();
        loop {
            let mut batch: Vec<VARIANT> = (0..BATCH).map(|_| VARIANT::default()).collect();
            let mut fetched = 0u32;
            en.Next(&mut batch, &mut fetched).ok()?;
            for v in batch.iter().take(fetched as usize) {
                let Ok(rule) = IDispatch::try_from(v).and_then(|d| d.cast::<INetFwRule>()) else { continue };
                let name = rule.Name().map(|b| b.to_string()).unwrap_or_default();
                let app = rule.ApplicationName().map(|b| b.to_string()).unwrap_or_default();
                if name != RULE_NAME && !same_program(&app, program) {
                    continue;
                }
                out.push(RuleInfo {
                    name,
                    app,
                    allow: rule.Action().map(|a| a == NET_FW_ACTION_ALLOW).unwrap_or(false),
                    inbound: rule.Direction().map(|d| d == NET_FW_RULE_DIR_IN).unwrap_or(false),
                    enabled: rule.Enabled().map(|b| b.as_bool()).unwrap_or(false),
                    protocol: rule.Protocol().unwrap_or(0),
                });
            }
            if (fetched as usize) < BATCH {
                break;
            }
        }
        Ok(out)
    }
}

#[cfg(not(windows))]
pub fn read_rules(_program: &str) -> Result<Vec<RuleInfo>, String> {
    Err("not supported".into())
}
