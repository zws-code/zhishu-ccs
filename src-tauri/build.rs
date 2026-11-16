fn dotenv_value(contents: &str, key: &str) -> Option<String> {
    contents.lines().find_map(|line| {
        let value = line.strip_prefix(key)?.strip_prefix('=')?.trim();
        if value.len() >= 2
            && ((value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\'')))
        {
            Some(value[1..value.len() - 1].to_string())
        } else {
            Some(value.to_string())
        }
    })
}

fn embed_release_platform_config() {
    let manifest_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let env_path = manifest_dir.join("../src/.env.production.local");
    println!("cargo:rerun-if-changed={}", env_path.display());
    if std::env::var("PROFILE").as_deref() != Ok("release") {
        return;
    }
    let Ok(contents) = std::fs::read_to_string(env_path) else {
        return;
    };
    if dotenv_value(&contents, "VITE_ZHISHU_PLATFORM_ALLOW_INSECURE_HTTP").as_deref()
        == Some("true")
    {
        println!("cargo:rustc-env=ZHISHU_PLATFORM_ALLOW_INSECURE_HTTP=true");
    }
}

fn main() {
    embed_release_platform_config();
    // Re-embed application icons whenever their source assets change.
    println!("cargo:rerun-if-changed=icons");
    tauri_build::build();

    // Windows: Embed Common Controls v6 manifest for test binaries
    //
    // When running `cargo test`, the generated test executables don't include
    // the standard Tauri application manifest. Without Common Controls v6,
    // `tauri::test` calls fail with STATUS_ENTRYPOINT_NOT_FOUND.
    //
    // This workaround:
    // 1. Embeds the manifest into test binaries via /MANIFEST:EMBED
    // 2. Uses /MANIFEST:NO for the main binary to avoid duplicate resources
    //    (Tauri already handles manifest embedding for the app binary)
    #[cfg(target_os = "windows")]
    {
        let manifest_path = std::path::PathBuf::from(
            std::env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR"),
        )
        .join("common-controls.manifest");
        let manifest_arg = format!("/MANIFESTINPUT:{}", manifest_path.display());

        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg={}", manifest_arg);
        // Avoid duplicate manifest resources in binary builds.
        println!("cargo:rustc-link-arg-bins=/MANIFEST:NO");
        println!("cargo:rerun-if-changed={}", manifest_path.display());
    }
}
