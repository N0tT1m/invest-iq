use rust_embed::Embed;

#[derive(Embed)]
#[folder = "../../frontend/"]
#[exclude = "venv/*"]
#[exclude = "__pycache__/*"]
#[exclude = "*.pyc"]
#[exclude = "Dockerfile"]
#[exclude = "package-lock.json"]
pub struct FrontendAssets;
