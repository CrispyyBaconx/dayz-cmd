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
    fetch_latest_release_from_url(&url, timeout_secs)
}

fn fetch_latest_release_from_url(url: &str, timeout_secs: u64) -> Result<Option<ReleaseInfo>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .user_agent(format!("dayz-cmd {}", env!("CARGO_PKG_VERSION")))
        .build()?;

    let body = client
        .get(url)
        .send()
        .context("Failed to fetch DCOM releases")?
        .error_for_status()
        .context("GitHub returned an error response for DCOM releases")?
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
    use std::io::Write;
    use std::net::TcpListener;

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

    #[test]
    fn surfaces_http_errors_instead_of_silently_returning_none() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let addr = listener.local_addr().expect("local addr");

        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let response = concat!(
                "HTTP/1.1 500 Internal Server Error\r\n",
                "Content-Length: 2\r\n",
                "Connection: close\r\n",
                "\r\n",
                "{}"
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });

        let result = fetch_latest_release_from_url(
            &format!("http://{addr}/repos/Arkensor/DayZCommunityOfflineMode/releases"),
            5,
        );

        assert!(result.is_err());
    }
}
