# SonaSub

[English](README.md) | **简体中文**

开源实时字幕项目。当前已打通桌面前端 → Tauri Rust → 本机 Node.js 引擎的真实通信与进程生命周期：启动、状态查询、停止、重启、失败提示和退出清理。Chrome 扩展仍只有 popup；音频采集、语音识别、翻译与扩展采集尚未实现。

## 技术栈

- 工程管理：pnpm workspace + Vite+（`vp` 统一管理格式、检查、类型检查与构建）
- 桌面：Tauri 2 + Rust，前端 React + TypeScript
- 样式与组件：Tailwind CSS 4 + CSS Variables、shadcn/ui、Iconify 静态图标
- 界面语言：i18next + react-i18next，共享 `en-US` / `zh-CN` 资源与类型安全 Selector
- 路由与状态：TanStack Router / Query / Form、Zustand、Day.js
- 本地引擎：Node.js 24 LTS；开发直接运行 TypeScript，安装包使用随应用分发的 Node.js 与自包含构建产物
- 扩展：WXT + React + TypeScript（Manifest V3）

## 环境要求

- Node.js 24 LTS（`^24.11.0`）
- pnpm 12（`packageManager` 已固定为 `pnpm@12.15.0`）
- Rust 稳定版 ≥ 1.90（含 `rustfmt`、`clippy`），仅桌面原生后端需要
- macOS 需 Xcode Command Line Tools；Linux 需 Tauri 2 的 WebKitGTK 等系统依赖

## 初始化

```bash
pnpm install
```

## 常用命令

```bash
pnpm dev:desktop        # 桌面前端开发服务器 http://localhost:5173
pnpm dev:desktop:app    # Tauri 桌面应用（编译 Rust 并打开窗口）
pnpm dev:extension      # WXT 扩展开发
pnpm start:engine       # 无 Rust 认证信息时拒绝独立启动（预期失败）

pnpm test               # 协议、真实 Node.js 引擎、桌面 UI 与 i18n 测试
pnpm check              # 格式 + 代码检查 + 类型检查
pnpm typecheck          # 仅类型检查
pnpm lint               # 仅代码检查
pnpm fmt                # 仅格式化
pnpm check:rust         # cargo fmt --check + clippy
pnpm build              # 构建 desktop / engine / extension
pnpm --filter @sonasub/desktop prepare:engine # 准备分发 Node.js 与引擎产物
pnpm --filter @sonasub/desktop tauri:build    # 构建当前平台安装包
pnpm --filter @sonasub/desktop exec tauri build --debug --bundles app --no-sign # macOS 本地未签名验证包
pnpm build:extension:zip # 打包扩展 zip
```

桌面概览页提供引擎状态、启动/停止/重启、真实版本与启动时间。开发模式的诊断区域显示 PID 与本机端口；认证信息不会传给前端。

桌面顶栏与扩展 popup 均提供界面语言设置。语言偏好会跨重启保留，默认跟随系统语言。

macOS 本地验证包位于 `apps/desktop/src-tauri/target/debug/bundle/macos/SonaSub.app`。构建会下载并校验 Node.js 24.22.1；运行时和引擎资源位于应用包的 `Contents/Resources/`。未签名包仅用于本机验证，不等同于已签名/公证的发布包。

扩展开发产物在 `apps/extension/.output/chrome-mv3-dev`，生产产物在 `apps/extension/.output/chrome-mv3`，可在 Chrome 扩展页以「加载已解压的扩展程序」载入。

## 结构

```
apps/desktop      桌面前端与 src-tauri 原生后端
apps/engine       本机 WebSocket 引擎与真实运行信息
apps/extension    Chrome 扩展 popup
packages/ui       共享组件、主题、图标与样式
packages/i18n     共享界面翻译与类型安全的 i18next 配置
packages/protocol 桌面后端与引擎的通信契约
.agents/skills    项目专属 Skills
docs/architecture.md  模块职责与依赖边界
AGENTS.md         全仓库开发约定
```

开发约定见 [AGENTS.md](AGENTS.md)，模块边界见 [docs/architecture.md](docs/architecture.md)。

## 当前验证

- `pnpm install --frozen-lockfile`、`pnpm check`、`pnpm check:rust`、`pnpm test`（22 个测试）、`pnpm build`。
- `cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml -- --test-threads=1`（14 个测试）：真实进程并发启动、停止/重启、意外退出、控制连接断开、认证隔离与退出清理。
- 同一组 11 个真实进程 Rust 集成测试在禁止外网、仅允许回环网络的环境中通过。
- macOS arm64 实际构建并运行 `.app`，验证随包 Node.js 与引擎产物，并在真实应用中检查启动/停止/重启、失败恢复、意外退出提示、超大消息连接隔离与受管子进程退出清理。
- CI 工作流已配置冻结安装、检查、TypeScript 测试、构建、引擎资源准备与串行 Rust 测试；独立启动引擎按预期因缺少认证信息而失败。本次改动尚未推送，因此新的远端 CI 运行尚未验证。
- Windows、Linux 与 macOS x64 的安装包运行尚未验证。

## Star History

<a href="https://www.star-history.com/?repos=moil-xm%2FSonaSub&type=date&legend=top-left">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/chart?repos=moil-xm/SonaSub&type=date&theme=dark&legend=top-left" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/chart?repos=moil-xm/SonaSub&type=date&legend=top-left" />
   <img alt="Star History Chart" src="https://api.star-history.com/chart?repos=moil-xm/SonaSub&type=date&legend=top-left" />
 </picture>
</a>
