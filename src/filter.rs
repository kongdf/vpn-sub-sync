use std::collections::HashMap;

use crate::probe::{extract_clash_endpoint, extract_v2ray_endpoint, ProbeCache, ProbeResult};
use crate::tag::TaggedNode;

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
    let tagged: Vec<TaggedNode> = nodes
        .iter()
        .map(|node| TaggedNode::new(node.clone(), ""))
        .collect();
    let (out, stats) = refine_v2ray_tagged(&tagged, probe_cache, cfg);
    (out.into_iter().map(|t| t.node).collect(), stats)
}

pub fn refine_v2ray_tagged(
    nodes: &[TaggedNode],
    probe_cache: &ProbeCache,
    cfg: &FilterConfig,
) -> (Vec<TaggedNode>, FilterStats) {
    let mut stats = FilterStats {
        before: nodes.len(),
        ..Default::default()
    };

    if nodes.is_empty() {
        return (vec![], stats);
    }

    let mut kept = nodes.to_vec();

    if cfg.drop_unparsed {
        kept.retain(|tagged| {
            if node_endpoint(&tagged.node).is_some() {
                true
            } else {
                stats.unparsed_dropped += 1;
                false
            }
        });
    }

    if cfg.max_latency_ms.is_some() {
        let max = cfg.max_latency_ms.unwrap();
        kept.retain(|tagged| {
            let Some(ep) = node_endpoint(&tagged.node) else {
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
        kept = dedupe_tagged_by_endpoint(kept, probe_cache, &mut stats);
    }

    kept.sort_by(|a, b| {
        endpoint_latency(&a.node, probe_cache)
            .unwrap_or(u32::MAX)
            .cmp(&endpoint_latency(&b.node, probe_cache).unwrap_or(u32::MAX))
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
    let tagged: Vec<crate::tag::TaggedChunk> = chunks
        .iter()
        .map(|body| crate::tag::TaggedChunk::new(body.clone(), ""))
        .collect();
    let (out, stats) = refine_clash_tagged(&tagged, probe_cache, cfg);
    (out.into_iter().map(|c| c.body).collect(), stats)
}

fn dedupe_tagged_by_endpoint(
    nodes: Vec<TaggedNode>,
    probe_cache: &ProbeCache,
    stats: &mut FilterStats,
) -> Vec<TaggedNode> {
    let mut unparsed = Vec::new();
    let mut best: HashMap<(String, u16), (TaggedNode, u32)> = HashMap::new();

    for tagged in nodes {
        let Some(ep) = node_endpoint(&tagged.node) else {
            unparsed.push(tagged);
            continue;
        };
        let rank = endpoint_latency(&tagged.node, probe_cache).unwrap_or(u32::MAX);

        match best.get(&ep) {
            Some((_, existing)) if *existing <= rank => {
                stats.deduped += 1;
            }
            _ => {
                if best.contains_key(&ep) {
                    stats.deduped += 1;
                }
                best.insert(ep, (tagged, rank));
            }
        }
    }

    let mut ranked: Vec<(TaggedNode, u32)> = best.into_values().collect();
    ranked.sort_by_key(|(_, rank)| *rank);

    let mut out = unparsed;
    out.extend(ranked.into_iter().map(|(tagged, _)| tagged));
    out
}

fn endpoint_latency(node: &str, probe_cache: &ProbeCache) -> Option<u32> {
    let ep = node_endpoint(node)?;
    probe_cache.get(&ep).and_then(|r| r.latency_ms)
}

pub fn refine_clash_tagged(
    chunks: &[crate::tag::TaggedChunk],
    probe_cache: &ProbeCache,
    cfg: &FilterConfig,
) -> (Vec<crate::tag::TaggedChunk>, FilterStats) {
    let mut stats = FilterStats::default();
    let mut out = Vec::new();

    for chunk in chunks {
        let Some(blocks) = crate::parser::extract_clash_proxy_blocks(&chunk.body) else {
            out.push(chunk.clone());
            continue;
        };

        stats.before += blocks.len();
        let tagged_blocks: Vec<TaggedNode> = blocks
            .into_iter()
            .map(|block| TaggedNode::new(block, chunk.source.clone()))
            .collect();
        let (refined, block_stats) = refine_v2ray_tagged(&tagged_blocks, probe_cache, cfg);
        merge_filter_stats(&mut stats, &block_stats);
        stats.after += refined.len();

        if !refined.is_empty() {
            let blocks: Vec<String> = refined.into_iter().map(|t| t.node).collect();
            out.push(crate::tag::TaggedChunk::new(
                crate::parser::build_clash_proxies(&blocks),
                chunk.source.clone(),
            ));
        }
    }

    (out, stats)
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
    use crate::tag::TaggedNode;

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

    #[test]
    fn dedup_preserves_first_source() {
        let nodes = vec![
            TaggedNode::new("vless://a@1.1.1.1:443#n1".into(), "Au1rxx"),
            TaggedNode::new("vless://b@1.1.1.1:443#n2".into(), "DaBao-Lee"),
        ];
        let cfg = FilterConfig {
            dedupe_endpoint: true,
            max_latency_ms: None,
            max_nodes: None,
            drop_unparsed: false,
        };
        let (out, stats) = refine_v2ray_tagged(&nodes, &cache(), &cfg);
        assert_eq!(out.len(), 1);
        assert_eq!(stats.deduped, 1);
        assert_eq!(out[0].source, "Au1rxx");
    }
}
