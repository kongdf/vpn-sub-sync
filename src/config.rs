use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub source: Vec<Source>,
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
