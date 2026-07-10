# VPN Subscription Output

> 公益节点聚合，仅供学习测试；不保证可用性与安全性；请遵守当地法律。

**上次同步：** 2026-07-10 14:05:26 北京时间

**v2ray 节点总数：** 240 | **Clash 代理数：** 0

## 订阅链接

- v2rayN / v2rayNG：
  `https://raw.githubusercontent.com/kongdf/vpn-sub-sync/main/output/v2ray.txt`

## 节点命名

首个节点：`{MM-DD}-孔大夫`（如 `06-30-孔大夫`）

其余节点：`{source}-{country}-{latency}`（如 `xrayvip-韩国-2ms`）

## 筛选

- 同 host:port 去重
- 延迟超过 3000ms 剔除
- 无法解析端点的节点剔除

## TCP 探测

超时 3s，并发 50。不可达节点已剔除；无法解析端点的节点剔除。

| 类型 | 探测前 | 保留 | 可达 | 不可达 | 未解析 |
|---|---|---|---|---|---|
| v2ray | 788 | 300 | 299 | 488 | 1 |
| clash | 0 | 0 | 0 | 0 | 0 |

## 各源状态

| 源 | 状态 | 节点数 | 说明 |
|---|---|---|---|
| xrayvip | ✅ | 9 | - |
| free-nodes | ✅ | 779 | - |
| nodev2rayn | ❌ | 0 | HTTP 404 Not Found for https://nodev2rayn.github.io/uploads/2026/07/4-20260705.txt |
