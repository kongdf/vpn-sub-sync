use std::collections::HashMap;

use crate::probe::{extract_clash_endpoint, extract_v2ray_endpoint, ProbeCache, ProbeResult};

fn node_endpoint(node: &str) -> Option<(String, u16)> {
    extract_v2ray_endpoint(node).or_else(|| extract_clash_endpoint(node))
}

#[derive(Debug, Clone)]
pub struct FilterConfig {
    pub dedupe_endpoint: bool,
    pub max_latency_ms: Option<u32>,
    pub max_nodes: Option<usize>,
    pub drop_unparsed: bool,
}

impl FilterConfig {
    pub fn enabled(&self) -> bool {
        self.dedupe_endpoint
            || self.max_latency_ms.is_some()
            || self.max_nodes.is_some()
            || self.drop_unparsed
    }
}

#[derive(Debug, Clone, Default)]
pub struct FilterStats {
    pub before: usize,
    pub after: usize,
    pub deduped: usize,
    pub slow_dropped: usize,
    pub unparsed_dropped: usize,
    pub capped: usize,
}

pub fn refine_v2ray_nodes(
    nodes: &[String],
    probe_cache: &ProbeCache,
    cfg: &FilterConfig,
) -> (Vec<String>, FilterStats) {
    let mut stats = FilterStats {
        before: nodes.len(),
        ..Default::default()
    };

    if nodes.is_empty() {
        return (vec![], stats);
    }

    let mut kept = nodes.to_vec();

    if cfg.drop_unparsed {
        let before = kept.len();
        kept.retain(|node| {
            if node_endpoint(node).is_some() {
                true
            } else {
                stats.unparsed_dropped += 1;
                false
            }
        });
        stats.after = kept.len();
        if kept.len() != before {
            stats.before = stats.before; // keep original before
        }
    }

    if cfg.max_latency_ms.is_some() {
        let max = cfg.max_latency_ms.unwrap();
        kept.retain(|node| {
            let Some(ep) = node_endpoint(node) else {
                return true;
            };
            match probe_cache.get(&ep) {
                Some(ProbeResult {
                    latency_ms: Some(ms),
                    ..
                }) if *ms <= max => true,
                Some(ProbeResult {
                    reachable: true,
                    latency_ms: None,
                    ..
                }) => true,
                Some(_) => {
                    stats.slow_dropped += 1;
                    false
                }
                None => true,
            }
        });
    }

    if cfg.dedupe_endpoint {
        kept = dedupe_by_endpoint(kept, probe_cache, &mut stats);
    }

    kept.sort_by(|a, b| {
        endpoint_latency(a, probe_cache)
            .unwrap_or(u32::MAX)
            .cmp(&endpoint_latency(b, probe_cache).unwrap_or(u32::MAX))
    });

    if let Some(max) = cfg.max_nodes {
        if kept.len() > max {
            stats.capped = kept.len() - max;
            kept.truncate(max);
        }
    }

    stats.after = kept.len();
    (kept, stats)
}

pub fn refine_clash_chunks(
    chunks: &[String],
    probe_cache: &ProbeCache,
    cfg: &FilterConfig,
) -> (Vec<String>, FilterStats) {
    let mut stats = FilterStats::default();
    let mut out = Vec::new();

    for chunk in chunks {
        let Some(blocks) = crate::parser::extract_clash_proxy_blocks(chunk) else {
            out.push(chunk.clone());
            continue;
        };

        stats.before += blocks.len();
        let (refined, block_stats) = refine_clash_blocks(&blocks, probe_cache, cfg);
        merge_filter_stats(&mut stats, &block_stats);
        stats.after += refined.len();

        if !refined.is_empty() {
            out.push(crate::parser::build_clash_proxies(&refined));
        }
    }

    (out, stats)
}

fn refine_clash_blocks(
    blocks: &[String],
    probe_cache: &ProbeCache,
    cfg: &FilterConfig,
) -> (Vec<String>, FilterStats) {
    let mut stats = FilterStats {
        before: blocks.len(),
        ..Default::default()
    };

    let pseudo_nodes: Vec<String> = blocks.to_vec();
    let (kept, node_stats) = refine_v2ray_nodes(&pseudo_nodes, probe_cache, cfg);
    stats.deduped = node_stats.deduped;
    stats.slow_dropped = node_stats.slow_dropped;
    stats.unparsed_dropped = node_stats.unparsed_dropped;
    stats.capped = node_stats.capped;
    stats.after = kept.len();
    (kept, stats)
}

fn dedupe_by_endpoint(
    nodes: Vec<String>,
    probe_cache: &ProbeCache,
    stats: &mut FilterStats,
) -> Vec<String> {
    let mut unparsed = Vec::new();
    let mut best: HashMap<(String, u16), (String, u32)> = HashMap::new();

    for node in nodes {
        let Some(ep) = node_endpoint(&node) else {
            unparsed.push(node);
            continue;
        };
        let rank = endpoint_latency(&node, probe_cache).unwrap_or(u32::MAX);

        match best.get(&ep) {
            Some((_, existing)) if *existing <= rank => {
                stats.deduped += 1;
            }
            _ => {
                if best.contains_key(&ep) {
                    stats.deduped += 1;
                }
                best.insert(ep, (node, rank));
            }
        }
    }

    let mut ranked: Vec<(String, u32)> = best.into_values().collect();
    ranked.sort_by_key(|(_, rank)| *rank);

    let mut out = unparsed;
    out.extend(ranked.into_iter().map(|(node, _)| node));
    out
}

fn endpoint_latency(node: &str, probe_cache: &ProbeCache) -> Option<u32> {
    let ep = node_endpoint(node)?;
    probe_cache.get(&ep).and_then(|r| r.latency_ms)
}

fn merge_filter_stats(total: &mut FilterStats, part: &FilterStats) {
    total.deduped += part.deduped;
    total.slow_dropped += part.slow_dropped;
    total.unparsed_dropped += part.unparsed_dropped;
    total.capped += part.capped;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::ProbeResult;

    fn cache() -> ProbeCache {
        let mut c = ProbeCache::new();
        c.insert(
            ("1.1.1.1".into(), 443),
            ProbeResult {
                reachable: true,
                latency_ms: Some(100),
            },
        );
        c.insert(
            ("2.2.2.2".into(), 443),
            ProbeResult {
                reachable: true,
                latency_ms: Some(2500),
            },
        );
        c
    }

    #[test]
    fn dedupes_same_endpoint() {
        let nodes = vec![
            "vless://a@1.1.1.1:443#n1".into(),
            "vless://b@1.1.1.1:443#n2".into(),
        ];
        let cfg = FilterConfig {
            dedupe_endpoint: true,
            max_latency_ms: None,
            max_nodes: None,
            drop_unparsed: false,
        };
        let (out, stats) = refine_v2ray_nodes(&nodes, &cache(), &cfg);
        assert_eq!(out.len(), 1);
        assert_eq!(stats.deduped, 1);
    }

    #[test]
    fn drops_slow_nodes() {
        let nodes = vec![
            "vless://a@1.1.1.1:443#fast".into(),
            "vless://b@2.2.2.2:443#slow".into(),
        ];
        let cfg = FilterConfig {
            dedupe_endpoint: false,
            max_latency_ms: Some(2000),
            max_nodes: None,
            drop_unparsed: false,
        };
        let (out, stats) = refine_v2ray_nodes(&nodes, &cache(), &cfg);
        assert_eq!(out.len(), 1);
        assert_eq!(stats.slow_dropped, 1);
    }
}
