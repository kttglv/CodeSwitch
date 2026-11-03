fn main() {
    // The release gate below bakes its result into the binary, so Cargo must re-run
    // this script whenever any input to the gate changes: everything
    // scripts/source_digest.py fingerprints plus the readiness/benchmark evidence.
    // scripts/test_build_rerun.py keeps this list in sync with the digest.
    for path in [
        "../Cargo.lock",
        "../Cargo.toml",
        "../package.json",
        "../package-lock.json",
        "../src",
        "../scripts",
        "../crates",
        "../release",
        "../benchmarks",
        "../resources/wordfreq",
        "build.rs",
        "src",
        "tests",
        "capabilities",
        "tauri.conf.json",
        "tauri.preview.conf.json",
        "tauri.release.conf.json",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    // A caller-provided environment variable cannot bypass the release gate.
    println!("cargo:rustc-env=CODESWITCH_RELEASE_APPROVED=false");
    let python = if cfg!(windows) { "python" } else { "python3" };
    // -B: no __pycache__ writes inside the watched scripts directory.
    let approved = std::process::Command::new(python)
        .args(["-B", "../scripts/check_release.py", "--quiet"])
        .status()
        .is_ok_and(|status| status.success());
    if approved {
        println!("cargo:rustc-env=CODESWITCH_RELEASE_APPROVED=true");
    }
    tauri_build::build();
}
