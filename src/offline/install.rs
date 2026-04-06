use crate::api::offline_releases::ReleaseInfo;
use crate::config::Config;
use crate::offline::storage::{
    cleanup_stale_staging, load_offline_state, offline_root, promote_release,
    save_offline_state, staging_dir_for_tag, validate_extracted_release,
};
use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use reqwest::blocking::Client;
use std::fs;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use tar::Archive;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedInstallResult {
    pub tag: String,
    pub managed_missions: Vec<String>,
}

pub fn install_release(
    config: &Config,
    release: &ReleaseInfo,
    client: &Client,
) -> Result<ManagedInstallResult> {
    cleanup_stale_staging(config)?;

    let staging_dir = staging_dir_for_tag(config, &release.tag);
    if staging_dir.exists() {
        fs::remove_dir_all(&staging_dir).with_context(|| {
            format!(
                "remove existing staging directory before install: {}",
                staging_dir.display()
            )
        })?;
    }

    let archive_path = download_release_tarball(config, release, client)?;
    let install_result = (|| {
        extract_release_tarball(&archive_path, &staging_dir)?;
        let managed_missions = validate_extracted_release(&staging_dir)?;
        promote_release(config, &release.tag, &staging_dir)?;
        let mut state = load_offline_state(config).unwrap_or_default();
        state.installed_tag = Some(release.tag.clone());
        state.latest_known_tag = Some(release.tag.clone());
        state.managed_missions = managed_missions.clone();
        state.last_check_ts = Some(chrono::Utc::now().timestamp());
        save_offline_state(config, &state)?;
        Ok(ManagedInstallResult {
            tag: release.tag.clone(),
            managed_missions,
        })
    })();

    let _ = fs::remove_file(&archive_path);
    if install_result.is_err() {
        let _ = fs::remove_dir_all(&staging_dir);
    }

    install_result
}

fn download_release_tarball(
    config: &Config,
    release: &ReleaseInfo,
    client: &Client,
) -> Result<PathBuf> {
    let tmp_root = offline_root(config).join("tmp");
    fs::create_dir_all(&tmp_root).context("create offline tmp dir")?;

    let archive_path = tmp_root.join(release.archive_file_name());
    let tmp_path = archive_path.with_file_name(format!("{}.part", release.archive_file_name()));

    let result = (|| {
        let mut response = client
            .get(&release.tarball_url)
            .send()
            .context("download DCOM release tarball")?
            .error_for_status()
            .context("download DCOM release tarball")?;

        let mut file = File::create(&tmp_path).context("create tarball temp file")?;
        io::copy(&mut response, &mut file).context("write DCOM release tarball")?;
        fs::rename(&tmp_path, &archive_path)
            .context("promote DCOM release tarball into offline tmp")?;
        Ok(archive_path)
    })();

    if result.is_err() {
        let _ = fs::remove_file(&tmp_path);
    }

    result
}

fn extract_release_tarball(archive_path: &Path, staging_dir: &Path) -> Result<()> {
    fs::create_dir_all(staging_dir).context("create staging directory")?;

    let archive_file = File::open(archive_path).context("open DCOM release tarball")?;
    let decoder = GzDecoder::new(archive_file);
    let mut archive = Archive::new(decoder);

    for entry in archive
        .entries()
        .context("read DCOM release tarball entries")?
    {
        let mut entry = entry.context("read DCOM release tarball entry")?;
        let path = entry
            .path()
            .context("read DCOM release tarball entry path")?;
        let stripped = strip_first_path_component(&path);
        if stripped.as_os_str().is_empty() {
            continue;
        }

        let output_path = staging_dir.join(&stripped);
        if entry.header().entry_type().is_dir() {
            fs::create_dir_all(&output_path).with_context(|| {
                format!("create extracted directory: {}", output_path.display())
            })?;
            continue;
        }

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("create extracted parent directory: {}", parent.display())
            })?;
        }
        entry
            .unpack(&output_path)
            .with_context(|| format!("extract archive entry to {}", output_path.display()))?;
    }

    Ok(())
}

