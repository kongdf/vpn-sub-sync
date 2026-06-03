use std::collections::HashSet;

use crate::country::detect_country;
use crate::probe::{extract_clash_endpoint, extract_v2ray_endpoint, ProbeCache};
use crate::tag::{TaggedChunk, TaggedNode};

#[derive(Debug, Clone)]
pub struct NamingConfig {
    pub enabled: bool,
    pub template: String,
    pub first_name: String,
    pub tag_source: bool,
}

pub fn rename_v2ray_tagged(
    nodes: &[TaggedNode],
    cfg: &NamingConfig,
    probe_cache: &ProbeCache,
) -> Vec<TaggedNode> {
    if !cfg.enabled {
        if !cfg.tag_source {
            return nodes.to_vec();
        }
        return nodes
            .iter()
            .map(|t| TaggedNode::new(tag_source_only(&t.node, &t.source), t.source.clone()))
            .collect();
    }

    let mut used = HashSet::new();
    nodes
        .iter()
        .enumerate()
        .map(|(i, tagged)| {
            let base = if i == 0 {
                cfg.first_name.clone()
            } else {
                build_name(cfg, &tagged.node, &tagged.source, i + 1, probe_cache)
            };
            let name = unique_name(&mut used, base);
            TaggedNode::new(
                set_v2ray_display_name(&tagged.node, &name),
                tagged.source.clone(),
            )
        })
        .collect()
}

pub fn rename_v2ray_nodes(
    nodes: &[String],
    cfg: &NamingConfig,
    probe_cache: &ProbeCache,
) -> Vec<String> {
    let tagged: Vec<TaggedNode> = nodes
        .iter()
        .map(|node| TaggedNode::new(node.clone(), ""))
        .collect();
    rename_v2ray_tagged(&tagged, cfg, probe_cache)
        .into_iter()
        .map(|t| t.node)
        .collect()
}

pub fn rename_clash_tagged(
    chunks: &[TaggedChunk],
    cfg: &NamingConfig,
    probe_cache: &ProbeCache,
) -> Vec<TaggedChunk> {
    if !cfg.enabled {
        if !cfg.tag_source {
            return chunks.to_vec();
        }
        return chunks
            .iter()
            .map(|chunk| {
                let Some(blocks) = crate::parser::extract_clash_proxy_blocks(&chunk.body) else {
                    return chunk.clone();
                };
                let renamed: Vec<String> = blocks
                    .iter()
                    .map(|block| {
                        set_clash_display_name(block, &format!("{}-{}", chunk.source, extract_original_display_name(block).unwrap_or_else(|| "node".into())))
                    })
                    .collect();
                TaggedChunk::new(
                    crate::parser::build_clash_proxies(&renamed),
                    chunk.source.clone(),
                )
            })
            .collect();
    }

    let mut used = HashSet::new();
    let mut out = Vec::new();

    for chunk in chunks {
        let Some(blocks) = crate::parser::extract_clash_proxy_blocks(&chunk.body) else {
            out.push(chunk.clone());
            continue;
        };

        let renamed: Vec<String> = blocks
            .iter()
            .enumerate()
            .map(|(i, block)| {
                let name = unique_name(
                    &mut used,
                    build_name(cfg, block, &chunk.source, i + 1, probe_cache),
                );
                set_clash_display_name(block, &name)
            })
            .collect();

        out.push(TaggedChunk::new(
            crate::parser::build_clash_proxies(&renamed),
            chunk.source.clone(),
        ));
    }

    out
}

pub fn rename_clash_chunks(
    chunks: &[String],
    cfg: &NamingConfig,
    probe_cache: &ProbeCache,
) -> Vec<String> {
    let tagged: Vec<TaggedChunk> = chunks
        .iter()
        .map(|body| TaggedChunk::new(body.clone(), ""))
        .collect();
    rename_clash_tagged(&tagged, cfg, probe_cache)
        .into_iter()
        .map(|c| c.body)
        .collect()
}

fn tag_source_only(node: &str, source: &str) -> String {
    let label = if source.is_empty() { "unknown" } else { source };
    let original = extract_original_display_name(node).unwrap_or_default();
    let name = if original.is_empty() {
        label.to_string()
    } else {
        format!("{label}-{original}")
    };
    set_v2ray_display_name(node, &name)
}

fn build_name(
    cfg: &NamingConfig,
    node: &str,
    source: &str,
    index: usize,
    probe_cache: &ProbeCache,
) -> String {
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
    let source_label = if source.is_empty() {
        "unknown".to_string()
    } else {
        source.to_string()
    };

    apply_template(
        &cfg.template,
        &[
            ("source", &source_label),
            ("country", &country),
            ("latency", &latency),
            ("proto", &proto),
            ("host", &host),
            ("port", &port_str),
            ("index", &index_str),
        ],
    )
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
    if trimmed.starts_with("ssr://") {
        return set_ssr_display_name(trimmed, display_name);
    }

    let base = trimmed.split('#').next().unwrap_or(trimmed);
    format!("{}#{}", base, format_v2ray_alias(display_name))
}

