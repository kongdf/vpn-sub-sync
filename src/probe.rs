use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use tokio::net::TcpStream;
use tokio::sync::Semaphore;
use tokio::time::timeout;

#[derive(Debug, Clone, Copy)]
pub struct ProbeResult {
    pub reachable: bool,
    pub latency_ms: Option<u32>,
}

pub type ProbeCache = HashMap<(String, u16), ProbeResult>;

#[derive(Debug, Clone)]
pub struct ProbeConfig {
    pub enabled: bool,
    pub timeout_secs: u64,
    pub concurrency: usize,
}

#[derive(Debug, Clone, Default)]
pub struct ProbeStats {
    pub before: usize,
    pub after: usize,
    pub reachable: usize,
    pub unreachable: usize,
    pub unparsed: usize,
}

pub async fn filter_v2ray_nodes(
    nodes: &[String],
    cfg: &ProbeConfig,
) -> (Vec<String>, ProbeStats, ProbeCache) {
    let mut stats = ProbeStats {
        before: nodes.len(),
        ..Default::default()
    };

    if !cfg.enabled || nodes.is_empty() {
        stats.after = nodes.len();
        return (nodes.to_vec(), stats, ProbeCache::new());
    }

    let timeout = Duration::from_secs(cfg.timeout_secs);
    let cache = probe_endpoints(collect_v2ray_endpoints(nodes), cfg, timeout).await;

    let mut filtered = Vec::new();
    for node in nodes {
        match extract_v2ray_endpoint(node) {
            Some(ep) => {
                if cache.get(&ep).map(|r| r.reachable).unwrap_or(false) {
                    stats.reachable += 1;
                    filtered.push(node.clone());
                } else {
                    stats.unreachable += 1;
                }
            }
            None => {
                stats.unparsed += 1;
                filtered.push(node.clone());
            }
        }
    }

    stats.after = filtered.len();
    (filtered, stats, cache)
}

pub async fn filter_clash_chunks(
    chunks: &[String],
    cfg: &ProbeConfig,
) -> (Vec<String>, ProbeStats, ProbeCache) {
    let mut stats = ProbeStats::default();
    let mut cache = ProbeCache::new();

    if !cfg.enabled || chunks.is_empty() {
        return (chunks.to_vec(), stats, cache);
    }

    let timeout = Duration::from_secs(cfg.timeout_secs);
    let mut filtered_chunks = Vec::new();

    for chunk in chunks {
        let blocks = match crate::parser::extract_clash_proxy_blocks(chunk) {
            Some(blocks) => blocks,
            None => {
                filtered_chunks.push(chunk.clone());
                continue;
            }
        };

        stats.before += blocks.len();
        let endpoints: Vec<(String, u16)> = blocks
            .iter()
            .filter_map(|b| extract_clash_endpoint(b))
            .collect();
        let chunk_cache = probe_endpoints(endpoints, cfg, timeout).await;
        cache.extend(chunk_cache);

        let mut kept = Vec::new();
        for block in blocks {
            match extract_clash_endpoint(&block) {
                Some(ep) => {
                    if cache.get(&ep).map(|r| r.reachable).unwrap_or(false) {
                        stats.reachable += 1;
                        kept.push(block);
                    } else {
                        stats.unreachable += 1;
                    }
                }
                None => {
                    stats.unparsed += 1;
                    kept.push(block);
                }
            }
        }

        stats.after += kept.len();
        if !kept.is_empty() {
            filtered_chunks.push(crate::parser::build_clash_proxies(&kept));
        }
    }

    (filtered_chunks, stats, cache)
}

fn collect_v2ray_endpoints(nodes: &[String]) -> Vec<(String, u16)> {
    nodes.iter().filter_map(|n| extract_v2ray_endpoint(n)).collect()
}

async fn probe_endpoints(
    endpoints: Vec<(String, u16)>,
    cfg: &ProbeConfig,
    probe_timeout: Duration,
) -> ProbeCache {
    let unique: Vec<(String, u16)> = endpoints
        .into_iter()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    let sem = std::sync::Arc::new(Semaphore::new(cfg.concurrency.max(1)));
    let mut tasks = Vec::new();

    for (host, port) in unique {
        let sem = sem.clone();
        tasks.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();
            let result = tcp_probe(&host, port, probe_timeout).await;
            ((host, port), result)
        }));
    }

    let mut cache = ProbeCache::new();
    for task in tasks {
        if let Ok((ep, result)) = task.await {
            cache.insert(ep, result);
        }
    }
    cache
}

async fn tcp_probe(host: &str, port: u16, probe_timeout: Duration) -> ProbeResult {
    let started = Instant::now();
    let addrs = match tokio::net::lookup_host((host, port)).await {
        Ok(addrs) => addrs.collect::<Vec<SocketAddr>>(),
        Err(_) => {
            return ProbeResult {
                reachable: false,
                latency_ms: None,
            }
        }
    };

    for addr in addrs {
        if timeout(probe_timeout, TcpStream::connect(addr))
            .await
            .ok()
            .and_then(|r| r.ok())
            .is_some()
        {
            let ms = started.elapsed().as_millis().min(u128::from(u32::MAX)) as u32;
            return ProbeResult {
                reachable: true,
                latency_ms: Some(ms),
            };
        }
    }

    ProbeResult {
        reachable: false,
        latency_ms: None,
    }
}

