use std::collections::HashSet;

const NODE_PREFIXES: &[&str] = &[
    "ss://",
    "ssr://",
    "vmess://",
    "vless://",
    "trojan://",
    "hysteria://",
    "hysteria2://",
    "tuic://",
    "hy2://",
];

pub struct NodeStats {
    pub nodes: Vec<String>,
    pub protocols: Vec<(String, usize)>,
}

pub fn parse_v2ray_content(raw: &str) -> NodeStats {
    let lines = extract_node_lines(raw);
    let mut seen = HashSet::new();
    let mut nodes = Vec::new();

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || !looks_like_node(trimmed) {
            continue;
        }
        if seen.insert(trimmed.to_string()) {
            nodes.push(trimmed.to_string());
        }
    }

    let protocols = count_protocols(&nodes);
    NodeStats { nodes, protocols }
}

fn extract_node_lines(raw: &str) -> Vec<String> {
    let trimmed = raw.trim();

    // 尝试 base64 解码
    if let Ok(decoded) = base64_decode(trimmed) {
        let decoded = decoded.trim().to_string();
        if decoded.lines().any(|l| looks_like_node(l.trim())) {
            return decoded.lines().map(str::trim).map(String::from).collect();
        }
    }

    // 纯文本，一行一个节点
    trimmed.lines().map(str::trim).map(String::from).collect()
}

fn base64_decode(input: &str) -> Result<String, base64::DecodeError> {
    use base64::Engine;
    let cleaned: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = base64::engine::general_purpose::STANDARD.decode(cleaned)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn looks_like_node(line: &str) -> bool {
    NODE_PREFIXES.iter().any(|p| line.starts_with(p))
}

pub fn count_protocols(nodes: &[String]) -> Vec<(String, usize)> {
    let mut counts = std::collections::BTreeMap::new();
    for node in nodes {
        let proto = NODE_PREFIXES
            .iter()
            .find(|p| node.starts_with(**p))
            .map(|p| p.trim_end_matches("://").to_string())
            .unwrap_or_else(|| "unknown".to_string());
        *counts.entry(proto).or_insert(0) += 1;
    }
    counts.into_iter().collect()
}

pub fn merge_v2ray_nodes(all_nodes: &[String]) -> String {
    let body = all_nodes.join("\n");
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(body)
}

pub fn merge_clash_yaml(chunks: &[String]) -> String {
    let mut proxies = Vec::new();
    let mut seen = HashSet::new();

    for chunk in chunks {
        if let Some(section) = extract_clash_proxies(chunk) {
            for block in section {
                let key = block.lines().next().unwrap_or("").to_string();
                if seen.insert(key.clone()) {
                    proxies.push(block);
                }
            }
        }
    }

    if proxies.is_empty() {
        return String::new();
    }

    let mut out = String::from("proxies:\n");
    for block in proxies {
        for line in block.lines() {
            out.push_str("  ");
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

pub fn extract_clash_proxy_blocks(yaml: &str) -> Option<Vec<String>> {
    extract_clash_proxies(yaml)
}

pub fn build_clash_proxies(blocks: &[String]) -> String {
    let mut out = String::from("proxies:\n");
    for block in blocks {
        for line in block.lines() {
            out.push_str("  ");
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn extract_clash_proxies(yaml: &str) -> Option<Vec<String>> {
    let lower = yaml.to_lowercase();
    if !lower.contains("proxies:") {
        return None;
    }

    let mut blocks = Vec::new();
    let mut in_proxies = false;
    let mut current = String::new();

    for line in yaml.lines() {
        if line.starts_with("proxies:") {
            in_proxies = true;
            continue;
        }
        if in_proxies {
            if line.starts_with("  - ") && !current.is_empty() {
                blocks.push(current.trim_end().to_string());
                current.clear();
            }
            if line.starts_with("  ") {
                current.push_str(line);
                current.push('\n');
            } else if !line.trim().is_empty() && !line.starts_with('#') {
                break;
            }
        }
    }
    if !current.is_empty() {
        blocks.push(current.trim_end().to_string());
    }

    if blocks.is_empty() {
        None
    } else {
        Some(blocks)
    }
}

pub fn validate_clash(raw: &str) -> bool {
    raw.len() > 50 && raw.to_lowercase().contains("proxies:")
}
