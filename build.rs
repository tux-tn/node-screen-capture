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
  }
}
