use ven4docking_agent::servers::{explain_ssh_error, remote_command, ProcParser};

const STREAM: &str = include_str!("fixtures/proc_stream.txt");

#[test]
fn first_block_has_no_cpu_second_has_delta() {
    let mut p = ProcParser::default();
    let blocks: Vec<_> = STREAM.lines().filter_map(|l| p.feed(l)).collect();
    assert_eq!(blocks.len(), 2);

    let a = &blocks[0];
    assert_eq!(a.cpu, None, "no previous /proc/stat sample yet");
    assert_eq!(a.cores, 2);
    assert_eq!(a.load, Some([0.13, 0.10, 0.05]));
    assert_eq!(a.uptime, Some(604800.25));
    assert_eq!(a.ram_total, Some(1992188 * 1024));
    assert_eq!(a.ram_used, Some((1992188 - 1513672) * 1024));
    assert_eq!(a.ram_pct, Some(24.0));
    assert_eq!(a.disk_total, Some(30271428 * 1024));
    assert_eq!(a.disk_used, Some(24804900 * 1024));
    assert_eq!(a.disk_pct, Some(86.0));

    // user+100, system+100, idle+800 -> 20 % busy
    let b = &blocks[1];
    assert_eq!(b.cpu, Some(20.0));
    assert_eq!(b.load, Some([0.20, 0.11, 0.05]));
}

#[test]
fn json_shape_is_camel_case() {
    let mut p = ProcParser::default();
    let m = STREAM.lines().filter_map(|l| p.feed(l)).last().unwrap();
    let v = serde_json::to_value(&m).unwrap();
    for k in ["cpu", "cores", "load", "uptime", "ramTotal", "ramUsed", "ramPct", "diskTotal", "diskUsed", "diskPct"] {
        assert!(v.get(k).is_some(), "missing {k}");
    }
}

#[test]
fn garbage_and_partial_blocks_are_tolerated() {
    let mut p = ProcParser::default();
    for l in ["N x", "B", "cpu  a b c", "L nope", "U", "MemTotal: x kB", "D short", "random noise"] {
        assert!(p.feed(l).is_none());
    }
    let m = p.feed("E").expect("block ends even if empty");
    assert_eq!(m.cpu, None);
    assert_eq!(m.cores, 1);
    assert_eq!(m.ram_total, None);
    // CRLF from a Windows-side shell
    assert!(p.feed("B\r").is_none());
    assert!(p.feed("E\r").is_some());
}

#[test]
fn remote_command_matches_prototype_loop() {
    let c = remote_command(10);
    assert!(c.contains("head -1 /proc/stat"));
    assert!(c.contains("/proc/loadavg") && c.contains("/proc/uptime") && c.contains("/proc/meminfo"));
    assert!(c.contains("df -P /"));
    assert!(c.contains("sleep 10"));
}

#[test]
fn ssh_errors_are_human_readable() {
    assert_eq!(explain_ssh_error("connection timed out"), "SSH: таймаут");
    assert_eq!(explain_ssh_error("Connection refused (os error 10061)"), "SSH: порт закрыт");
    assert_eq!(explain_ssh_error("host key changed"), "SSH: ключ хоста не совпал");
    assert_eq!(explain_ssh_error("unknown host key"), "SSH: неизвестный ключ хоста (нет в known_hosts)");
    assert_eq!(explain_ssh_error("auth rejected"), "SSH: ключ не принят");
    assert_eq!(explain_ssh_error(""), "сессия оборвалась");
}
