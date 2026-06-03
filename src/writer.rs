use anyhow::Result;
use chrono::Utc;
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
pub struct SyncReport {
    pub synced_at: String,
    pub v2ray_total_nodes: usize,
    pub clash_proxy_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probe: Option<ProbeReport>,
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

fn render_readme(report: &SyncReport, v2ray_b64: &str, clash_yaml: &str) -> String {
    let mut md = String::new();
    md.push_str("# VPN Subscription Output\n\n");
    md.push_str("> 公益节点聚合，仅供学习测试；不保证可用性与安全性；请遵守当地法律。\n\n");
    md.push_str(&format!("**上次同步：** {}\n\n", report.synced_at));
    md.push_str(&format!(
        "**v2ray 节点总数：** {} | **Clash 代理数：** {}\n\n",
        report.v2ray_total_nodes, report.clash_proxy_count
    ));

    if let Some(probe) = &report.probe {
        md.push_str("## TCP 探测\n\n");
        md.push_str(&format!(
            "超时 {}s，并发 {}。不可达节点已剔除；无法解析端点的节点保留。\n\n",
            probe.timeout_secs, probe.concurrency
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

    md.push_str("## 订阅链接\n\n");
    md.push_str("将 `YOUR_USER/YOUR_REPO` 替换为你的 GitHub 仓库路径：\n\n");
    if !v2ray_b64.is_empty() {
        md.push_str("- v2rayN / v2rayNG：\n");
        md.push_str("  `https://raw.githubusercontent.com/YOUR_USER/YOUR_REPO/main/output/v2ray.txt`\n\n");
    }
    if !clash_yaml.is_empty() {
        md.push_str("- Clash / Clash Verge：\n");
        md.push_str("  `https://raw.githubusercontent.com/YOUR_USER/YOUR_REPO/main/output/clash.yaml`\n\n");
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
    Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string()
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
