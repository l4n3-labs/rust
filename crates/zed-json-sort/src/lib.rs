//! Zed editor extension for JSON/JSONC sorting.
//!
//! Compiled to WASM and loaded by Zed. Locates (or downloads) the `json-sort-server`
//! binary and launches it as the language server for JSON and JSONC files.

use zed_extension_api::{self as zed, LanguageServerId, Result, Worktree};

/// Extension state, caching the resolved LSP binary path for the session.
struct JsonSortExtension {
    /// Path to the `json-sort-server` binary, cached after first resolution.
    cached_binary_path: Option<String>,
}

impl zed::Extension for JsonSortExtension {
    fn new() -> Self {
        Self { cached_binary_path: None }
    }

    /// Resolve the `json-sort-server` binary path using a 3-step strategy:
    /// 1. Return the cached path if already resolved this session.
    /// 2. Search the system `PATH` via the worktree.
    /// 3. Download the latest release from GitHub.
    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<zed::Command> {
        // 1. Check if we already found the binary this session
        if let Some(path) = &self.cached_binary_path {
            return Ok(zed::Command { command: path.clone(), args: vec![], env: Default::default() });
        }

        // 2. Check PATH
        if let Some(path) = worktree.which("json-sort-server") {
            self.cached_binary_path = Some(path.clone());
            return Ok(zed::Command { command: path, args: vec![], env: Default::default() });
        }

        // 3. Try to download from GitHub releases
        let binary_path = self.download_binary(language_server_id)?;
        self.cached_binary_path = Some(binary_path.clone());

        Ok(zed::Command { command: binary_path, args: vec![], env: Default::default() })
    }

    /// Read `lsp.json-sort-server.initialization_options` from Zed settings and pass
    /// them through to the LSP server.
    fn language_server_initialization_options(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<Option<serde_json::Value>> {
        let settings = zed::settings::LspSettings::for_worktree("json-sort-server", worktree)
            .ok()
            .and_then(|s| s.initialization_options);
        Ok(settings)
    }
}

impl JsonSortExtension {
    /// Download the latest `json-sort-server` release from GitHub for the current platform.
    fn download_binary(&self, language_server_id: &LanguageServerId) -> Result<String> {
        let release = zed::latest_github_release(
            "l4n3-labs/rust",
            zed::GithubReleaseOptions { require_assets: true, pre_release: false },
        )?;

        let (os, arch) = zed::current_platform();
        let asset_suffix = match (os, arch) {
            (zed::Os::Mac, zed::Architecture::Aarch64) => "aarch64-apple-darwin.tar.gz",
            (zed::Os::Mac, zed::Architecture::X8664) => "x86_64-apple-darwin.tar.gz",
            (zed::Os::Linux, zed::Architecture::Aarch64) => "aarch64-unknown-linux-gnu.tar.gz",
            (zed::Os::Linux, zed::Architecture::X8664) => "x86_64-unknown-linux-gnu.tar.gz",
            (zed::Os::Windows, zed::Architecture::X8664) => "x86_64-pc-windows-msvc.zip",
            _ => return Err("unsupported platform".into()),
        };

        let asset_name = format!("json-sort-server-{asset_suffix}");
        let asset = release
            .assets
            .iter()
            .find(|a| a.name == asset_name)
            .ok_or_else(|| format!("no asset found for {asset_name}"))?;

        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::Downloading,
        );

        let file_type =
            if asset_name.ends_with(".zip") { zed::DownloadedFileType::Zip } else { zed::DownloadedFileType::GzipTar };

        zed::download_file(&asset.download_url, "json-sort-server", file_type)?;
        zed::make_file_executable("json-sort-server")?;

        Ok("json-sort-server".to_string())
    }
}

zed::register_extension!(JsonSortExtension);
