use anyhow::{Context, Result};
use serde::Deserialize;
use std::time::Duration;

const OWNER: &str = "Arkensor";
const REPO: &str = "DayZCommunityOfflineMode";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseInfo {
    pub tag: String,
    pub tarball_url: String,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    tarball_url: String,
}

pub fn fetch_latest_release(timeout_secs: u64) -> Result<Option<ReleaseInfo>> {
    let url = format!("https://api.github.com/repos/{OWNER}/{REPO}/releases");
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .user_agent(format!("dayz-cmd {}", env!("CARGO_PKG_VERSION")))
        .build()?;

    let body = client
        .get(url)
        .send()
        .context("Failed to fetch DCOM releases")?
        .text()
        .context("Failed to read DCOM releases response")?;

    Ok(parse_latest_release(&body))
}

pub fn parse_latest_release(body: &str) -> Option<ReleaseInfo> {
    let releases: Vec<GithubRelease> = serde_json::from_str(body).ok()?;
    releases
        .into_iter()
        .find(|release| !release.draft && !release.prerelease)
        .map(|release| ReleaseInfo {
            tag: normalize_version(&release.tag_name),
            tarball_url: release.tarball_url,
        })
}

fn normalize_version(version: &str) -> String {
    version.trim_start_matches('v').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_prereleases_and_drafts_and_selects_latest_stable_release() {
        let json = r#"
        [
          {
            "tag_name": "v0.6.0-beta.1",
            "draft": false,
            "prerelease": true,
            "tarball_url": "https://example.test/beta.tar.gz"
          },
          {
            "tag_name": "0.5.1",
            "draft": true,
            "prerelease": false,
            "tarball_url": "https://example.test/draft.tar.gz"
          },
          {
            "tag_name": "0.5.0",
            "draft": false,
            "prerelease": false,
            "tarball_url": "https://example.test/stable-050.tar.gz"
          }
        ]
        "#;

        let release = parse_latest_release(json).expect("stable release");
        assert_eq!(release.tag, "0.5.0");
        assert_eq!(
            release.tarball_url,
            "https://example.test/stable-050.tar.gz"
        );
    }
}
