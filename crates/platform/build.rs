fn main() {
    println!("cargo:rerun-if-changed=native");
    let target = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    let mut build = cc::Build::new();
    build.cpp(true).include("native");
    match target.as_str() {
        "macos" => {
            build
                .file("native/macos.mm")
                .flag("-std=c++17")
                .flag("-fobjc-arc")
                .flag("-mmacosx-version-min=13.0");
            for framework in ["Cocoa", "ApplicationServices", "Carbon"] {
                println!("cargo:rustc-link-lib=framework={framework}");
            }
        }
        "windows" => {
            build
                .file("native/windows.cpp")
                .flag_if_supported("/std:c++17")
                .define("UNICODE", None)
                .define("_UNICODE", None);
            for lib in [
                "user32",
                "ole32",
                "oleaut32",
                "advapi32",
                "uiautomationcore",
                "uuid",
            ] {
                println!("cargo:rustc-link-lib={lib}");
            }
        }
        _ => {
            build.file("native/unsupported.cpp");
        }
    }
    build.compile("codeswitch_native");
}
