fn main() -> anyhow::Result<()> {
    // Guard: if Python multiprocessing "spawn" re-launched this binary as a
    // worker, exit immediately before creating the tokio runtime or doing
    // anything else.  See lib.rs for the full explanation.
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a.contains("multiprocessing"))
        || std::env::var("_PYTHON_MULTIPROCESSING_WORKER").is_ok()
    {
        return Ok(());
    }

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(api_server::run_server())
}
