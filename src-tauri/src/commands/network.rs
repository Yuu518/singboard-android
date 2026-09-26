use hickory_resolver::config::{ConnectionConfig, NameServerConfig, ResolverConfig, ResolverOpts};
use hickory_resolver::net::runtime::TokioRuntimeProvider;
use hickory_resolver::proto::rr::RecordType;
use hickory_resolver::Resolver;
use reqwest;
use serde::Serialize;
use std::net::IpAddr;
use std::str::FromStr;
use std::time::{Duration, Instant};

const FALLBACK_DNS: [&str; 2] = ["223.5.5.5", "1.1.1.1"];

pub(crate) fn build_client(timeout: Option<Duration>) -> Result<reqwest::Client, reqwest::Error> {
    let mut builder = reqwest::Client::builder();

    if let Some(t) = timeout {
        builder = builder.timeout(t);
    }

    builder.no_proxy().build()
}

#[tauri::command]
pub async fn fetch_url(url: String) -> Result<String, String> {
    let client = build_client(None).map_err(|e| e.to_string())?;
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    resp.text().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn http_ping(url: String, count: u32) -> Result<f64, String> {
    let client = build_client(Some(Duration::from_secs(5))).map_err(|e| e.to_string())?;

    let mut total = 0.0;
    let mut success = 0u32;

    for _ in 0..count {
        let start = Instant::now();
        if client.head(&url).send().await.is_ok() {
            total += start.elapsed().as_secs_f64() * 1000.0;
            success += 1;
        }
    }

    if success == 0 {
        return Err("timeout".to_string());
    }
    Ok(total / success as f64)
}

#[derive(Serialize)]
pub struct DnsRecord {
    record_type: String,
    name: String,
    ttl: u32,
    value: String,
    ip: Option<String>,
}

#[derive(Serialize)]
pub struct DnsQueryResult {
    source: String,
    server: Option<String>,
    records: Vec<DnsRecord>,
}

fn parse_record_type(record_type: &str) -> Result<RecordType, String> {
    let normalized = record_type.trim().to_ascii_uppercase();
    if normalized.is_empty() {
        return Err("DNS record type is empty".to_string());
    }
    RecordType::from_str(&normalized)
        .map_err(|_| format!("unsupported DNS record type: {}", normalized))
}

fn parse_dns_server(server: &str) -> Result<(IpAddr, u16), String> {
    let trimmed = server.trim();
    if trimmed.is_empty() {
        return Err("DNS server is empty".to_string());
    }

    if let Ok(ip) = trimmed.parse::<IpAddr>() {
        return Ok((ip, 53));
    }

    if let Some((host, port_text)) = trimmed.rsplit_once(':') {
        let ip = host
            .trim_matches(['[', ']'])
            .parse::<IpAddr>()
            .map_err(|_| "DNS server must be an IP address".to_string())?;
        let port = port_text
            .parse::<u16>()
            .map_err(|_| "DNS server port is invalid".to_string())?;
        return Ok((ip, port));
    }

    Err("DNS server must be an IP address".to_string())
}

fn display_dns_value(data: &hickory_resolver::proto::rr::RData) -> String {
    match data {
        hickory_resolver::proto::rr::RData::TXT(txt) => txt
            .txt_data
            .iter()
            .map(|part| String::from_utf8_lossy(part).into_owned())
            .collect::<Vec<_>>()
            .join(""),
        _ => data.to_string(),
    }
}

fn name_server(ip: IpAddr, port: u16) -> NameServerConfig {
    let mut udp = ConnectionConfig::udp();
    udp.port = port;
    let mut tcp = ConnectionConfig::tcp();
    tcp.port = port;
    NameServerConfig::new(ip, true, vec![udp, tcp])
}

fn system_name_servers(app: &tauri::AppHandle) -> Vec<NameServerConfig> {
    let reported = crate::native::dns_servers(app);
    let servers: Vec<IpAddr> = reported
        .iter()
        .filter_map(|s| s.trim().trim_start_matches('/').parse().ok())
        .collect();
    let servers = if servers.is_empty() {
        FALLBACK_DNS.iter().filter_map(|s| s.parse().ok()).collect()
    } else {
        servers
    };
    servers.into_iter().map(|ip| name_server(ip, 53)).collect()
}

#[tauri::command]
pub async fn dns_query(
    app: tauri::AppHandle,
    domain: String,
    record_type: String,
    server: Option<String>,
) -> Result<DnsQueryResult, String> {
    let domain = domain.trim().trim_end_matches('.').to_string();
    if domain.is_empty() {
        return Err("domain is empty".to_string());
    }

    let record_type = parse_record_type(&record_type)?;
    let server = server.and_then(|s| {
        let trimmed = s.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });

    let name_servers = if let Some(server_text) = server.as_deref() {
        let (ip, port) = parse_dns_server(server_text)?;
        vec![name_server(ip, port)]
    } else {
        system_name_servers(&app)
    };
    let config = ResolverConfig::from_parts(None, vec![], name_servers);
    let resolver = Resolver::builder_with_config(config, TokioRuntimeProvider::default())
        .with_options(ResolverOpts::default())
        .build()
        .map_err(|e| e.to_string())?;

    let lookup = resolver
        .lookup(format!("{}.", domain), record_type)
        .await
        .map_err(|e| e.to_string())?;

    let records = lookup
        .answers()
        .iter()
        .filter_map(|record| {
            let data = &record.data;
            Some(DnsRecord {
                record_type: data.record_type().to_string(),
                name: record.name.to_string(),
                ttl: record.ttl,
                value: display_dns_value(data),
                ip: data.ip_addr().map(|ip| ip.to_string()),
            })
        })
        .collect::<Vec<_>>();

    Ok(DnsQueryResult {
        source: if server.is_some() {
            "custom".to_string()
        } else {
            "system".to_string()
        },
        server,
        records,
    })
}
