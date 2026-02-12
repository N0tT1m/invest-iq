use std::sync::Mutex;
use std::time::Duration;

use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    Manager, RunEvent, WindowEvent,
};

const API_PORT: u16 = 3000;
const HEALTH_TIMEOUT: Duration = Duration::from_secs(120);
const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Tracks whether the embedded server is running so we can log on shutdown.
struct ServerState {
    running: bool,
}

async fn wait_for_api() -> bool {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();

    let url = format!("http://localhost:{}/health", API_PORT);
    let start = std::time::Instant::now();
    while start.elapsed() < HEALTH_TIMEOUT {
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!(
                    "[desktop] API server ready ({:.1}s)",
                    start.elapsed().as_secs_f64()
                );
                return true;
            }
            _ => {}
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    tracing::error!(
        "[desktop] API server failed to start within {}s",
        HEALTH_TIMEOUT.as_secs()
    );
    false
}

pub fn run() {
    // Load .env from the current working directory (or wherever the user runs from)
    dotenvy::dotenv().ok();

    let server_state = Mutex::new(ServerState { running: false });

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(move |app| {
            // --- System Tray ---
            let show = MenuItemBuilder::with_id("show", "Show InvestIQ").build(app)?;
            let hide = MenuItemBuilder::with_id("hide", "Hide").build(app)?;
            let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

            let menu = MenuBuilder::new(app)
                .item(&show)
                .item(&hide)
                .separator()
                .item(&quit)
                .build()?;

            let _tray = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("InvestIQ")
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "hide" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.hide();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::DoubleClick { .. } = event {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            // --- Start embedded API server in a background thread ---
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
                rt.block_on(async move {
                    // Mark server as running
                    if let Ok(mut state) = handle.state::<Mutex<ServerState>>().lock() {
                        state.running = true;
                    }

                    // Spawn the API server
                    let handle_err = handle.clone();
                    let server_handle = tokio::spawn(async move {
                        if let Err(e) = api_server::run_server().await {
                            tracing::error!("[desktop] API server error: {}", e);
                            if let Some(window) = handle_err.get_webview_window("main") {
                                let msg = e.to_string().replace('\'', "\\'");
                                let _ = window.eval(&format!(
                                    "document.querySelector('.status span').textContent = 'Error: {}'",
                                    msg
                                ));
                                let _ = window.show();
                            }
                        }
                    });

                    // Wait for the API server to become available
                    let api_ready = wait_for_api().await;

                    if api_ready {
                        tracing::info!("[desktop] API ready, showing window");
                    } else {
                        tracing::warn!("[desktop] API not ready, showing window anyway");
                    }

                    // Tauri serves the React frontend directly from devUrl/frontendDist,
                    // so we just need to show the window — no navigate() needed.
                    if let Some(window) = handle.get_webview_window("main") {
                        let _ = window.show();
                    }

                    // Keep the runtime alive until the server exits
                    let _ = server_handle.await;
                });
            });

            Ok(())
        })
        .manage(server_state)
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            match event {
                RunEvent::WindowEvent {
                    event: WindowEvent::CloseRequested { api, .. },
                    label,
                    ..
                } => {
                    // Hide instead of close (minimize to tray)
                    if label == "main" {
                        api.prevent_close();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.hide();
                        }
                    }
                }
                RunEvent::ExitRequested { .. } | RunEvent::Exit => {
                    tracing::info!("[desktop] Shutting down...");
                    #[cfg(unix)]
                    unsafe {
                        libc::kill(std::process::id() as i32, libc::SIGTERM);
                    }
                }
                _ => {}
            }
        });
}
