use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub source: Vec<Source>,
    #[serde(default)]
    pub probe: ProbeSettings,
    #[serde(default)]
    pub naming: NamingSettings,
    #[serde(default)]
    pub filter: FilterSettings,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FilterSettings {
    #[serde(default = "default_filter_dedupe")]
    pub dedupe_endpoint: bool,
    #[serde(default = "default_filter_max_latency")]
    pub max_latency_ms: Option<u32>,
    #[serde(default = "default_filter_max_nodes")]
    pub max_nodes: Option<usize>,
    #[serde(default = "default_filter_drop_unparsed")]
    pub drop_unparsed: bool,
}

impl Default for FilterSettings {
    fn default() -> Self {
        Self {
            dedupe_endpoint: default_filter_dedupe(),
            max_latency_ms: default_filter_max_latency(),
            max_nodes: default_filter_max_nodes(),
            drop_unparsed: default_filter_drop_unparsed(),
        }
    }
}

fn default_filter_dedupe() -> bool {
    true
}

fn default_filter_max_latency() -> Option<u32> {
    Some(3000)
}

fn default_filter_max_nodes() -> Option<usize> {
    None
}

fn default_filter_drop_unparsed() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
pub struct NamingSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_naming_template")]
    pub template: String,
    #[serde(default = "default_first_name")]
    pub first_name: String,
    #[serde(default = "default_tag_source")]
    pub tag_source: bool,
}

impl Default for NamingSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            template: default_naming_template(),
            first_name: default_first_name(),
            tag_source: default_tag_source(),
        }
    }
}

fn default_naming_template() -> String {
    "{source}-{country}-{latency}".to_string()
}

fn default_first_name() -> String {
    "孔大夫-我做个艺术家".to_string()
}

fn default_tag_source() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProbeSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_probe_timeout_secs")]
    pub timeout_secs: u64,
    #[serde(default = "default_probe_concurrency")]
    pub concurrency: usize,
}

impl Default for ProbeSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            timeout_secs: default_probe_timeout_secs(),
            concurrency: default_probe_concurrency(),
        }
    }
}

fn default_probe_timeout_secs() -> u64 {
    3
}

fn default_probe_concurrency() -> usize {
    50
}

#[derive(Debug, Clone, Deserialize)]
pub struct Source {
    pub name: String,
    pub kind: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub format: String,

    // direct
    pub url: Option<String>,

    // github_readme
    pub repo: Option<String>,
    #[serde(default = "default_branch")]
    pub branch: String,
    pub url_pattern: Option<String>,
}

fn default_enabled() -> bool {
    true
}

fn default_branch() -> String {
    "main".to_string()
}

impl Source {
    pub fn is_v2ray(&self) -> bool {
        self.format.eq_ignore_ascii_case("v2ray")
    }

    pub fn is_clash(&self) -> bool {
        self.format.eq_ignore_ascii_case("clash")
    }
}

pub fn load_config(path: &str) -> anyhow::Result<Config> {
    let content = std::fs::read_to_string(path)?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}
