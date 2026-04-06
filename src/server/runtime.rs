use std::process::Command;

#[derive(Debug, Clone, Default)]
pub struct ServerRuntimeInfo {
    pub ping_ms: Option<f64>,
    pub country: Option<String>,
}

pub fn lookup_runtime_info(ip: &str) -> ServerRuntimeInfo {
    ServerRuntimeInfo {
        ping_ms: lookup_ping(ip),
        country: lookup_country(ip),
    }
}

fn lookup_ping(ip: &str) -> Option<f64> {
    let output = Command::new("ping")
        .args(["-c", "1", "-W", "1", ip])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_ping_output(&String::from_utf8_lossy(&output.stdout))
}

fn lookup_country(ip: &str) -> Option<String> {
    let geoip = Command::new("geoiplookup").arg(ip).output().ok();
    if let Some(output) = geoip {
        if output.status.success() {
            if let Some(country) = parse_geoip_output(&String::from_utf8_lossy(&output.stdout)) {
                return Some(country);
            }
        }
    }

    let whois = Command::new("whois").arg(ip).output().ok()?;
    if !whois.status.success() {
        return None;
    }
    parse_whois_country(&String::from_utf8_lossy(&whois.stdout))
}

fn parse_ping_output(output: &str) -> Option<f64> {
    output
        .lines()
        .find_map(|line| line.split("time=").nth(1))
        .and_then(|tail| tail.split_whitespace().next())
        .and_then(|value| value.parse::<f64>().ok())
}

fn parse_geoip_output(output: &str) -> Option<String> {
    let tail = output.split(':').nth(1)?.trim();
    if tail.contains("IP Address not found") || tail == "N/A" {
        return None;
    }
    Some(tail.to_string())
}

fn parse_whois_country(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        if key.trim().eq_ignore_ascii_case("country") {
            let value = value.trim();
            if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            }
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ping_time_from_output() {
        let output = "64 bytes from 1.2.3.4: icmp_seq=1 ttl=57 time=42.7 ms";
        assert_eq!(parse_ping_output(output), Some(42.7));
    }

    #[test]
    fn parses_geoip_country_from_output() {
        let output = "GeoIP Country Edition: US, United States";
        assert_eq!(
            parse_geoip_output(output),
            Some("US, United States".to_string())
        );
    }

    #[test]
    fn parses_country_from_whois_output() {
        let output = "NetRange: 1.2.3.0 - 1.2.3.255\nCountry: DE\nOrgName: Example";
        assert_eq!(parse_whois_country(output), Some("DE".to_string()));
    }
}
