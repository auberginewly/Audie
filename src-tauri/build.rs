fn main() -> Result<(), Box<dyn std::error::Error>> {
    let windows_msvc = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    let windows = if windows_msvc {
        tauri_build::WindowsAttributes::new_without_app_manifest()
    } else {
        tauri_build::WindowsAttributes::new()
    };
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))?;

    if windows_msvc {
        // Tauri 2.6 的资源只链接到 bin；lib 单元测试也需要 Common Controls v6，
        // 否则加载 TaskDialogIndirect 时退出（0xc0000139），尚未进入测试。
        // 按上游 #13419 的方案统一由 MSVC 嵌入清单，避免 bin 重复嵌入。
        let manifest = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").ok_or(
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "CARGO_MANIFEST_DIR is missing",
            ),
        )?)
        .join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    }
    Ok(())
}