pub fn extract_v2ray_endpoint(node: &str) -> Option<(String, u16)> {
    let trimmed = node.trim();
    if trimmed.starts_with("ss://") {
        return parse_ss(trimmed);
    }
    if trimmed.starts_with("vmess://") {
        return parse_vmess(trimmed);
    }
    if trimmed.starts_with("vless://") {
        return parse_uri_userinfo_host_port(trimmed, "vless://");
    }
    if trimmed.starts_with("trojan://") {
        return parse_uri_userinfo_host_port(trimmed, "trojan://");
    }
    if trimmed.starts_with("hysteria2://") {
        return parse_uri_authority(trimmed.strip_prefix("hysteria2://")?);
    }
    if trimmed.starts_with("hy2://") {
        return parse_uri_authority(trimmed.strip_prefix("hy2://")?);
    }
    if trimmed.starts_with("hysteria://") {
        return parse_hysteria(trimmed);
    }
    if trimmed.starts_with("tuic://") {
        return parse_uri_userinfo_host_port(trimmed, "tuic://");
    }
    if trimmed.starts_with("ssr://") {
        return parse_ssr(trimmed);
    }
    None
}

pub fn extract_clash_endpoint(block: &str) -> Option<(String, u16)> {
    let mut server = None;
    let mut port = None;

    for line in block.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("server:") {
            server = Some(v.trim().trim_matches('"').trim_matches('\'').to_string());
        } else if let Some(v) = line.strip_prefix("port:") {
            port = v.trim().parse().ok();
        }
    }

    match (server, port) {
        (Some(host), Some(port)) if !host.is_empty() && port > 0 => Some((host, port)),
        _ => None,
    }
}

fn parse_ss(url: &str) -> Option<(String, u16)> {
    let rest = url.strip_prefix("ss://")?;
    let body = rest.split('#').next()?.split('?').next()?;

    if body.contains('@') {
        return parse_userinfo_host_port(body);
    }

    let decoded = base64_decode(body)?;
    parse_userinfo_host_port(&decoded)
}

fn parse_vmess(url: &str) -> Option<(String, u16)> {
    let body = url.strip_prefix("vmess://")?.split('#').next()?;
    let json = base64_decode(body)?;
    let value: serde_json::Value = serde_json::from_str(&json).ok()?;
    let host = value.get("add")?.as_str()?.to_string();
    let port = value.get("port").and_then(|p| {
        p.as_u64()
            .map(|n| n as u16)
            .or_else(|| p.as_str()?.parse().ok())
    })?;
    Some((host, port))
}

fn parse_uri_userinfo_host_port(url: &str, scheme: &str) -> Option<(String, u16)> {
    let rest = url.strip_prefix(scheme)?.split('#').next()?.split('?').next()?;
    parse_uri_authority(rest)
}

fn parse_uri_authority(rest: &str) -> Option<(String, u16)> {
    let hostport = rest.split('#').next()?.split('?').next()?.rsplit('@').next()?;
    parse_host_port(hostport)
}

fn parse_hysteria(url: &str) -> Option<(String, u16)> {
    let rest = url.strip_prefix("hysteria://")?.split('#').next()?.split('?').next()?;
    if rest.contains('@') {
        return parse_userinfo_host_port(rest);
    }
    parse_host_port(rest)
}

fn parse_ssr(url: &str) -> Option<(String, u16)> {
    let body = url.strip_prefix("ssr://")?.split('/').next()?;
    let decoded = base64_decode(body)?;
    let main = decoded.split('/').next()?;
    let mut parts = main.split(':');
    let host = parts.next()?.to_string();
    let port: u16 = parts.next()?.parse().ok()?;
    if host.is_empty() {
        return None;
    }
    Some((host, port))
}

fn parse_userinfo_host_port(text: &str) -> Option<(String, u16)> {
    let hostport = text.rsplit('@').next()?;
    parse_host_port(hostport)
}

fn parse_host_port(hostport: &str) -> Option<(String, u16)> {
    let hostport = hostport.trim();
    if hostport.starts_with('[') {
        let end = hostport.find(']')?;
        let host = hostport[1..end].to_string();
        let port = hostport.get(end + 2..)?.parse().ok()?;
        return Some((host, port));
    }

    let idx = hostport.rfind(':')?;
    let host = hostport[..idx].trim().to_string();
    let port = hostport[idx + 1..].trim().parse().ok()?;
    if host.is_empty() {
        return None;
    }
    Some((host, port))
}

fn base64_decode(input: &str) -> Option<String> {
    use base64::Engine;
    let cleaned: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(cleaned)
        .ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_vless_endpoint() {
        let ep = extract_v2ray_endpoint(
            "vless://uuid@example.com:443?encryption=none&security=tls",
        );
        assert_eq!(ep, Some(("example.com".into(), 443)));
    }

    #[test]
    fn parse_vmess_endpoint() {
        use base64::Engine;
        let json = r#"{"add":"1.2.3.4","port":"8443"}"#;
        let b64 = base64::engine::general_purpose::STANDARD.encode(json);
        let node = format!("vmess://{b64}");
        assert_eq!(extract_v2ray_endpoint(&node), Some(("1.2.3.4".into(), 8443)));
    }

    #[test]
    fn parse_clash_endpoint() {
        let block = "  - name: test\n    type: ss\n    server: node.example.com\n    port: 8388\n";
        assert_eq!(
            extract_clash_endpoint(block),
            Some(("node.example.com".into(), 8388))
        );
    }
}
