---
name: project-workspace
description: Use when adding or changing a pnpm workspace, installing or upgrading a dependency (wxt, vite-plus, tauri, react, etc.), updating pnpm-lock.yaml or Cargo.lock, Vite+ toolchain settings, package exports, import aliases, or root/package scripts in SonaSub. Not for page UI or business logic.
---

# project-workspace

## 先读

- [AGENTS.md](../../../AGENTS.md)：技术栈、依赖归属、命令、当前阶段。
- [package.json](../../../package.json)、[pnpm-workspace.yaml](../../../pnpm-workspace.yaml)、[.npmrc](../../../.npmrc)、[vite.config.ts](../../../vite.config.ts)、[tsconfig.base.json](../../../tsconfig.base.json)。
- 目标 workspace 的 `package.json`、`tsconfig.json` 与其构建配置（`vite.config.ts` / `wxt.config.ts` / `src-tauri/tauri.conf.json`）。

## 步骤

1. 确认改动属于哪个 workspace；依赖只加到实际使用它的包，开发工具进 `devDependencies`。
2. 版本固定为精确版本（`.npmrc` 已设 `save-exact`）。升级前用 `pnpm view <pkg> engines peerDependencies --json` 核对：Node 需 `^24.11.0`；声明 `vite` 的 workspace 必须使用别名 `npm:@voidzero-dev/vite-plus-core@<vite-plus 版本>`（`vp` 会校验该内核版本），WXT 与 `@vitejs/plugin-react` 的 `vite` peer 由它满足。升级 `vite-plus` 时同步更新根、desktop、extension 三处别名以及 `pnpm-workspace.yaml` 的 `overrides['vite@*']`。
3. 新增 workspace：在 `apps/*` 或 `packages/*` 下建目录，写 `package.json`（`private: true`、`type: module`）与 `tsconfig.json`（`extends ../../tsconfig.base.json`）。packages 不依赖 apps。
4. 新增共享导出：同时更新该包 `exports`（`packages/ui` 内部引用用 `#lib/*`），并确认消费方能解析。
5. 工具链只在根 `vite.config.ts` 的 `lint` / `fmt` 块配置，包级差异用 `overrides`；不新增 ESLint、Prettier、Biome 等重复工具。Rust 检查用 `cargo fmt` / `clippy`。
6. 脚本：包内保留自己的 `dev` / `build`，根目录用 `vp run -r <script>` 或 `pnpm --filter <name> <script>` 聚合；Tauri 与 WXT 保留各自构建入口。
7. 执行 `pnpm install` 更新 `pnpm-lock.yaml`；Rust 依赖变更后在 `apps/desktop/src-tauri` 执行 `cargo generate-lockfile` 或任一 cargo 命令更新 `Cargo.lock`。锁文件必须提交。

## 约束

- 遵循 AGENTS.md 的 KISS/DRY/YAGNI 与边界规则；不为尚未确定的能力预建 workspace、导出或脚本。
- 不引入第二套格式化/检查工具；不改 `packageManager` 与 Node 主版本，除非任务明确要求。

## 验证

- 依赖/导出/脚本改动：`pnpm install`，然后 `pnpm check`；受影响应用再执行 `pnpm build`。
- 仅根脚本或配置改动：`pnpm check` 与所改脚本本身（如 `pnpm start:engine`）。
- Rust 依赖改动：`pnpm check:rust`。

## 交付

改动的 workspace 与文件、版本选择依据（兼容性核对结果）、锁文件是否更新、执行的命令与结果、未验证项。
