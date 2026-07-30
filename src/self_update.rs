use std::{
    env, fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, bail, Context, Result};
use reqwest::{
    blocking::Client,
    header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION},
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

const DEFAULT_UPDATE_API_ROOT: &str = "https://api.github.com";
const DEFAULT_UPDATE_REPO: &str = "nikvdp/mdvi";

#[derive(Debug, Deserialize)]
struct ReleaseResponse {
    tag_name: String,
    assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Deserialize)]
struct ReleaseAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug)]
pub struct SelfUpdateOptions {
    pub check: bool,
    pub force: bool,
    pub tag: Option<String>,
    pub path: Option<PathBuf>,
}

#[derive(Debug)]
pub struct SelfUpdateResult {
    pub local_version: String,
    pub remote_version: String,
    pub already_updated: bool,
    pub binary_path: PathBuf,
    pub release_url: String,
    pub updated_path: Option<PathBuf>,
}

pub fn run_self_update(
    options: SelfUpdateOptions,
    mut status: impl FnMut(&str),
) -> Result<SelfUpdateResult> {
    status("Checking for updates...");
    let api_root =
        env::var("MDVI_UPDATE_API_ROOT").unwrap_or_else(|_| DEFAULT_UPDATE_API_ROOT.to_string());
    let repo = env::var("MDVI_UPDATE_REPO").unwrap_or_else(|_| DEFAULT_UPDATE_REPO.to_string());
    let client = github_client()?;
    let release = fetch_release(&client, &api_root, &repo, options.tag.as_deref())?;
    let remote_version = normalize_tag(&release.tag_name);
    let local_version = normalize_tag(env!("CARGO_PKG_VERSION"));
    let asset_name = release_asset_name()?.to_string();

    status("Locating installed binary...");
    let explicit_path = options.path.is_some();
    let binary_path = resolve_binary_path(options.path)?;
    let release_url = format!("https://github.com/{repo}/releases/tag/{remote_version}");
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name == asset_name)
        .ok_or_else(|| {
            anyhow!("release {remote_version} does not contain the expected asset {asset_name:?}")
        })?;

    let already_updated = remote_version == local_version;
    if options.check || (already_updated && !options.force) {
        return Ok(SelfUpdateResult {
            local_version,
            remote_version,
            already_updated,
            binary_path,
            release_url,
            updated_path: None,
        });
    }

    let checksums = release
        .assets
        .iter()
        .find(|asset| asset.name == "checksums.txt" || asset.name == "SHA256SUMS")
        .ok_or_else(|| anyhow!("release {remote_version} does not include checksums.txt"))?;
    status("Downloading checksums...");
    let checksums = download_text(&client, &checksums.browser_download_url)?;

    status("Downloading update...");
    let bytes = download_bytes(&client, &asset.browser_download_url)?;
    status("Verifying checksum...");
    verify_checksum(&checksums, &asset_name, &bytes)?;

    status("Installing update...");
    let staged_binary = write_staged_binary(&asset_name, &bytes)?;
    let install_result = install_staged_binary(&staged_binary, &binary_path, explicit_path);
    let _ = fs::remove_file(&staged_binary);
    install_result?;

    Ok(SelfUpdateResult {
        local_version,
        remote_version,
        already_updated: false,
        binary_path: binary_path.clone(),
        release_url,
        updated_path: Some(binary_path),
    })
}

fn github_client() -> Result<Client> {
    let mut headers = HeaderMap::new();
    if let Ok(token) = env::var("GITHUB_TOKEN").or_else(|_| env::var("GH_TOKEN")) {
        if !token.trim().is_empty() {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {token}"))
                    .context("GitHub token contains invalid header characters")?,
            );
        }
    }
    Client::builder()
        .user_agent(concat!("mdvi/", env!("CARGO_PKG_VERSION")))
        .default_headers(headers)
        .build()
        .context("could not create GitHub HTTP client")
}

fn fetch_release(
    client: &Client,
    api_root: &str,
    repo: &str,
    tag: Option<&str>,
) -> Result<ReleaseResponse> {
    let tag = tag.map(normalize_tag);
    let url = match tag {
        Some(tag) => format!("{api_root}/repos/{repo}/releases/tags/{tag}"),
        None => format!("{api_root}/repos/{repo}/releases/latest"),
    };
    client
        .get(&url)
        .header(ACCEPT, "application/vnd.github+json")
        .send()
        .with_context(|| format!("could not fetch GitHub release from {url}"))?
        .error_for_status()
        .with_context(|| format!("GitHub release request failed for {url}"))?
        .json()
        .with_context(|| format!("could not parse GitHub release response from {url}"))
}