fn strip_first_path_component(path: &Path) -> PathBuf {
    path.components().skip(1).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::offline::storage::release_dir_for_tag;
    use crate::offline::storage::save_offline_state;
    use crate::offline::types::OfflineState;
    use std::fs;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn offline_install_downloads_selected_tarball_into_offline_tmp() {
        let root = test_root("install-download");
        fs::create_dir_all(&root).expect("create temp root");
        let config = test_config(&root);
        let client = test_client();
        let release = sample_release(&start_tarball_server());

        let archive_path =
            download_release_tarball(&config, &release, &client).expect("download archive");

        assert!(archive_path.starts_with(root.join("offline/tmp")));
        assert!(archive_path.exists());
    }

    #[test]
    fn offline_install_extracts_tarball_into_staging_directory() {
        let root = test_root("install-extract");
        fs::create_dir_all(&root).expect("create temp root");
        let config = test_config(&root);
        let client = test_client();
        let release = sample_release(&start_tarball_server());
        let archive_path =
            download_release_tarball(&config, &release, &client).expect("download archive");
        let staging_dir = staging_dir_for_tag(&config, &release.tag);

        extract_release_tarball(&archive_path, &staging_dir).expect("extract archive");

        assert!(
            staging_dir
                .join(
                    "Missions/DayZCommunityOfflineMode.ChernarusPlus/core/CommunityOfflineClient.c"
                )
                .exists()
        );
    }

    #[test]
    fn offline_install_failed_download_or_extract_leaves_previous_release_untouched() {
        let root = test_root("install-failure");
        fs::create_dir_all(&root).expect("create temp root");
        let config = test_config(&root);
        let existing_release = release_dir_for_tag(&config, "v1.0.0");
        fs::create_dir_all(
            existing_release.join("Missions/DayZCommunityOfflineMode.ChernarusPlus/core"),
        )
        .expect("create existing release");
        fs::write(
            existing_release.join(
                "Missions/DayZCommunityOfflineMode.ChernarusPlus/core/CommunityOfflineClient.c",
            ),
            "HIVE_ENABLED = false;",
        )
        .expect("write existing mission");
        save_offline_state(
            &config,
            &OfflineState {
                installed_tag: Some("v1.0.0".into()),
                latest_known_tag: None,
                managed_missions: vec!["DayZCommunityOfflineMode.ChernarusPlus".into()],
                last_check_ts: None,
            },
        )
        .expect("save state");

        let client = test_client();
        let release = sample_release(&start_error_server());
        let result = install_release(&config, &release, &client);

        assert!(result.is_err());
        assert_eq!(
            crate::offline::storage::load_offline_state(&config)
                .expect("load state")
                .installed_tag
                .as_deref(),
            Some("v1.0.0")
        );
        assert!(
            existing_release
                .join(
                    "Missions/DayZCommunityOfflineMode.ChernarusPlus/core/CommunityOfflineClient.c"
                )
                .exists()
        );
    }

    #[test]
    fn offline_install_extract_failure_leaves_previous_release_untouched() {
        let root = test_root("install-extract-failure");
        fs::create_dir_all(&root).expect("create temp root");
        let config = test_config(&root);
        let existing_release = release_dir_for_tag(&config, "v1.0.0");
        fs::create_dir_all(
            existing_release.join("Missions/DayZCommunityOfflineMode.ChernarusPlus/core"),
        )
        .expect("create existing release");
        fs::write(
            existing_release.join(
                "Missions/DayZCommunityOfflineMode.ChernarusPlus/core/CommunityOfflineClient.c",
            ),
            "HIVE_ENABLED = false;",
        )
        .expect("write existing mission");
        save_offline_state(
            &config,
            &OfflineState {
                installed_tag: Some("v1.0.0".into()),
                latest_known_tag: None,
                managed_missions: vec!["DayZCommunityOfflineMode.ChernarusPlus".into()],
                last_check_ts: None,
            },
        )
        .expect("save state");

        let client = test_client();
        let release = sample_release(&start_tarball_server());
        let archive_path =
            download_release_tarball(&config, &release, &client).expect("download archive");
        fs::write(&archive_path, b"not a gzip archive").expect("corrupt downloaded archive");
        let staging_dir = staging_dir_for_tag(&config, &release.tag);
        let result = extract_release_tarball(&archive_path, &staging_dir);

        assert!(result.is_err());
        assert_eq!(
            crate::offline::storage::load_offline_state(&config)
                .expect("load state")
                .installed_tag
                .as_deref(),
            Some("v1.0.0")
        );
        assert!(
            existing_release
                .join(
                    "Missions/DayZCommunityOfflineMode.ChernarusPlus/core/CommunityOfflineClient.c"
                )
                .exists()
        );
    }

    #[test]
    fn offline_install_valid_release_promotes_content_and_returns_missions() {
        let root = test_root("install-success");
        fs::create_dir_all(&root).expect("create temp root");
        let config = test_config(&root);
        let client = test_client();
        let release = sample_release(&start_tarball_server());

        let result = install_release(&config, &release, &client).expect("install release");

        assert_eq!(result.tag, "v1.0.0");
        assert_eq!(
            result.managed_missions,
            vec!["DayZCommunityOfflineMode.ChernarusPlus".to_string()]
        );
        assert!(
            release_dir_for_tag(&config, "v1.0.0")
                .join(
                    "Missions/DayZCommunityOfflineMode.ChernarusPlus/core/CommunityOfflineClient.c"
                )
                .exists()
        );
    }

    fn start_tarball_server() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let addr = listener.local_addr().expect("local addr");
        let payload = build_tarball(true);

        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                payload.len()
            );
            stream.write_all(header.as_bytes()).expect("write header");
            stream.write_all(&payload).expect("write payload");
        });

        format!("http://{addr}/archive.tar.gz")
    }

    fn start_error_server() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let addr = listener.local_addr().expect("local addr");

        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
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

        format!("http://{addr}/archive.tar.gz")
    }

    fn build_tarball(include_client_file: bool) -> Vec<u8> {
        use flate2::Compression;
        use flate2::write::GzEncoder;
        use tar::Builder;

        let mut tar_data = Vec::new();
        {
            let mut builder = Builder::new(&mut tar_data);
            let mission_root = "Arkensor-DayZCommunityOfflineMode-123/Missions/DayZCommunityOfflineMode.ChernarusPlus";
            let mut dir_header = tar::Header::new_gnu();
            dir_header.set_entry_type(tar::EntryType::Directory);
            dir_header.set_mode(0o755);
            dir_header.set_size(0);
            dir_header.set_cksum();
            builder
                .append_data(&mut dir_header, format!("{mission_root}/core/"), &[][..])
                .expect("append core dir");
            if include_client_file {
                let content = b"HIVE_ENABLED = true;";
                let mut header = tar::Header::new_gnu();
                header.set_size(content.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                builder
                    .append_data(
                        &mut header,
                        format!("{mission_root}/core/CommunityOfflineClient.c"),
                        &content[..],
                    )
                    .expect("append file");
            }
            builder.finish().expect("finish tar");
        }

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&tar_data).expect("compress tar");
        encoder.finish().expect("finish gzip")
    }

    fn sample_release(tarball_url: &str) -> ReleaseInfo {
        ReleaseInfo {
            tag: "v1.0.0".into(),
            tarball_url: tarball_url.into(),
        }
    }

    fn test_root(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("dayz-cmd-{name}-{}-{unique}", std::process::id()))
    }

    fn test_config(root: &Path) -> Config {
        Config {
            path: root.join("dayz-cmd.conf"),
            data_dir: root.to_path_buf(),
            server_db_path: root.join("servers.json"),
            news_db_path: root.join("news.json"),
            mods_db_path: root.join("mods.json"),
            profile_path: root.join("profile.json"),
            api_url: "https://example.test/api".into(),
            github_owner: "owner".into(),
            github_repo: "repo".into(),
            request_timeout: 1,
            server_request_timeout: 1,
            server_db_ttl: 1,
            news_db_ttl: 1,
            history_size: 5,
            steamcmd_enabled: true,
            filter_mod_limit: 10,
            filter_players_limit: 50,
            filter_players_slots: 60,
            applications_dir: root.join("applications"),
        }
    }

    fn test_client() -> Client {
        Client::builder().build().expect("build client")
    }
}
