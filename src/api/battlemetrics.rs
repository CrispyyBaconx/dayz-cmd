use anyhow::Result;
use std::time::Duration;

pub fn get_battlemetrics_url(
    ip: &str,
    port: u16,
    name: &str,
    timeout_secs: u64,
) -> Result<Option<String>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .user_agent(format!("dayz-cmd {}", env!("CARGO_PKG_VERSION")))
        .build()?;

    let resp: serde_json::Value = client
        .get("https://api.battlemetrics.com/servers")
        .query(&[
            ("page[size]", "10"),
            ("filter[game]", "dayz"),
            ("filter[search]", &format!("{ip} {name}")),
        ])
        .send()?
        .json()?;

    if let Some(data) = resp["data"].as_array() {
        for entry in data {
            let attrs = &entry["attributes"];
            let entry_ip = attrs["ip"].as_str().unwrap_or("");
            let entry_port = attrs["portQuery"].as_u64().unwrap_or(0) as u16;
            if entry_ip == ip && entry_port == port {
                if let Some(id) = attrs["id"].as_str().or_else(|| entry["id"].as_str()) {
                    return Ok(Some(format!(
                        "https://www.battlemetrics.com/servers/dayz/{id}"
                    )));
                }
            }
        }
    }
    Ok(None)
}
