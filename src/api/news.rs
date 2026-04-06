use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsResponse {
    pub rows: Vec<NewsArticle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsArticle {
    pub title: String,
    pub slug: String,
    #[serde(rename = "ArticleCategory")]
    pub category: Option<ArticleCategory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArticleCategory {
    pub slug: String,
}

impl NewsArticle {
    pub fn url(&self) -> String {
        let cat = self
            .category
            .as_ref()
            .map(|c| c.slug.as_str())
            .unwrap_or("news");
        format!("https://dayz.com/article/{}/{}", cat, self.slug)
    }
}

pub fn fetch_news(timeout_secs: u64) -> Result<Vec<NewsArticle>> {
    let url = "https://dayz.com/api/article?rowsPerPage=100";
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .user_agent(format!("dayz-ctl {}", env!("CARGO_PKG_VERSION")))
        .build()?;

    let resp: NewsResponse = client
        .get(url)
        .send()
        .context("Failed to fetch DayZ news")?
        .json()
        .context("Failed to parse news JSON")?;

    Ok(resp.rows)
}

pub fn load_cached_news(path: &Path, ttl_secs: u64) -> Result<Option<Vec<NewsArticle>>> {
    if !path.exists() {
        return Ok(None);
    }
    let metadata = fs::metadata(path)?;
    let modified = metadata.modified()?;
    let age = SystemTime::now()
        .duration_since(modified)
        .unwrap_or(Duration::from_secs(u64::MAX));

    if age.as_secs() > ttl_secs {
        return Ok(None);
    }

    let content = fs::read_to_string(path)?;
    let resp: NewsResponse = serde_json::from_str(&content)?;
    Ok(Some(resp.rows))
}

pub fn save_news_cache(path: &Path, articles: &[NewsArticle]) -> Result<()> {
    let resp = NewsResponse {
        rows: articles.to_vec(),
    };
    let json = serde_json::to_string(&resp)?;
    fs::write(path, json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::thread::sleep;

    fn temp_path(prefix: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "dayz-ctl-{prefix}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time before unix epoch")
                .as_nanos()
        ))
    }

    fn sample_articles() -> Vec<NewsArticle> {
        vec![NewsArticle {
            title: "Status Report".into(),
            slug: "status-report".into(),
            category: Some(ArticleCategory {
                slug: "dev-hub".into(),
            }),
        }]
    }

    #[test]
    fn loads_fresh_news_cache() {
        let path = temp_path("news-cache");
        let articles = sample_articles();

        save_news_cache(&path, &articles).expect("save news cache");
        let loaded = load_cached_news(&path, 60).expect("load news cache");

        assert!(loaded.is_some());
        assert_eq!(loaded.expect("cache contents")[0].url(), articles[0].url());

        fs::remove_file(path).expect("remove news cache");
    }

    #[test]
    fn expires_news_cache_after_ttl() {
        let path = temp_path("news-cache-expired");
        let articles = sample_articles();

        save_news_cache(&path, &articles).expect("save news cache");
        sleep(Duration::from_millis(1100));

        let loaded = load_cached_news(&path, 0).expect("load expired news cache");
        assert!(loaded.is_none());

        fs::remove_file(path).expect("remove expired news cache");
    }
}
