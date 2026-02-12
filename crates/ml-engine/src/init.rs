use pyo3::prelude::*;
use pyo3::types::PyList;
use std::path::PathBuf;

/// Prepare the embedded Python interpreter.
///
/// Sets `PYTHONPATH` so that our `ml-services/` directory is importable, and
/// optionally prepends a virtualenv's site-packages to `sys.path`.
pub fn setup_python(ml_services_path: &str) -> PyResult<()> {
    Python::attach(|py| {
        let sys = py.import("sys")?;
        let path: Bound<'_, PyList> = sys.getattr("path")?.cast_into()?;

        // Add ml-services root so `import signal_models.model` works
        let abs_path = std::fs::canonicalize(ml_services_path)
            .unwrap_or_else(|_| PathBuf::from(ml_services_path));
        let path_str = abs_path.to_string_lossy().to_string();

        if !python_path_contains(&path, &path_str)? {
            path.insert(0, &path_str)?;
            tracing::info!("Added to sys.path: {}", path_str);
        }

        // If a venv is active, make sure its site-packages are on the path
        if let Ok(venv) = std::env::var("VIRTUAL_ENV") {
            let site_packages = find_site_packages(&venv);
            if let Some(sp) = site_packages {
                let sp_str = sp.to_string_lossy().to_string();
                if !python_path_contains(&path, &sp_str)? {
                    path.insert(0, &sp_str)?;
                    tracing::info!("Added venv site-packages: {}", sp_str);
                }
            }
        }

        tracing::debug!("Python sys.path configured");
        Ok(())
    })
}

/// Detect the best available PyTorch device: mps > cuda > cpu.
pub fn detect_device() -> Option<String> {
    Python::attach(|py| {
        let torch = py.import("torch").ok()?;

        // Check MPS (Apple Silicon)
        let mps = torch.getattr("backends").ok()?.getattr("mps").ok()?;
        if mps
            .call_method0("is_available")
            .ok()?
            .extract::<bool>()
            .unwrap_or(false)
        {
            tracing::info!("GPU detected: Apple MPS (Metal Performance Shaders)");
            return Some("mps".to_string());
        }

        // Check CUDA (NVIDIA)
        let cuda = torch.getattr("cuda").ok()?;
        if cuda
            .call_method0("is_available")
            .ok()?
            .extract::<bool>()
            .unwrap_or(false)
        {
            let device_name = cuda
                .call_method1("get_device_name", (0i32,))
                .ok()
                .and_then(|v| v.extract::<String>().ok())
                .unwrap_or_else(|| "unknown".into());
            tracing::info!("GPU detected: CUDA ({})", device_name);
            return Some("cuda".to_string());
        }

        tracing::info!("No GPU detected, using CPU");
        Some("cpu".to_string())
    })
}

fn python_path_contains(path: &Bound<'_, PyList>, needle: &str) -> PyResult<bool> {
    for item in path.iter() {
        if item.extract::<String>()? == needle {
            return Ok(true);
        }
    }
    Ok(false)
}

fn find_site_packages(venv: &str) -> Option<PathBuf> {
    let base = std::fs::canonicalize(venv).unwrap_or_else(|_| PathBuf::from(venv));
    // Unix: lib/pythonX.Y/site-packages
    let lib = base.join("lib");
    if lib.exists() {
        if let Ok(entries) = std::fs::read_dir(&lib) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with("python") {
                    let sp = entry.path().join("site-packages");
                    if sp.exists() {
                        return Some(sp);
                    }
                }
            }
        }
    }
    // Windows: Lib/site-packages
    let sp = base.join("Lib").join("site-packages");
    if sp.exists() {
        return Some(sp);
    }
    None
}
