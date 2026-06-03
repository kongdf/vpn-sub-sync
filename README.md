# vpn-sub-sync

每日自动拉取、校验、合并公开 VPN 订阅源，输出 v2ray / Clash 订阅文件。

> 公益节点聚合，仅供学习测试；不保证可用性与安全性；请遵守当地法律。

## 功能

- 从 `sources.toml` 读取订阅源（直链 + GitHub README 解析）
- 并发拉取、重试、节点去重
- 输出 `output/v2ray.txt`（base64）、`output/clash.yaml`、`output/sources.json`
- GitHub Actions 每日定时同步

## 本地运行

```bash
cd vpn-sub-sync
cargo run --release
```

产物在 `output/` 目录（已 gitignore，本地运行不会误提交）。

## 推到 GitHub

```bash
cd ~/Desktop/vpn-sub-sync
git init
git add .
git commit -m "init: vpn subscription sync tool"
git remote add origin git@github.com:YOUR_USER/vpn-sub-sync.git
git push -u origin main
```

推送后，在仓库 **Settings → Actions → General** 中允许 workflow 写权限（或保持默认，workflow 已声明 `contents: write`）。

## v2rayN 订阅

仓库 push 且 Actions 跑通后，订阅地址为：

```
https://raw.githubusercontent.com/YOUR_USER/vpn-sub-sync/main/output/v2ray.txt
```

Clash：

```
https://raw.githubusercontent.com/YOUR_USER/vpn-sub-sync/main/output/clash.yaml
```

## 添加新源

编辑 `sources.toml`：

```toml
[[source]]
name = "my-source"
kind = "direct"          # 或 github_readme
url = "https://example.com/sub.txt"
format = "v2ray"         # 或 clash
enabled = true
```

`github_readme` 类型额外需要：

```toml
repo = "owner/repo"
branch = "main"
url_pattern = 'https://raw\.githubusercontent\.com/owner/repo/main/sub\d+'
```

## 目录结构

```
vpn-sub-sync/
├── sources.toml              # 源配置
├── src/
│   ├── main.rs
│   ├── config.rs
│   ├── fetcher.rs
│   ├── github_readme.rs
│   ├── parser.rs
│   └── writer.rs
├── output/                   # 同步产物（本地忽略，Actions 发布）
└── .github/workflows/sync.yml
```

## License

MIT
