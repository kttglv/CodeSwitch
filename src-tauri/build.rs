fn main() {
    println!("cargo:rerun-if-changed=../release");
    println!("cargo:rerun-if-changed=../benchmarks");
    println!("cargo:rerun-if-changed=../crates/engine/src");
    println!("cargo:rerun-if-changed=../resources/wordfreq");
    // A caller-provided environment variable cannot bypass the release gate.
    println!("cargo:rustc-env=CODESWITCH_RELEASE_APPROVED=false");
    let python = if cfg!(windows) { "python" } else { "python3" };
    let approved = std::process::Command::new(python)
        .args(["../scripts/check_release.py", "--quiet"])
        .status()
        .is_ok_and(|status| status.success());
    if approved {
        println!("cargo:rustc-env=CODESWITCH_RELEASE_APPROVED=true");
    }
    tauri_build::build();
}
