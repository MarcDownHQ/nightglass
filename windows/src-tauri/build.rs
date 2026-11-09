fn main() {
  // macOS: libmpv and the libraries it loads are found through @rpath: Contents/Frameworks in the app bundle
  // (an rpath tauri-build adds), and lib/macos (scripts/fetch-libmpv.sh) in dev builds, never in a release binary.
  if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") && std::env::var("PROFILE").as_deref() == Ok("debug") {
    let dir = std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{dir}/lib/macos");
  }
  tauri_build::build()
}
