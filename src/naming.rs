use std::collections::HashSet;

use crate::country::detect_country;
use crate::probe::{extract_clash_endpoint, extract_v2ray_endpoint, ProbeCache};

#[derive(Debug, Clone)]
pub struct NamingConfig {
    pub enabled: bool,
    pub template: String,
    pub first_name: String,
}

pub fn rename_v2ray_nodes(
    nodes: &[String],
    cfg: &NamingConfig,
    probe_cache: &ProbeCache,
) -> Vec<String> {
    if !cfg.enabled {
        return nodes.to_vec();
    }

    let mut used = HashSet::new();
    nodes
        .iter()
        .enumerate()
        .map(|(i, node)| {
            let base = if i == 0 {
                cfg.first_name.clone()
            } else {
                build_name(cfg, node, i + 1, probe_cache)
            };
            let name = unique_name(&mut used, base);
            set_v2ray_display_name(node, &name)
        })
        .collect()
}

pub fn rename_clash_chunks(
    chunks: &[String],
    cfg: &NamingConfig,
    probe_cache: &ProbeCache,
) -> Vec<String> {
    if !cfg.enabled {
        return chunks.to_vec();
    }

    let mut used = HashSet::new();
    let mut out = Vec::new();

    for chunk in chunks {
        let Some(blocks) = crate::parser::extract_clash_proxy_blocks(chunk) else {
            out.push(chunk.clone());
            continue;
        };

        let renamed: Vec<String> = blocks
            .iter()
            .enumerate()
            .map(|(i, block)| {
                let name = unique_name(
                    &mut used,
                    build_name(cfg, block, i + 1, probe_cache),
                );
                set_clash_display_name(block, &name)
            })
            .collect();

        out.push(crate::parser::build_clash_proxies(&renamed));
    }

    out
}

fn unique_name(used: &mut HashSet<String>, base: String) -> String {
    if used.insert(base.clone()) {
        return base;
    }

    let mut n = 2;
    loop {
        let candidate = format!("{base}-{n}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
        n += 1;
    }
}

fn build_name(cfg: &NamingConfig, node: &str, index: usize, probe_cache: &ProbeCache) -> String {
    let hint = extract_original_display_name(node);
    let (host, port) = extract_v2ray_endpoint(node)
        .or_else(|| extract_clash_endpoint(node))
        .unwrap_or_else(|| ("?".into(), 0));

    let country = detect_country(&host, hint.as_deref());
    let latency = format_latency(probe_cache.get(&(host.clone(), port)));
    let proto = node_protocol(node);
    let port_str = if port > 0 {
        port.to_string()
    } else {
        "?".to_string()
    };
    let index_str = format!("{index:03}");

    apply_template(
        &cfg.template,
        &[
            ("country", &country),
            ("latency", &latency),
            ("proto", &proto),
            ("host", &host),
            ("port", &port_str),
            ("index", &index_str),
        ],
    )
}

fn format_latency(result: Option<&crate::probe::ProbeResult>) -> String {
    match result.and_then(|r| r.latency_ms) {
        Some(ms) => format!("{ms}ms"),
        None => "-".to_string(),
    }
}

fn extract_original_display_name(node: &str) -> Option<String> {
    let trimmed = node.trim();

    if trimmed.starts_with("vmess://") {
        let body = trimmed.strip_prefix("vmess://")?.split('#').next()?;
        let json = base64_decode(body)?;
        let value: serde_json::Value = serde_json::from_str(&json).ok()?;
        return value
            .get("ps")
            .and_then(|v| v.as_str())
            .map(str::to_string);
    }

    if let Some(fragment) = trimmed.split('#').nth(1) {
        let name = decode_fragment(fragment);
        if !name.is_empty() {
            return Some(name);
        }
    }

    for line in trimmed.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("- name:") {
            return Some(v.trim().trim_matches('"').trim_matches('\'').to_string());
        }
        if let Some(v) = line.strip_prefix("name:") {
            return Some(v.trim().trim_matches('"').trim_matches('\'').to_string());
        }
    }

    None
}

fn apply_template(template: &str, vars: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (key, value) in vars {
        out = out.replace(&format!("{{{key}}}"), value);
    }
    out
}

fn node_protocol(node: &str) -> String {
    const PREFIXES: &[(&str, &str)] = &[
        ("ss://", "ss"),
        ("ssr://", "ssr"),
        ("vmess://", "vmess"),
        ("vless://", "vless"),
        ("trojan://", "trojan"),
        ("hysteria2://", "hysteria2"),
        ("hy2://", "hy2"),
        ("hysteria://", "hysteria"),
        ("tuic://", "tuic"),
    ];

    let trimmed = node.trim();
    for (prefix, proto) in PREFIXES {
        if trimmed.starts_with(prefix) {
            return proto.to_string();
        }
    }

    for line in node.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("type:") {
            return v.trim().trim_matches('"').trim_matches('\'').to_string();
        }
    }

    "unknown".to_string()
}