fn download_text(client: &Client, url: &str) -> Result<String> {
    client
        .get(url)
        .send()
        .with_context(|| format!("could not download checksums from {url}"))?
        .error_for_status()
        .with_context(|| format!("checksum download failed from {url}"))?
        .text()
        .with_context(|| format!("could not read checksums from {url}"))
}

fn download_bytes(client: &Client, url: &str) -> Result<Vec<u8>> {
    let mut response = client
        .get(url)
        .send()
        .with_context(|| format!("could not download release asset from {url}"))?
        .error_for_status()
        .with_context(|| format!("release asset download failed from {url}"))?;
    let mut bytes = Vec::new();
    response
        .read_to_end(&mut bytes)
        .with_context(|| format!("could not read release asset from {url}"))?;
    Ok(bytes)
}

fn write_staged_binary(asset_name: &str, bytes: &[u8]) -> Result<PathBuf> {
    let mut temp = tempfile::Builder::new()
        .prefix(&format!("{asset_name}-"))
        .tempfile()?;
    temp.write_all(bytes)?;
    temp.flush()?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o755))?;
    }

    temp.into_temp_path()
        .keep()
        .context("could not persist the downloaded update")
}

fn install_staged_binary(
    staged_binary: &Path,
    binary_path: &Path,
    explicit_path: bool,
) -> Result<()> {
    if !explicit_path {
        return self_replace::self_replace(staged_binary)
            .with_context(|| format!("could not replace {}", binary_path.display()));
    }

    fs::copy(staged_binary, binary_path)
        .with_context(|| format!("could not replace {}", binary_path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(binary_path, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

fn resolve_binary_path(path: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = path {
        let path = if path.is_absolute() {
            path
        } else {
            env::current_dir()?.join(path)
        };
        if !path.is_file() {
            bail!("target binary is not a regular file: {}", path.display());
        }
        return Ok(path);
    }

    env::current_exe().context("could not resolve the current mdvi executable path")
}

fn release_asset_name() -> Result<&'static str> {
    match (env::consts::OS, env::consts::ARCH) {
        ("linux", "x86_64") => Ok("mdvi-linux-x86_64-musl"),
        ("linux", "aarch64") => Ok("mdvi-linux-aarch64-musl"),
        ("macos", "x86_64") => Ok("mdvi-macos-x86_64"),
        ("macos", "aarch64") => Ok("mdvi-macos-aarch64"),
        ("windows", "x86_64") => Ok("mdvi-windows-x86_64.exe"),
        (os, arch) => bail!("self-update is not configured for {os}/{arch}"),
    }
}

fn verify_checksum(checksums: &str, asset_name: &str, bytes: &[u8]) -> Result<()> {
    let Some(expected) = parse_checksum_file(checksums, asset_name) else {
        bail!("release checksums are missing an entry for {asset_name}");
    };
    let actual = format!("{:x}", Sha256::digest(bytes));
    if actual != expected {
        bail!("checksum mismatch for {asset_name}");
    }
    Ok(())
}

fn parse_checksum_file(checksums: &str, asset_name: &str) -> Option<String> {
    checksums.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let hash = parts.next()?.trim();
        let name = parts.next()?.trim_start_matches('*');
        (hash.len() == 64 && name == asset_name).then(|| hash.to_ascii_lowercase())
    })
}

fn normalize_tag(tag: &str) -> String {
    let tag = tag.trim();
    if tag.starts_with('v') {
        tag.to_string()
    } else {
        format!("v{tag}")
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_checksum_file, verify_checksum};

    const SHA256_ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    #[test]
    fn verifies_the_named_binary_checksum() {
        let checksums = format!("{SHA256_ABC}  mdvi-macos-aarch64\n");
        assert!(verify_checksum(&checksums, "mdvi-macos-aarch64", b"abc").is_ok());
        assert!(verify_checksum(&checksums, "mdvi-linux-x86_64-musl", b"abc").is_err());
        assert_eq!(
            parse_checksum_file(&checksums, "mdvi-macos-aarch64"),
            Some(SHA256_ABC.to_string())
        );
    }
}
