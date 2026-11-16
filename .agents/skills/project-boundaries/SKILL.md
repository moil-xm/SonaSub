---
name: project-boundaries
description: Use when deciding where SonaSub code belongs or how modules connect - desktop frontend calling Tauri commands, src-tauri Rust vs engine ownership, Chrome extension permissions or messaging, shared packages, packages/protocol contract types, or any new dependency between apps and packages.
---

# project-boundaries

## 先读

- [docs/architecture.md](../../../docs/architecture.md)：职责表与允许依赖方向。
- [AGENTS.md](../../../AGENTS.md)：边界与当前阶段。
- 涉及双方的入口：`apps/desktop/src-tauri/src/{lib,engine,protocol,resources}.rs`、`apps/desktop/src-tauri/capabilities/default.json`、`apps/engine/src/main.ts`、`apps/extension/wxt.config.ts`、`packages/protocol/README.md`。

## 步骤

1. 画出数据流：谁产生数据、谁消费、经过哪个运行时边界（Tauri IPC / 扩展消息 / 引擎进程）。先确定归属再写 API。
2. 按归属放置：原生能力 → `src-tauri`（新增命令同步 `capabilities` 权限，保持最小）；本地处理 → `apps/engine`；浏览器/扩展能力 → 对应应用；跨应用共享类型或契约 → `packages/protocol`；共享 UI → `packages/ui`。
3. 检查依赖方向：apps 之间不导入内部源码；packages 不依赖 apps；`packages/ui` 不出现 `@tauri-apps/*`、`chrome.*`、`node:*`。跨应用复用改为通过 `packages/*` 的显式 `exports`。
4. 引擎协议只覆盖认证、运行信息、控制响应与关闭通知；Rust 持有进程/连接/凭据，React 只走 Tauri IPC。修改契约时同步 TS、Rust 与测试，不预定义音频/字幕/翻译字段。
5. 平台专属依赖只加到对应应用；共享包新增依赖前确认它对所有消费方都成立。
6. 用 `rg` 检查是否出现越界导入（如 `apps/extension` 引用 `apps/desktop/src`，或 `packages/ui` 引用 `node:`）。

## 约束

- 遵循 AGENTS.md 的最小 API 规则：先满足真实 call site，不为“以后可能用到”预留接口。
- 不新增未确定的音频/识别/翻译/多标签页流程。

## 验证

- `pnpm check` + `pnpm build`；Rust 改动加 `pnpm check:rust` 与 `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -- --test-threads=1`；引擎改动加 `pnpm test`。
- `pnpm start:engine` 缺少 Rust 认证信息时应拒绝启动，不再作为成功标准。生命周期改动运行 `pnpm dev:desktop:app` 或实际 `.app`，验证启动、停止/重启、意外退出和退出清理。
- 分发资源改动执行 `pnpm --filter @sonasub/desktop prepare:engine` 与实际安装包构建/启动；无法验证的平台明确说明。

## 交付

职责归属与依赖方向的变化、新增契约或权限及其理由、越界检查结果、执行命令与结果、未验证项。
