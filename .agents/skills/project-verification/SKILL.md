---
name: project-verification
description: Use when finishing a SonaSub change and deciding which checks to run (pnpm check, test, build, check:rust, engine lifecycle and packaged-resource verification), whether docs-only or Skills-only edits need compiling, executing checks, reporting results and gaps, or locating the owning workspace after a check or build fails.
---

# project-verification

## 先读

- [AGENTS.md](../../../AGENTS.md)：命令与交付要求。
- [package.json](../../../package.json) 与受影响 workspace 的 `package.json` 脚本。
- `git status --porcelain=v1` 与 `git diff --stat`，确认改动范围并保留已有 staged/unstaged/untracked 内容。

## 按范围选择

| 改动范围                  | 命令                                                                                                                           |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| 任意 TS/TSX/配置          | `pnpm check`                                                                                                                   |
| 桌面端前端                | `pnpm --filter @sonasub/desktop build`；界面改动再 `pnpm dev:desktop` 查看                                                     |
| Tauri / Rust              | `pnpm check:rust` + `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml -- --test-threads=1`；界面改动再运行实际应用 |
| 引擎                      | `pnpm test` + `pnpm --filter @sonasub/engine build`；独立 `pnpm start:engine` 应因缺少认证而拒绝启动                           |
| 引擎生命周期 / 分发资源   | `pnpm --filter @sonasub/desktop prepare:engine`；macOS 构建并运行 `.app`，检查真实信息、重启、意外退出与应用退出清理           |
| 扩展                      | `pnpm --filter @sonasub/extension build`（产物 `.output/chrome-mv3`，manifest_version 为 3）；交互改动再 `pnpm dev:extension`  |
| workspace / 依赖 / 锁文件 | `pnpm install` 后 `pnpm build`                                                                                                 |
| 全部应用                  | `pnpm build`                                                                                                                   |
| 仅文档 / Skills           | frontmatter 与引用路径检查、`git diff --check`；不要求编译                                                                     |

## 步骤

1. 列出改动文件并映射到上表；只运行覆盖改动的命令，必要时再扩大到 `pnpm build`。
2. 逐条执行并读取完整输出与退出码；失败先定位所属 workspace 与根因，修复后重跑同一命令，不吞错。
3. Skills 改动：核对每个 `SKILL.md` 的 `name` 与目录一致、`description` 非空，引用的相对路径存在，命令在 `package.json` 中真实存在。
4. 环境无法完成的项（如需 GUI、缺少 Rust 系统依赖）明确记录原因，不用“应该可以”替代。
5. 生命周期验证使用真实进程：并发启动不重复、认证错误不能控制、启动失败保留引擎诊断、停止/重启后 PID/端口一致更新、意外退出显示失败、应用退出只清理受管理子进程。离线验证只针对已构建产物，不依赖互联网。
6. 结束前再跑一次覆盖最终改动的检查，确认结果对应当前文件。

## 约束

- 不为不存在的逻辑添加凑数测试；不为局部改动修复无关失败，只报告。
- 不重复运行已通过的检查，除非文件再次改动或出现未解决风险。

## 交付

改动范围、每条命令及其结果（通过/失败/未运行及原因）、遗留风险。
