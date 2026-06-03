use anyhow::{Context, Result};
use regex::Regex;

use crate::config::Source;
use crate::fetcher::Fetcher;

pub async fn resolve_url(fetcher: &Fetcher, source: &Source) -> Result<String> {
    match source.kind.as_str() {
        "direct" => source
            .url
            .clone()
            .context(format!("source '{}' missing url", source.name)),
        "github_readme" => resolve_from_readme(fetcher, source).await,
        other => anyhow::bail!("unknown source kind '{other}' for '{}'", source.name),
    }
}

async fn resolve_from_readme(fetcher: &Fetcher, source: &Source) -> Result<String> {
    let repo = source
        .repo
        .as_ref()
        .context(format!("source '{}' missing repo", source.name))?;
    let pattern = source
        .url_pattern
        .as_ref()
        .context(format!("source '{}' missing url_pattern", source.name))?;

    let readme_url = format!(
        "https://raw.githubusercontent.com/{repo}/{}/README.md",
        source.branch
    );
    let readme = fetcher.fetch_text(&readme_url).await?;
    let re = Regex::new(pattern).context("invalid url_pattern regex")?;

    let mut matches: Vec<&str> = re.find_iter(&readme).map(|m| m.as_str()).collect();
    if matches.is_empty() {
        anyhow::bail!("no subscription URL matched pattern in {repo} README");
    }

    // 取最后一个匹配（通常 README 里最新链接在末尾）
    Ok(matches.pop().unwrap().to_string())
}