fn set_v2ray_display_name(node: &str, display_name: &str) -> String {
    let trimmed = node.trim();
    if trimmed.starts_with("vmess://") {
        return set_vmess_display_name(trimmed, display_name);
    }

    let base = trimmed.split('#').next().unwrap_or(trimmed);
    format!("{}#{}", base, encode_fragment(display_name))
}

fn set_vmess_display_name(url: &str, display_name: &str) -> String {
    let body = match url.strip_prefix("vmess://") {
        Some(b) => b.split('#').next().unwrap_or(b),
        None => return url.to_string(),
    };

    let Some(json) = base64_decode(body) else {
        return url.to_string();
    };
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&json) else {
        return url.to_string();
    };

    if let serde_json::Value::Object(ref mut map) = value {
        map.insert(
            "ps".to_string(),
            serde_json::Value::String(display_name.to_string()),
        );
    }

    let updated = serde_json::to_string(&value).unwrap_or(json);
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(updated.as_bytes());
    format!("vmess://{encoded}")
}

fn set_clash_display_name(block: &str, display_name: &str) -> String {
    block
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with("- name:") {
                let indent = line.len().saturating_sub(line.trim_start().len());
                format!(
                    "{}- name: {}",
                    " ".repeat(indent),
                    clash_name_value(display_name)
                )
            } else if trimmed.starts_with("name:") {
                let indent = line.len().saturating_sub(line.trim_start().len());
                format!(
                    "{}name: {}",
                    " ".repeat(indent),
                    clash_name_value(display_name)
                )
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn clash_name_value(name: &str) -> String {
    if name.chars().any(|c| c == ':' || c == '#' || c == '"') {
        format!("\"{}\"", name.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        name.to_string()
    }
}

fn encode_fragment(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            _ => format!("%{:02X}", c as u32),
        })
        .collect()
}

fn decode_fragment(input: &str) -> String {
    let mut out = String::new();
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            let h1 = chars.next();
            let h2 = chars.next();
            if let (Some(a), Some(b)) = (h1, h2) {
                if let Ok(byte) = u8::from_str_radix(&format!("{a}{b}"), 16) {
                    out.push(byte as char);
                    continue;
                }
            }
            out.push('%');
            if let Some(a) = h1 {
                out.push(a);
            }
            if let Some(b) = h2 {
                out.push(b);
            }
        } else {
            out.push(c);
        }
    }
    out
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
    use crate::probe::ProbeResult;
    use std::collections::HashMap;

    fn cfg() -> NamingConfig {
        NamingConfig {
            enabled: true,
            template: "{country}-{latency}".to_string(),
            first_name: "孔大夫-我做个艺术家".to_string(),
        }
    }

    #[test]
    fn first_node_uses_custom_name() {
        let cache = HashMap::new();
        let nodes = vec![
            "vless://a@1.2.3.4:443#x".to_string(),
            "vless://b@1.2.3.5:443#y".to_string(),
        ];
        let out = rename_v2ray_nodes(&nodes, &cfg(), &cache);
        let n1 = decode_fragment(out[0].split('#').nth(1).unwrap());
        assert_eq!(n1, "孔大夫-我做个艺术家");
    }

    #[test]
    fn builds_country_latency_name() {
        let mut cache = HashMap::new();
        cache.insert(
            ("1.2.3.4".into(), 443),
            ProbeResult {
                reachable: true,
                latency_ms: Some(86),
            },
        );
        let node = "vless://uuid@1.2.3.4:443?security=tls#%F0%9F%87%AD%F0%9F%87%B0%E9%A6%99%E6%B8%AF";
        let nodes = vec![
            "vless://first@9.9.9.9:443#skip".to_string(),
            node.to_string(),
        ];
        let out = rename_v2ray_nodes(&nodes, &cfg(), &cache);
        let name = decode_fragment(out[1].split('#').nth(1).unwrap());
        assert_eq!(name, "香港-86ms");
    }

    #[test]
    fn deduplicates_same_name() {
        let mut cache = HashMap::new();
        cache.insert(
            ("1.2.3.4".into(), 443),
            ProbeResult {
                reachable: true,
                latency_ms: Some(50),
            },
        );
        cache.insert(
            ("1.2.3.5".into(), 443),
            ProbeResult {
                reachable: true,
                latency_ms: Some(50),
            },
        );
        let nodes = vec![
            "vless://a@1.2.3.4:443#HK".to_string(),
            "vless://b@1.2.3.5:443#HK".to_string(),
            "vless://c@1.2.3.6:443#HK".to_string(),
        ];
        let out = rename_v2ray_nodes(&nodes, &cfg(), &cache);
        let n1 = decode_fragment(out[0].split('#').nth(1).unwrap());
        let n2 = decode_fragment(out[1].split('#').nth(1).unwrap());
        let n3 = decode_fragment(out[2].split('#').nth(1).unwrap());
        assert_eq!(n1, "孔大夫-我做个艺术家");
        assert_ne!(n2, n3);
    }
}
