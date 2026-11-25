#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! ven4docking-agent.exe — background agent with a tray icon.
//! Flags: --no-tray (headless), --console (log to stderr too).
//!        --firewall add|remove: only change the firewall rules «Ven4Docking» for this exe and exit
//!        (needs elevation; exit code 0 = done). No tray, server or data folder in this mode.
//! Env: VEN4DOCKING_HOME (data dir), VEN4DOCKING_PORT (override config port),
//!      VEN4DOCKING_PANEL_DIR, VEN4DOCKING_APK_DIR.

use std::net::SocketAddr;
use std::sync::Arc;
use ven4docking_agent::{config, discovery, firewall, http, logging, metrics, platform, poll, state::AppState};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--firewall") {
        let action = args.get(i + 1).map(String::as_str).unwrap_or("");
        std::process::exit(firewall::run_cli(action));
    }
    let no_tray = args.iter().any(|a| a == "--no-tray");
    let console = args.iter().any(|a| a == "--console");

    let paths = config::Paths::from_env();
    let _ = std::fs::create_dir_all(&paths.home);
    let _ = std::fs::create_dir_all(&paths.layouts);
    let _ = std::fs::create_dir_all(&paths.icons);
    logging::init(paths.log.clone(), console);

    let (mut cfg, load_err) = match config::Config::load_or_create(&paths.config) {
        Ok(c) => (c, None),
        Err(e) => {
            // never overwrite a broken file: run with defaults, the editor shows the error
            tracing::error!("config.json: {e}; running with defaults");
            (config::Config::default(), Some(e.to_string()))
        }
    };
    if let Some(p) = std::env::var("VEN4DOCKING_PORT").ok().and_then(|p| p.parse().ok()) {
        cfg.port = p;
    }
    let port = cfg.port;
    if !platform::single_instance(port) {
        tracing::warn!("another agent on port {port} is running; exiting");
        return;
    }
    let admin_key = match config::load_or_create_admin_key(&paths.admin_key) {
        Ok(k) => k,
        Err(e) => {
            tracing::error!("admin.key: {e}");
            return;
        }
    };
    tracing::info!("Ven4Docking agent {} starting, home {}, port {port}, panel {}", config::AGENT_VERSION, paths.home.display(), paths.panel.display());

    let st = Arc::new(AppState::new(paths, cfg, admin_key, Box::new(metrics::WinActions)));
    *st.load_error.lock().unwrap() = load_err;
    let _sampler = poll::spawn_sampler(st.clone());

    let st_rt = st.clone();
    let rt_thread = std::thread::Builder::new()
        .name("runtime".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .max_blocking_threads(4)
                .thread_keep_alive(std::time::Duration::from_secs(10))
                .build()
                .expect("tokio runtime");
            rt.block_on(async move {
                st_rt.reconcile_servers();
                tokio::spawn(discovery::run(st_rt.discovery_port, st_rt.port));
                let listener = match tokio::net::TcpListener::bind(("0.0.0.0", st_rt.port)).await {
                    Ok(l) => l,
                    Err(e) => {
                        tracing::error!("cannot listen on {}: {e}", st_rt.port);
                        std::process::exit(2);
                    }
                };
                tracing::info!("listening on 0.0.0.0:{}", st_rt.port);
                let app = http::router(st_rt.clone());
                if let Err(e) = axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await {
                    tracing::error!("http server: {e}");
                }
            });
        })
        .expect("runtime thread");

    #[cfg(windows)]
    if !no_tray {
        match ven4docking_agent::tray::run(st.clone()) {
            Ok(()) => {
                tracing::info!("bye");
                std::process::exit(0);
            }
            Err(e) => tracing::warn!("tray unavailable ({e}); running without it"),
        }
    }
    let _ = no_tray;
    let _ = rt_thread.join();
}
