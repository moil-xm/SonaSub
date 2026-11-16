---
name: project-boundaries
description: Use when deciding where SonaSub code belongs or how modules connect - desktop frontend calling Tauri commands, src-tauri Rust vs engine ownership, Chrome extension permissions or messaging, shared packages, packages/protocol contract types, or any new dependency between apps and packages.
---

# project-boundaries

## 先读

- [docs/architecture.md](../../../docs/architecture.md)：职责表与允许依赖方向。
- [AGENTS.md](../../../AGENTS.md)：边界与当前阶段。
- 涉及双方的入口：`apps/desktop/src-tauri/src/lib.rs`、`apps/desktop/src-tauri/capabilities/default.json`、`apps/engine/src/main.ts`、`apps/extension/wxt.config.ts`、`packages/protocol/README.md`。

## 步骤

1. 画出数据流：谁产生数据、谁消费、经过哪个运行时边界（Tauri IPC / 扩展消息 / 引擎进程）。先确定归属再写 API。
2. 按归属放置：原生能力 → `src-tauri`（新增命令同步 `capabilities` 权限，保持最小）；本地处理 → `apps/engine`；浏览器/扩展能力 → 对应应用；跨应用共享类型或契约 → `packages/protocol`；共享 UI → `packages/ui`。
3. 检查依赖方向：apps 之间不导入内部源码；packages 不依赖 apps；`packages/ui` 不出现 `@tauri-apps/*`、`chrome.*`、`node:*`。跨应用复用改为通过 `packages/*` 的显式 `exports`。
4. `packages/protocol` 目前为空：只有调用双方与数据流已确定时才新增契约文件并声明 `exports`，不预定义字段。
5. 平台专属依赖只加到对应应用；共享包新增依赖前确认它对所有消费方都成立。
6. 用 `rg` 检查是否出现越界导入（如 `apps/extension` 引用 `apps/desktop/src`，或 `packages/ui` 引用 `node:`）。

## 约束

- 遵循 AGENTS.md 的最小 API 规则：先满足真实 call site，不为“以后可能用到”预留接口。
- 不新增未确定的音频/识别/翻译/多标签页流程。

## 验证

- `pnpm check` + `pnpm build`；Rust 改动加 `pnpm check:rust`；引擎改动加 `pnpm start:engine`。
- 新增 Tauri 命令或扩展权限：`pnpm dev:desktop:app` / `pnpm dev:extension` 实际调用一次；无法运行时说明。

## 交付

职责归属与依赖方向的变化、新增契约或权限及其理由、越界检查结果、执行命令与结果、未验证项。
