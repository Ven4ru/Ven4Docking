//! Firewall state for /admin/status (the rules themselves are changed only by `--firewall add|remove`).

use ven4docking_agent::firewall::{classify, RuleInfo, RULE_NAME};

const AGENT: &str = r"C:\Users\Test\AppData\Local\Programs\Ven4Docking\ven4docking-agent.exe";

fn rule(name: &str, app: &str, allow: bool, protocol: i32) -> RuleInfo {
    RuleInfo { name: name.into(), app: app.into(), allow, inbound: true, enabled: true, protocol }
}

#[test]
fn both_allow_rules_are_ok() {
    let rules = vec![rule(RULE_NAME, AGENT, true, 6), rule(RULE_NAME, &AGENT.to_uppercase(), true, 17)];
    assert_eq!(classify(&rules, AGENT), "ok");
}

#[test]
fn missing_disabled_or_foreign_rules() {
    assert_eq!(classify(&[], AGENT), "missing");
    // only TCP
    assert_eq!(classify(&[rule(RULE_NAME, AGENT, true, 6)], AGENT), "missing");
    // rules for another copy of the agent do not count
    let other = r"D:\old\ven4docking-agent.exe";
    assert_eq!(classify(&[rule(RULE_NAME, other, true, 6), rule(RULE_NAME, other, true, 17)], AGENT), "missing");
    // a disabled rule does not let the phone in
    let mut udp = rule(RULE_NAME, AGENT, true, 17);
    udp.enabled = false;
    assert_eq!(classify(&[rule(RULE_NAME, AGENT, true, 6), udp], AGENT), "missing");
    // outbound rules are not what the phone needs
    let mut udp = rule(RULE_NAME, AGENT, true, 17);
    udp.inbound = false;
    assert_eq!(classify(&[rule(RULE_NAME, AGENT, true, 6), udp], AGENT), "missing");
    // a rule with another name (e.g. a user's own) is not ours
    assert_eq!(classify(&[rule("My rule", AGENT, true, 6), rule("My rule", AGENT, true, 17)], AGENT), "missing");
}

#[test]
fn windows_block_rules_win() {
    // what Windows creates after «Allow access?» is cancelled: block rules named after the exe
    let rules = vec![
        rule(RULE_NAME, AGENT, true, 6),
        rule(RULE_NAME, AGENT, true, 17),
        rule("ven4docking-agent.exe", AGENT, false, 6),
    ];
    assert_eq!(classify(&rules, AGENT), "blocked");
    assert_eq!(classify(&[rule("ven4docking-agent.exe", AGENT, false, 17)], AGENT), "blocked");
}
