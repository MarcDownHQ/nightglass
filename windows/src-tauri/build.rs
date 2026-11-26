use std::path::{Path, PathBuf};
use std::process::Command;

/// The sidecar `bundle > externalBin` names (tauri.conf.json), found next to the app's executable at run time.
const HELPER: &str = "nightglass-widevine";

fn main() {
  // macOS: libmpv and the libraries it loads are found through @rpath: Contents/Frameworks in the app bundle
  // (an rpath tauri-build adds), and lib/macos (scripts/fetch-libmpv.sh) in dev builds, never in a release binary.
  if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") && std::env::var("PROFILE").as_deref() == Ok("debug") {
    let dir = std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{dir}/lib/macos");
  }
  build_helper();
  tauri_build::build()
}

/// The CDM helper (crate nightglass-cdm) as tauri-build wants the sidecar: `binaries/<name>-<target triple>`, which it
/// then copies next to the executable. Cargo can't make one crate's binary a build input of another, so it is built
/// here, in a target directory of its own so as not to wait on the build that runs this script.
fn build_helper() {
  let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
  let workspace = manifest.parent().expect("workspace").to_path_buf();
  let target = std::env::var("TARGET").expect("target");
  let release = std::env::var("PROFILE").as_deref() == Ok("release");
  for watched in ["cdm-helper/src", "cdm-helper/Cargo.toml", "drm/src", "drm/Cargo.toml", "Cargo.lock"] {
    println!("cargo:rerun-if-changed={}", workspace.join(watched).display());
  }

  let target_dir = workspace.join("target").join("cdm-helper");
  let mut cargo = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
  cargo.current_dir(&workspace).args(["build", "-p", "nightglass-cdm", "--target", &target]).arg("--target-dir").arg(&target_dir);
  if release {
    cargo.arg("--release");
  }
  for inherited in ["RUSTC_WORKSPACE_WRAPPER", "CLIPPY_ARGS", "CARGO_TARGET_DIR", "CARGO_BUILD_TARGET"] {
    cargo.env_remove(inherited);
  }
  let status = cargo.status().expect("cargo, to build nightglass-cdm");
  assert!(status.success(), "building nightglass-cdm failed");

  let exe = if target.contains("windows") { ".exe" } else { "" };
  let built = target_dir.join(&target).join(if release { "release" } else { "debug" }).join(format!("nightglass-cdm{exe}"));
  let binaries = manifest.join("binaries");
  std::fs::create_dir_all(&binaries).expect("binaries directory");
  copy_if_changed(&built, &binaries.join(format!("{HELPER}-{target}{exe}")));

  // A universal macOS bundle wants one sidecar for both architectures; tauri builds them one after the other.
  if target.ends_with("-apple-darwin") {
    let arm = binaries.join(format!("{HELPER}-aarch64-apple-darwin"));
    let intel = binaries.join(format!("{HELPER}-x86_64-apple-darwin"));
    if arm.exists() && intel.exists() {
      let status = Command::new("lipo").arg("-create").arg("-output").arg(binaries.join(format!("{HELPER}-universal-apple-darwin"))).arg(&arm).arg(&intel).status();
      assert!(status.is_ok_and(|s| s.success()), "lipo failed to join the two nightglass-cdm builds");
    }
  }
}

fn copy_if_changed(from: &Path, to: &Path) {
  let bytes = std::fs::read(from).unwrap_or_else(|e| panic!("{}: {e}", from.display()));
  if std::fs::read(to).ok().as_deref() != Some(&bytes[..]) {
    std::fs::write(to, &bytes).unwrap_or_else(|e| panic!("{}: {e}", to.display()));
    #[cfg(unix)]
    {
      use std::os::unix::fs::PermissionsExt;
      let _ = std::fs::set_permissions(to, std::fs::Permissions::from_mode(0o755));
    }
  }
}
