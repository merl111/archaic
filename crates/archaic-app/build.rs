use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=../../assets/branding/archaic.ico");
    println!("cargo:rerun-if-changed=../../packaging/windows/archaic.rc");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    assert_eq!(
        env::var("CARGO_CFG_TARGET_ENV").as_deref(),
        Ok("msvc"),
        "CUI requires MSVC"
    );
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let resource = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("archaic.res");
    let status = Command::new("rc.exe")
        .current_dir(&root)
        .arg("/nologo")
        .arg(format!("/fo{}", resource.display()))
        .arg("packaging/windows/archaic.rc")
        .status()
        .expect("Run from a Visual Studio developer terminal with the Windows SDK (rc.exe)");
    assert!(status.success(), "Windows icon resource compilation failed");
    println!("cargo:rustc-link-arg-bins={}", resource.display());
}