fn format_v2ray_alias(name: &str) -> String {
    // v2rayN / v2rayNG 直接支持 UTF-8 别名，只需转义 fragment 保留字
    name.replace('%', "%25").replace('#', "%23")
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

fn set_ssr_display_name(url: &str, display_name: &str) -> String {
    let payload = match url.strip_prefix("ssr://") {
        Some(p) => p.split('#').next().unwrap_or(p),
        None => return url.to_string(),
    };

    let Some(decoded) = base64_decode(payload) else {
        return url.to_string();
    };

    let remarks_b64 = base64_encode(display_name);
    let new_inner = if let Some(pos) = decoded.find("/?") {
        let (main, query) = decoded.split_at(pos);
        let query = query.trim_start_matches("/?");
        format!("{main}/?{}", upsert_query_param(query, "remarks", &remarks_b64))
    } else if let Some(pos) = decoded.find('?') {
        let (main, query) = decoded.split_at(pos);
        let query = query.trim_start_matches('?');
        format!("{main}?{}", upsert_query_param(query, "remarks", &remarks_b64))
    } else {
        format!("{decoded}/?remarks={remarks_b64}")
    };

    format!("ssr://{}", base64_encode(&new_inner))
}

fn upsert_query_param(query: &str, key: &str, value: &str) -> String {
    let prefix = format!("{key}=");
    let mut parts: Vec<String> = query
        .split('&')
        .filter(|p| !p.is_empty() && !p.starts_with(&prefix))
        .map(|p| p.to_string())
        .collect();
    parts.push(format!("{prefix}{value}"));
    parts.join("&")
}

fn base64_encode(input: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(input.as_bytes())
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
            tag_source: true,
        }
    }

    #[test]
    fn first_node_uses_plain_utf8_alias() {
        let cache = HashMap::new();
        let nodes = vec![
            "vless://a@1.2.3.4:443#x".to_string(),
            "vless://b@1.2.3.5:443#y".to_string(),
        ];
        let out = rename_v2ray_nodes(&nodes, &cfg(), &cache);
        assert_eq!(
            out[0].split('#').nth(1).unwrap(),
            "孔大夫-我做个艺术家"
        );
    }

    #[test]
    fn renames_ssr_remarks() {
        use base64::Engine;
        let inner = "1.2.3.4:8388:origin:aes-256-cfb:plain:/?remarks=";
        let old_remarks = base64::engine::general_purpose::STANDARD.encode("old-name");
        let payload = base64::engine::general_purpose::STANDARD
            .encode(format!("{inner}{old_remarks}"));
        let node = format!("ssr://{payload}");
        let mut cache = HashMap::new();
        cache.insert(
            ("1.2.3.4".into(), 8388),
            ProbeResult {
                reachable: true,
                latency_ms: Some(42),
            },
        );
        let out = rename_v2ray_nodes(&[node], &cfg(), &cache);
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(out[0].strip_prefix("ssr://").unwrap())
            .unwrap();
        let text = String::from_utf8_lossy(&decoded);
        assert!(text.contains("remarks="));
        let remarks = text
            .split("remarks=")
            .nth(1)
            .unwrap()
            .split('&')
            .next()
            .unwrap();
        let name = String::from_utf8(
            base64::engine::general_purpose::STANDARD
                .decode(remarks)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(name, "孔大夫-我做个艺术家");
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
        assert_eq!(out[1].split('#').nth(1).unwrap(), "香港-86ms");
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
        assert_eq!(out[0].split('#').nth(1).unwrap(), "孔大夫-我做个艺术家");
        let n2 = out[1].split('#').nth(1).unwrap();
        let n3 = out[2].split('#').nth(1).unwrap();
        assert_ne!(n2, n3);
    }

    #[test]
    fn includes_source_in_name() {
        let mut cache = HashMap::new();
        cache.insert(
            ("1.2.3.4".into(), 443),
            ProbeResult {
                reachable: true,
                latency_ms: Some(42),
            },
        );
        let cfg = NamingConfig {
            enabled: true,
            template: "{source}-{country}-{latency}".to_string(),
            first_name: "first".to_string(),
            tag_source: true,
        };
        let nodes = vec![
            TaggedNode::new("vless://a@9.9.9.9:443#x".to_string(), "src-a"),
            TaggedNode::new("vless://b@1.2.3.4:443#y".to_string(), "nodev2rayn"),
        ];
        let out = rename_v2ray_tagged(&nodes, &cfg, &cache);
        assert_eq!(out[1].node.split('#').nth(1).unwrap(), "nodev2rayn-香港-42ms");
    }
}
