use anyhow::{Context, Result};
use serde::Deserialize;
use std::time::Duration;

const INSTALLER_ASSET_NAME: &str = "dayz-ctl-installer-linux.sh";

#[derive(Debug, Clone, PartialEq)]
pub struct ReleaseInfo {
    pub tag: String,
    pub installer_url: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateAvailability {
    UpToDate,
    Available(ReleaseInfo),
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

pub fn check_for_update(
    owner: &str,
    repo: &str,
    current_version: &str,
    timeout_secs: u64,
) -> Result<UpdateAvailability> {
    let latest = fetch_latest_release(owner, repo, timeout_secs)?;
    Ok(match latest {
        Some(release) if is_newer_version(current_version, &release.tag) => {
            UpdateAvailability::Available(release)
        }
        _ => UpdateAvailability::UpToDate,
    })
}

pub fn fetch_latest_release(owner: &str, repo: &str, timeout_secs: u64) -> Result<Option<ReleaseInfo>> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}/releases");
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .user_agent(format!("dayz-ctl {}", env!("CARGO_PKG_VERSION")))
        .build()?;

    let body = client
        .get(url)
        .send()
        .context("Failed to fetch GitHub releases")?
        .text()
        .context("Failed to read GitHub releases response")?;

    Ok(parse_latest_release(&body))
}

fn parse_latest_release(body: &str) -> Option<ReleaseInfo> {
    let releases: Vec<GithubRelease> = serde_json::from_str(body).ok()?;
    releases
        .into_iter()
        .find(|release| !release.draft && !release.prerelease)
        .and_then(|release| {
            release
                .assets
                .into_iter()
                .find(|asset| asset.name == INSTALLER_ASSET_NAME)
                .map(|asset| ReleaseInfo {
                    tag: normalize_version(&release.tag_name),
                    installer_url: asset.browser_download_url,
                })
        })
}

pub fn is_newer_version(current: &str, latest: &str) -> bool {
    parse_version(normalize_version(latest).as_str()) > parse_version(normalize_version(current).as_str())
}

fn normalize_version(version: &str) -> String {
    version.trim_start_matches('v').to_string()
}

fn parse_version(version: &str) -> Vec<u64> {
    version
        .split('.')
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_latest_stable_release_with_installer_asset() {
        let json = r#"
        [
          {
            "tag_name": "0.4.0",
            "draft": false,
            "prerelease": false,
            "assets": [
              { "name": "dayz-ctl-installer-linux.sh", "browser_download_url": "https://example.test/0.4.0/installer.sh" }
            ]
          }
        ]
        "#;

        let release = parse_latest_release(json).expect("latest release");
        assert_eq!(release.tag, "0.4.0");
        assert_eq!(release.installer_url, "https://example.test/0.4.0/installer.sh");
    }

    #[test]
    fn ignores_prereleases_and_drafts() {
        let json = r#"
        [
          {
            "tag_name": "0.5.0-beta.1",
            "draft": false,
            "prerelease": true,
            "assets": [
              { "name": "dayz-ctl-installer-linux.sh", "browser_download_url": "https://example.test/beta/installer.sh" }
            ]
          },
          {
            "tag_name": "0.4.1",
            "draft": true,
            "prerelease": false,
            "assets": [
              { "name": "dayz-ctl-installer-linux.sh", "browser_download_url": "https://example.test/draft/installer.sh" }
            ]
          },
          {
            "tag_name": "0.4.0",
            "draft": false,
            "prerelease": false,
            "assets": [
              { "name": "dayz-ctl-installer-linux.sh", "browser_download_url": "https://example.test/0.4.0/installer.sh" }
            ]
          }
        ]
        "#;

        let release = parse_latest_release(json).expect("stable release");
        assert_eq!(release.tag, "0.4.0");
    }

    #[test]
    fn returns_none_when_installer_asset_missing() {
        let json = r#"
        [
          {
            "tag_name": "0.4.0",
            "draft": false,
            "prerelease": false,
            "assets": [
              { "name": "dayz-ctl-linux.tar.gz", "browser_download_url": "https://example.test/0.4.0/archive.tar.gz" }
            ]
          }
        ]
        "#;

        assert!(parse_latest_release(json).is_none());
    }

    #[test]
    fn compares_versions_with_optional_v_prefix() {
        assert!(is_newer_version("0.3.0", "v0.4.0"));
        assert!(!is_newer_version("0.4.0", "0.4.0"));
        assert!(!is_newer_version("0.5.0", "0.4.0"));
    }
}
