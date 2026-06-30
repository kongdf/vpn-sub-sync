# vpn-sub-sync

每日自动拉取、校验、合并公开 VPN 订阅源，输出 v2ray / Clash 订阅文件。

> 公益节点聚合，仅供学习测试；不保证可用性与安全性；请遵守当地法律。

## 订阅地址

v2ray（N2.0 / v2rayNG 等）：

```
https://raw.githubusercontent.com/kongdf/vpn-sub-sync/main/output/v2ray.txt
```

Clash（ClashX / Clash Verge 等，有 Clash 源时可用）：

```
https://raw.githubusercontent.com/kongdf/vpn-sub-sync/main/output/clash.yaml
```

同步状态与节点统计见 [output/README.md](output/README.md)。

## 功能

- 从 `sources.toml` 读取订阅源（直链 + GitHub README 解析）
- 并发拉取、重试
- TCP 探测节点可用性与延迟
- 按 host:port 去重、延迟上限过滤
- 按订阅源 / 国家 / 延迟重命名节点
- 输出 `output/v2ray.txt`（base64）、`output/clash.yaml`、`output/sources.json`、`output/README.md`
- GitHub Actions 每日定时同步（UTC 02:00 / 北京时间 10:00）

## 节点命名

首个节点：`{MM-DD}-孔大夫`（如 `06-30-孔大夫`）

其余节点：`{source}-{country}-{latency}`（如 `xrayvip-韩国-2ms`）

可在 `sources.toml` 的 `[naming]` 段调整 `first_name` 与 `template`。

## 筛选与探测

默认配置（见 `sources.toml`）：

- TCP 探测：超时 3s，并发 50；不可达节点剔除
- 同 host:port 去重
- 延迟超过 3000ms 剔除
- 无法解析端点的节点剔除

## 本地运行

```bash
cd vpn-sub-sync
cargo run --release
```

产物在 `output/` 目录（已 gitignore，本地运行不会误提交；Actions 用 `git add -f` 发布）。

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
├── sources.toml              # 源、探测、筛选、命名配置
├── src/
│   ├── main.rs
│   ├── config.rs
│   ├── fetcher.rs
│   ├── github_readme.rs
│   ├── parser.rs
│   ├── probe.rs
│   ├── filter.rs
│   ├── naming.rs
│   ├── country.rs
│   ├── tag.rs
│   └── writer.rs
├── output/                   # 同步产物（本地忽略，Actions 发布）
└── .github/workflows/sync.yml
```

## License

MIT
