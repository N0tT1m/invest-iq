fn main() {
    // Re-embed frontend files when source changes
    println!("cargo:rerun-if-changed=../../frontend/app.py");
    println!("cargo:rerun-if-changed=../../frontend/requirements.txt");
    println!("cargo:rerun-if-changed=../../frontend/components/");
    println!("cargo:rerun-if-changed=../../frontend/assets/");
    println!("cargo:rerun-if-changed=../../frontend/utils/");
}
