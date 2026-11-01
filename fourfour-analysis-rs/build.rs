use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let macos = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos");
    if !macos || std::env::var_os("CARGO_FEATURE_ORT").is_none() {
        return;
    }
    // ONNX Runtime's prebuilt macOS library (pyke, CoreML build) uses `@available`, which needs
    // `___isPlatformVersionAtLeast` from clang's compiler-rt; rustc links with -nodefaultlibs, so add it by hand.
    let out = Command::new("clang").arg("--print-resource-dir").output().expect("clang --print-resource-dir (Xcode command line tools)");
    assert!(out.status.success(), "clang --print-resource-dir failed: {}", String::from_utf8_lossy(&out.stderr));
    let dir = String::from_utf8(out.stdout).expect("clang resource dir is UTF-8");
    println!("cargo:rustc-link-search=native={}/lib/darwin", dir.trim());
    println!("cargo:rustc-link-lib=static=clang_rt.osx");
}
