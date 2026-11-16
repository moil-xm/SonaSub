# SonaSub

[English](README.md) | **简体中文**

开源实时字幕项目。当前仓库处于工程骨架阶段：提供桌面应用壳、Chrome 扩展 popup 与本地引擎入口，以及统一的工程配置与协作规范。音频采集、语音识别、翻译等业务能力尚未实现。

## 技术栈

- 工程管理：pnpm workspace + Vite+（`vp` 统一管理格式、检查、类型检查与构建）
- 桌面：Tauri 2 + Rust，前端 React + TypeScript
- 样式与组件：Tailwind CSS 4 + CSS Variables、shadcn/ui、Iconify 静态图标
- 路由与状态：TanStack Router / Query / Form、Zustand、Day.js
- 本地引擎：Node.js 24 LTS 直接运行 TypeScript
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
pnpm start:engine       # 运行引擎入口并正常退出

pnpm check              # 格式 + 代码检查 + 类型检查
pnpm typecheck          # 仅类型检查
pnpm lint               # 仅代码检查
pnpm fmt                # 仅格式化
pnpm check:rust         # cargo fmt --check + clippy
pnpm build              # 构建 desktop / engine / extension
pnpm build:extension:zip # 打包扩展 zip
```

桌面原生后端也可单独构建：`pnpm --filter @sonasub/desktop exec tauri build --debug --no-bundle`。当前 `bundle.active` 为 `false`，不产出安装包。

扩展开发产物在 `apps/extension/.output/chrome-mv3-dev`，生产产物在 `apps/extension/.output/chrome-mv3`，可在 Chrome 扩展页以「加载已解压的扩展程序」载入。

## 结构

```
apps/desktop      桌面前端与 src-tauri 原生后端
apps/engine       本地处理引擎入口
apps/extension    Chrome 扩展 popup
packages/ui       共享组件、主题、图标与样式
packages/protocol 跨模块通信契约的归属位置（当前为空）
.agents/skills    项目专属 Skills
docs/architecture.md  模块职责与依赖边界
AGENTS.md         全仓库开发约定
```

开发约定见 [AGENTS.md](AGENTS.md)，模块边界见 [docs/architecture.md](docs/architecture.md)。
