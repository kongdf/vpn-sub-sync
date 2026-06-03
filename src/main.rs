mod config;
mod fetcher;
mod github_readme;
mod parser;
mod writer;

use anyhow::Result;
use config::{load_config, Source};
use fetcher::Fetcher;
use writer::{now_iso, output_path, SourceReport, SyncReport, Writer};

struct SyncOutcome {
    report: SourceReport,
    v2ray_nodes: Vec<String>,
    clash_body: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let config_path = writer::config_path();
    let cfg = load_config(&config_path)?;
    let fetcher = Fetcher::new()?;
    let output_dir = output_path();
    writer::ensure_output_dir(std::path::Path::new(&output_dir))?;

    println!("vpn-sub-sync");
    println!("config: {config_path}");
    println!("output: {output_dir}/\n");

    let mut v2ray_nodes: Vec<String> = Vec::new();
    let mut clash_chunks: Vec<String> = Vec::new();
    let mut reports: Vec<SourceReport> = Vec::new();

    for source in &cfg.source {
        if !source.enabled {
            println!("[skip] {} (disabled)", source.name);
            continue;
        }

        println!("[sync] {} ({})", source.name, source.kind);
        let outcome = sync_source(&fetcher, source).await;

        if outcome.report.ok {
            v2ray_nodes.extend(outcome.v2ray_nodes);
            if let Some(body) = outcome.clash_body {
                clash_chunks.push(body);
            }
            println!("  ok — {} nodes", outcome.report.node_count);
        } else {
            println!(
                "  fail — {}",
                outcome.report.error.as_deref().unwrap_or("unknown")
            );
        }

        reports.push(outcome.report);
    }

    let merged_stats = parser::parse_v2ray_content(&v2ray_nodes.join("\n"));
    let v2ray_b64 = if merged_stats.nodes.is_empty() {
        String::new()
    } else {
        parser::merge_v2ray_nodes(&merged_stats.nodes)
    };

    let clash_yaml = parser::merge_clash_yaml(&clash_chunks);
    let clash_count = if clash_yaml.is_empty() {
        0
    } else {
        clash_yaml.matches("\n  - ").count()
    };

    let report = SyncReport {
        synced_at: now_iso(),
        v2ray_total_nodes: merged_stats.nodes.len(),
        clash_proxy_count: clash_count,
        sources: reports,
    };

    Writer::new(&output_dir).write_all(&report, &v2ray_b64, &clash_yaml)?;

    println!("\ndone");
    println!("  v2ray nodes: {}", report.v2ray_total_nodes);
    println!("  clash proxies: {}", report.clash_proxy_count);
    println!("  written to {output_dir}/");

    Ok(())
}

async fn sync_source(fetcher: &Fetcher, source: &Source) -> SyncOutcome {
    let fetched_at = now_iso();
    let base_report = || SourceReport {
        name: source.name.clone(),
        kind: source.kind.clone(),
        format: source.format.clone(),
        resolved_url: String::new(),
        ok: false,
        node_count: 0,
        protocols: vec![],
        error: None,
        fetched_at: fetched_at.clone(),
    };

    let resolved_url = match github_readme::resolve_url(fetcher, source).await {
        Ok(url) => url,
        Err(e) => {
            return SyncOutcome {
                report: SourceReport {
                    resolved_url: source.url.clone().unwrap_or_default(),
                    error: Some(e.to_string()),
                    ..base_report()
                },
                v2ray_nodes: vec![],
                clash_body: None,
            };
        }
    };

    let body = match fetcher.fetch_text(&resolved_url).await {
        Ok(body) => body,
        Err(e) => {
            return SyncOutcome {
                report: SourceReport {
                    resolved_url,
                    error: Some(e.to_string()),
                    ..base_report()
                },
                v2ray_nodes: vec![],
                clash_body: None,
            };
        }
    };

    if source.is_v2ray() {
        let stats = parser::parse_v2ray_content(&body);
        if stats.nodes.is_empty() {
            return SyncOutcome {
                report: SourceReport {
                    name: source.name.clone(),
                    kind: source.kind.clone(),
                    format: source.format.clone(),
                    resolved_url,
                    ok: false,
                    node_count: 0,
                    protocols: vec![],
                    error: Some("no valid nodes found".into()),
                    fetched_at,
                },
                v2ray_nodes: vec![],
                clash_body: None,
            };
        }
        let node_count = stats.nodes.len();
        let protocols = stats.protocols.clone();
        return SyncOutcome {
            report: SourceReport {
                name: source.name.clone(),
                kind: source.kind.clone(),
                format: source.format.clone(),
                resolved_url,
                ok: true,
                node_count,
                protocols,
                error: None,
                fetched_at,
            },
            v2ray_nodes: stats.nodes,
            clash_body: None,
        };
    }

    if source.is_clash() {
        let ok = parser::validate_clash(&body);
        return SyncOutcome {
            report: SourceReport {
                name: source.name.clone(),
                kind: source.kind.clone(),
                format: source.format.clone(),
                resolved_url,
                ok,
                node_count: if ok { body.matches("\n  - ").count() } else { 0 },
                protocols: vec![],
                error: if ok {
                    None
                } else {
                    Some("invalid clash yaml".into())
                },
                fetched_at,
            },
            v2ray_nodes: vec![],
            clash_body: if ok { Some(body) } else { None },
        };
    }

    SyncOutcome {
        report: SourceReport {
            name: source.name.clone(),
            kind: source.kind.clone(),
            format: source.format.clone(),
            resolved_url,
            ok: false,
            node_count: 0,
            protocols: vec![],
            error: Some(format!("unknown format '{}'", source.format)),
            fetched_at,
        },
        v2ray_nodes: vec![],
        clash_body: None,
    }
}
