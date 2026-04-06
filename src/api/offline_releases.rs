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

    parse_latest_release(&body)
}

pub fn parse_latest_release(body: &str) -> Result<Option<ReleaseInfo>> {
    let releases: Vec<GithubRelease> =
        serde_json::from_str(body).context("parse DCOM release metadata")?;
    Ok(releases
        .into_iter()
        .filter(|release| !release.draft && !release.prerelease)
        .max_by(|a, b| compare_versions(&a.tag_name, &b.tag_name))
        .map(|release| ReleaseInfo {
            tag: release.tag_name,
            tarball_url: release.tarball_url,
        }))
}

fn normalize_version(version: &str) -> String {
    version.trim_start_matches('v').to_string()
}

fn compare_versions(left: &str, right: &str) -> std::cmp::Ordering {
    let left = parse_version(&normalize_version(left));
    let right = parse_version(&normalize_version(right));
    let max_len = left.len().max(right.len());

    for index in 0..max_len {
        let lhs = *left.get(index).unwrap_or(&0);
        let rhs = *right.get(index).unwrap_or(&0);
        match lhs.cmp(&rhs) {
            std::cmp::Ordering::Equal => continue,
            other => return other,
        }
    }

    std::cmp::Ordering::Equal
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

        let release = parse_latest_release(json)
            .expect("parse releases")
            .expect("stable release");
        assert_eq!(release.tag, "0.5.0");
        assert_eq!(
            release.tarball_url,
            "https://example.test/stable-050.tar.gz"
        );
    }

    #[test]
    fn selects_highest_stable_release_even_when_payload_is_not_latest_first() {
        let json = r#"
        [
          {
            "tag_name": "v0.4.0",
            "draft": false,
            "prerelease": false,
            "tarball_url": "https://example.test/0.4.0.tar.gz"
          },
          {
            "tag_name": "v0.6.0",
            "draft": false,
            "prerelease": false,
            "tarball_url": "https://example.test/0.6.0.tar.gz"
          },
          {
            "tag_name": "v0.5.0",
            "draft": false,
            "prerelease": false,
            "tarball_url": "https://example.test/0.5.0.tar.gz"
          }
        ]
        "#;

        let release = parse_latest_release(json)
            .expect("parse releases")
            .expect("stable release");
        assert_eq!(release.tag, "v0.6.0");
        assert_eq!(release.tarball_url, "https://example.test/0.6.0.tar.gz");
    }

    #[test]
    fn preserves_raw_upstream_tag_name() {
        let json = r#"
        [
          {
            "tag_name": "v1.0.0",
            "draft": false,
            "prerelease": false,
            "tarball_url": "https://example.test/v1.0.0.tar.gz"
          }
        ]
        "#;

        let release = parse_latest_release(json)
            .expect("parse releases")
            .expect("stable release");
        assert_eq!(release.tag, "v1.0.0");
    }

    #[test]
    fn surfaces_malformed_json_as_an_error() {
        let err = parse_latest_release("not json").expect_err("parse error");
        assert!(err.to_string().contains("parse DCOM release metadata"));
    }

    #[test]
    fn surfaces_schema_mismatches_as_an_error() {
        let json = r#"
        [
          {
            "tag_name": "0.5.0",
            "draft": false,
            "prerelease": false
          }
        ]
        "#;

        let err = parse_latest_release(json).expect_err("schema error");
        assert!(err.to_string().contains("parse DCOM release metadata"));
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
