use anyhow::Result;
use chrono::{FixedOffset, Utc};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
pub struct ProbeKindReport {
    pub before: usize,
    pub after: usize,
    pub reachable: usize,
    pub unreachable: usize,
    pub unparsed: usize,
}

impl From<crate::probe::ProbeStats> for ProbeKindReport {
    fn from(stats: crate::probe::ProbeStats) -> Self {
        Self {
            before: stats.before,
            after: stats.after,
            reachable: stats.reachable,
            unreachable: stats.unreachable,
            unparsed: stats.unparsed,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ProbeReport {
    pub enabled: bool,
    pub timeout_secs: u64,
    pub concurrency: usize,
    pub v2ray: ProbeKindReport,
    pub clash: ProbeKindReport,
}

#[derive(Debug, Serialize)]
pub struct SourceReport {
    pub name: String,
    pub kind: String,
    pub format: String,
    pub resolved_url: String,
    pub ok: bool,
    pub node_count: usize,
    pub protocols: Vec<(String, usize)>,
    pub error: Option<String>,
    pub fetched_at: String,
}

#[derive(Debug, Serialize)]
pub struct NamingReport {
    pub enabled: bool,
    pub template: String,
    pub first_name: String,
    pub tag_source: bool,
}

#[derive(Debug, Serialize)]
pub struct FilterReport {
    pub dedupe_endpoint: bool,
    pub max_latency_ms: Option<u32>,
    pub max_nodes: Option<usize>,
    pub drop_unparsed: bool,
}

#[derive(Debug, Serialize)]
pub struct SyncReport {
    pub synced_at: String,
    pub v2ray_total_nodes: usize,
    pub clash_proxy_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probe: Option<ProbeReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<FilterReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub naming: Option<NamingReport>,
    pub sources: Vec<SourceReport>,
}

pub struct Writer {
    output_dir: String,
}

impl Writer {
    pub fn new(output_dir: impl Into<String>) -> Self {
        Self {
            output_dir: output_dir.into(),
        }
    }

    pub fn write_all(
        &self,
        report: &SyncReport,
        v2ray_b64: &str,
        clash_yaml: &str,
    ) -> Result<()> {
        std::fs::create_dir_all(&self.output_dir)?;

        if !v2ray_b64.is_empty() {
            std::fs::write(format!("{}/v2ray.txt", self.output_dir), v2ray_b64)?;
        }

        if !clash_yaml.is_empty() {
            std::fs::write(format!("{}/clash.yaml", self.output_dir), clash_yaml)?;
        }

        let json = serde_json::to_string_pretty(report)?;
        std::fs::write(format!("{}/sources.json", self.output_dir), json)?;

        let readme = render_readme(report, v2ray_b64, clash_yaml);
        std::fs::write(format!("{}/README.md", self.output_dir), readme)?;

        Ok(())
    }
}

const REPO_RAW_BASE: &str =
    "https://raw.githubusercontent.com/kongdf/vpn-sub-sync/main/output";

fn render_readme(report: &SyncReport, v2ray_b64: &str, clash_yaml: &str) -> String {
    let mut md = String::new();
    md.push_str("# VPN Subscription Output\n\n");
    md.push_str("> 公益节点聚合，仅供学习测试；不保证可用性与安全性；请遵守当地法律。\n\n");
    md.push_str(&format!("**上次同步：** {}\n\n", report.synced_at));
    md.push_str(&format!(
        "**v2ray 节点总数：** {} | **Clash 代理数：** {}\n\n",
        report.v2ray_total_nodes, report.clash_proxy_count
    ));

    md.push_str("## 订阅链接\n\n");
    if !v2ray_b64.is_empty() {
        md.push_str("- v2rayN / v2rayNG：\n");
        md.push_str(&format!("  `{REPO_RAW_BASE}/v2ray.txt`\n\n"));
    }
    if !clash_yaml.is_empty() {
        md.push_str("- Clash / Clash Verge：\n");
        md.push_str(&format!("  `{REPO_RAW_BASE}/clash.yaml`\n\n"));
    }

    if let Some(naming) = &report.naming {
        if naming.enabled || naming.tag_source {
            md.push_str("## 节点命名\n\n");
            if naming.enabled {
                md.push_str(&format!(
                    "首个节点：`{{MM-DD}}-{}`（如 `06-30-{}`）\n\n",
                    naming.first_name, naming.first_name
                ));
                md.push_str(&format!(
                    "其余节点：`{}`（如 `xrayvip-韩国-2ms`）\n\n",
                    naming.template
                ));
            } else {
                md.push_str("按订阅源前缀标记节点名称。\n\n");
            }
        }
    }

    if let Some(filter) = &report.filter {
        if filter.dedupe_endpoint
            || filter.max_latency_ms.is_some()
            || filter.max_nodes.is_some()
            || filter.drop_unparsed
        {
            md.push_str("## 筛选\n\n");
            if filter.dedupe_endpoint {
                md.push_str("- 同 host:port 去重\n");
            }
            if let Some(ms) = filter.max_latency_ms {
                md.push_str(&format!("- 延迟超过 {ms}ms 剔除\n"));
            }
            if filter.drop_unparsed {
                md.push_str("- 无法解析端点的节点剔除\n");
            }
            if let Some(max) = filter.max_nodes {
                md.push_str(&format!("- 最多保留 {max} 个节点\n"));
            }
            md.push('\n');
        }
    }

    if let Some(probe) = &report.probe {
        md.push_str("## TCP 探测\n\n");
        md.push_str(&format!(
            "超时 {}s，并发 {}。不可达节点已剔除；无法解析端点的节点{}。\n\n",
            probe.timeout_secs,
            probe.concurrency,
            if report
                .filter
                .as_ref()
                .is_some_and(|f| f.drop_unparsed)
            {
                "剔除"
            } else {
                "保留"
            }
        ));
        md.push_str("| 类型 | 探测前 | 保留 | 可达 | 不可达 | 未解析 |\n");
        md.push_str("|---|---|---|---|---|---|\n");
        md.push_str(&format!(
            "| v2ray | {} | {} | {} | {} | {} |\n",
            probe.v2ray.before,
            probe.v2ray.after,
            probe.v2ray.reachable,
            probe.v2ray.unreachable,
            probe.v2ray.unparsed
        ));
        md.push_str(&format!(
            "| clash | {} | {} | {} | {} | {} |\n\n",
            probe.clash.before,
            probe.clash.after,
            probe.clash.reachable,
            probe.clash.unreachable,
            probe.clash.unparsed
        ));
    }

    md.push_str("## 各源状态\n\n");
    md.push_str("| 源 | 状态 | 节点数 | 说明 |\n");
    md.push_str("|---|---|---|---|\n");
    for s in &report.sources {
        let status = if s.ok { "✅" } else { "❌" };
        let err = s.error.as_deref().unwrap_or("-");
        md.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            s.name, status, s.node_count, err
        ));
    }

    md
}

pub fn now_iso() -> String {
    let beijing = FixedOffset::east_opt(8 * 3600).expect("Beijing offset");
    Utc::now()
        .with_timezone(&beijing)
        .format("%Y-%m-%d %H:%M:%S 北京时间")
        .to_string()
}

pub fn config_path() -> String {
    std::env::args()
        .nth(1)
        .unwrap_or_else(|| "sources.toml".to_string())
}

pub fn output_path() -> String {
    "output".to_string()
}

pub fn ensure_output_dir(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)?;
    Ok(())
}
