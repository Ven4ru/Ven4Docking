//! Spec §6 / plan: the agent must not contain code that reads account credentials or calls private
//! vendor endpoints. This scans every file under agent/src.

use std::path::Path;

/// Forbidden anywhere in agent sources.
const FORBIDDEN: &[&str] = &[
    "credentials.json",
    "auth.json",
    "oauth_creds.json",
    "api/oauth/usage",
    "oauth-2025",
    "copilot_internal",
    "retrieveUserQuota",
    "api.anthropic.com",
    "chatgpt.com",
    "backend-api",
    "cloudcode-pa.googleapis.com",
    "cursor.com/api",
    "windsurf.com/_backend",
    "state.vscdb",
    "claudeAiOauth",
    "accessToken",
    "refreshToken",
    "Cookies",
];

/// The panel API uses `Authorization: Bearer <phone token>` (our own token). Anywhere else — in
/// particular in the AI module — a Bearer header would mean talking to a vendor with an account token.
const BEARER_ALLOWED_IN: &[&str] = &["http"];

fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().and_then(|x| x.to_str()) == Some("rs") {
            out.push(p);
        }
    }
}

#[test]
fn forbidden_sources() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    walk(&src, &mut files);
    assert!(files.len() > 10, "scanner found the sources");
    let mut hits = Vec::new();
    for f in &files {
        let text = std::fs::read_to_string(f).unwrap();
        let lower = text.to_lowercase();
        for s in FORBIDDEN {
            if lower.contains(&s.to_lowercase()) {
                hits.push(format!("{}: {s}", f.display()));
            }
        }
        let rel = f.strip_prefix(&src).unwrap();
        let top = rel.components().next().unwrap().as_os_str().to_string_lossy().to_string();
        if lower.contains("bearer") && !BEARER_ALLOWED_IN.contains(&top.as_str()) {
            hits.push(format!("{}: Bearer outside the panel API", f.display()));
        }
    }
    assert!(hits.is_empty(), "forbidden strings in agent/src:\n{}", hits.join("\n"));
}

#[test]
fn ai_module_makes_no_remote_http() {
    let ai = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("ai");
    let mut files = Vec::new();
    walk(&ai, &mut files);
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap();
        assert!(!text.contains("https://"), "{} mentions a remote URL", f.display());
        assert!(!text.contains("http://"), "{} mentions an URL; only util::http_get_local (127.0.0.1) is allowed", f.display());
    }
}
