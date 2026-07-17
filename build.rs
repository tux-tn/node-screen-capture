extern crate napi_build;

fn main() {
  napi_build::setup();

  if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");

    if let Ok(output) = std::process::Command::new("xcrun")
      .args(["--find", "swiftc"])
      .output()
      && output.status.success()
    {
      let swiftc = String::from_utf8_lossy(&output.stdout);
      let swiftc = std::path::Path::new(swiftc.trim());
      if let Some(swift_usr) = swiftc.parent().and_then(std::path::Path::parent) {
        let runtime = swift_usr.join("lib/swift/macosx");
        if runtime.exists() {
          println!("cargo:rustc-link-arg=-Wl,-rpath,{}", runtime.display());
        }
      }
    }

    // Build the local Swift helper that bootstraps NSApplication and pumps
    // the main CFRunLoop so the SCContentSharingPicker UI can be shown from
    // a headless Node.js CLI process.
    let swift_helper_source = "lib/ScreenCaptureHelper.swift";
    println!("cargo:rerun-if-changed={}", swift_helper_source);

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let lib_path = format!("{out_dir}/libScreenCaptureHelper.a");

    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let swift_triple = match target_arch.as_str() {
      "x86_64" => "x86_64-apple-macosx14.0",
      "aarch64" => "arm64-apple-macosx14.0",
      other => panic!(
        "screen_capture_node: unsupported target arch '{other}' for Swift helper. \
         Expected x86_64 or aarch64."
      ),
    };

    let status = std::process::Command::new("swiftc")
      .args([
        "-emit-library",
        "-o",
        &lib_path,
        "-static",
        "-O",
        "-target",
        swift_triple,
        swift_helper_source,
      ])
      .status()
      .expect("Failed to run swiftc; ensure Xcode or Command Line Tools is installed");

    if !status.success() {
      panic!("Swift helper compilation failed");
    }

    println!("cargo:rustc-link-search=native={out_dir}");
    println!("cargo:rustc-link-lib=static=ScreenCaptureHelper");
    println!("cargo:rustc-link-lib=framework=AppKit");
  }
}
