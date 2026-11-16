# @sonasub/protocol

[English](README.md) | **简体中文**

跨模块通信契约的归属位置。当前只定义桌面 Rust 后端与 Node.js 引擎之间的本机 WebSocket 协议，不包含音频、字幕、翻译或扩展采集消息。

## 当前契约

- `@sonasub/protocol/engine` 导出协议版本、启动握手标记、运行信息类型与边界解析函数。
- 引擎在 stdout 输出一次 `SONASUB_HANDSHAKE` 前缀的 JSON，包含协议版本、实际监听端口与进程 ID；普通日志不得使用该前缀。
- WebSocket 首条消息为 Rust 生成的随机认证信息。认证完成前不接受控制请求，认证信息不进入前端状态或日志。
- 控制请求只有 `engine.info` 与 `engine.shutdown`；响应携带同一 `id`，区分成功结果与错误。
- 运行信息来自引擎进程：引擎版本、Node.js 版本、PID 与 ISO 启动时间。

Rust 侧的序列化与校验位于 `apps/desktop/src-tauri/src/protocol.rs`，与这里的字段和版本保持一致。修改协议时同时更新两侧及对应测试，不跨应用引用内部源码。

边界见 [docs/architecture.md](../../docs/architecture.md)。
