use std::{
    env, fs,
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, bail, Context, Result};
use flate2::read::GzDecoder;
use reqwest::{
    blocking::Client,
    header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION},
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use zip::ZipArchive;

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
    digest: Option<String>,
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

    let expected_checksum = release_asset_checksum(asset)?;
    status("Downloading update...");
    let archive_bytes = download_bytes(&client, &asset.browser_download_url)?;
    status("Verifying checksum...");
    verify_checksum(expected_checksum, &asset_name, &archive_bytes)?;
    status("Extracting update...");
    let binary_bytes = extract_binary(&asset_name, &archive_bytes)?;

    status("Installing update...");
    let staged_binary = write_staged_binary(&asset_name, &binary_bytes)?;
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

fn release_asset_checksum(asset: &ReleaseAsset) -> Result<&str> {
    let digest = asset
        .digest
        .as_deref()
        .context("GitHub did not provide a checksum for the release asset")?;
    let checksum = digest
        .strip_prefix("sha256:")
        .context("GitHub release asset has an unsupported checksum format")?;
    if checksum.len() != 64
        || !checksum
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        bail!("GitHub release asset has an invalid SHA-256 checksum");
    }
    Ok(checksum)
}

fn extract_binary(asset_name: &str, archive_bytes: &[u8]) -> Result<Vec<u8>> {
    let binary_name = release_binary_name();
    if let Some(package_name) = asset_name.strip_suffix(".tar.gz") {
        let expected_path = Path::new(package_name).join(binary_name);
        let mut archive = tar::Archive::new(GzDecoder::new(Cursor::new(archive_bytes)));
        for entry in archive
            .entries()
            .context("could not read the release tarball")?
        {
            let mut entry = entry.context("could not read an entry from the release tarball")?;
            if entry
                .path()
                .context("could not read a release tarball entry path")?
                .as_ref()
                == expected_path
            {
                let mut binary = Vec::new();
                entry
                    .read_to_end(&mut binary)
                    .context("could not extract mdvi from the release tarball")?;
                return Ok(binary);
            }
        }
        bail!(
            "release tarball does not contain {}",
            expected_path.display()
        );
    }

    if asset_name.ends_with(".zip") {
        let mut archive = ZipArchive::new(Cursor::new(archive_bytes))
            .context("could not read the release zip")?;
        let mut entry = archive
            .by_name(binary_name)
            .with_context(|| format!("release zip does not contain {binary_name}"))?;
        let mut binary = Vec::new();
        entry
            .read_to_end(&mut binary)
            .context("could not extract mdvi from the release zip")?;
        return Ok(binary);
    }

    bail!("self-update does not support release asset {asset_name:?}")
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
        ("linux", "x86_64") => Ok("mdvi-linux-x86_64-musl.tar.gz"),
        ("linux", "aarch64") => Ok("mdvi-linux-aarch64-musl.tar.gz"),
        ("macos", "x86_64") => Ok("mdvi-macos-x86_64.tar.gz"),
        ("macos", "aarch64") => Ok("mdvi-macos-aarch64.tar.gz"),
        ("windows", "x86_64") => Ok("mdvi-windows-x86_64.zip"),
        (os, arch) => bail!("self-update is not configured for {os}/{arch}"),
    }
}

fn release_binary_name() -> &'static str {
    if cfg!(windows) {
        "mdvi.exe"
    } else {
        "mdvi"
    }
}

fn verify_checksum(expected: &str, asset_name: &str, bytes: &[u8]) -> Result<()> {
    let actual = format!("{:x}", Sha256::digest(bytes));
    if !actual.eq_ignore_ascii_case(expected) {
        bail!("checksum mismatch for {asset_name}");
    }
    Ok(())
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
    use std::io::{Cursor, Write};

    use anyhow::Result;
    use flate2::{write::GzEncoder, Compression};
    use tar::{Builder, Header};
    use zip::{write::SimpleFileOptions, ZipWriter};

    use super::{
        extract_binary, release_asset_checksum, release_binary_name, verify_checksum, ReleaseAsset,
    };

    const SHA256_ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    #[test]
    fn verifies_the_github_asset_checksum() -> Result<()> {
        let asset = ReleaseAsset {
            name: "mdvi-macos-aarch64.tar.gz".to_string(),
            browser_download_url: "https://example.test/mdvi".to_string(),
            digest: Some(format!("sha256:{SHA256_ABC}")),
        };

        assert_eq!(release_asset_checksum(&asset)?, SHA256_ABC);
        assert!(verify_checksum(SHA256_ABC, &asset.name, b"abc").is_ok());
        assert!(verify_checksum(SHA256_ABC, &asset.name, b"abd").is_err());
        Ok(())
    }

    #[test]
    fn extracts_mdvi_from_existing_release_archives() -> Result<()> {
        let binary_name = release_binary_name();
        let binary = b"mdvi binary";

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        {
            let mut archive = Builder::new(&mut encoder);
            let mut header = Header::new_gnu();
            header.set_size(binary.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            archive.append_data(
                &mut header,
                format!("mdvi-macos-aarch64/{binary_name}"),
                &binary[..],
            )?;
            archive.finish()?;
        }
        let tarball = encoder.finish()?;
        assert_eq!(
            extract_binary("mdvi-macos-aarch64.tar.gz", &tarball)?,
            binary
        );

        let mut archive = ZipWriter::new(Cursor::new(Vec::new()));
        archive.start_file(binary_name, SimpleFileOptions::default())?;
        archive.write_all(binary)?;
        let zip = archive.finish()?.into_inner();
        assert_eq!(extract_binary("mdvi-windows-x86_64.zip", &zip)?, binary);
        Ok(())
    }
}
