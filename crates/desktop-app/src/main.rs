// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Guard: if Python multiprocessing "spawn" re-launched this binary as a
    // worker, exit immediately.  With PyO3 embedded ML, sys.executable points
    // to this binary, so Python's multiprocessing would create an infinite
    // cascade of new desktop-app processes without this check.
    if std::env::args().any(|a| a.contains("multiprocessing"))
        || std::env::var("_PYTHON_MULTIPROCESSING_WORKER").is_ok()
    {
        return;
    }

    desktop_app::run()
}
