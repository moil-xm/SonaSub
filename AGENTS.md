# SonaSub

开源实时字幕项目。当前已实现桌面端与本地引擎的真实通信和进程生命周期；pnpm workspace + Vite+ 管理 JS/TS 工具链，Tauri 与 WXT 保留各自构建入口。

## 技术栈与模块职责

| 模块                | 职责                                               | 技术                                                                           |
| ------------------- | -------------------------------------------------- | ------------------------------------------------------------------------------ |
| `apps/desktop`      | React 状态/操作界面；Rust 进程、连接与生命周期管理 | React + TypeScript、TanStack Router/Query/Form、Zustand、Tauri 2 + Rust        |
| `apps/engine`       | 本机 WebSocket 服务与真实运行信息                  | Node.js 24 LTS；开发运行 TypeScript，分发运行自包含构建产物                    |
| `apps/extension`    | Chrome 扩展（Manifest V3）                         | WXT + React + TypeScript                                                       |
| `packages/ui`       | 共享组件、主题变量、图标、样式                     | shadcn/ui、Tailwind CSS 4 + CSS Variables、Iconify 静态图标、Day.js 由应用引入 |
| `packages/protocol` | 桌面后端与引擎的通信契约                           | TypeScript 类型与边界解析                                                      |

依赖方向与导入方式见 [docs/architecture.md](docs/architecture.md)。

## 边界

- apps 之间不导入彼此内部源码；packages 不依赖 apps。
- `packages/ui` 不调用 Tauri、Chrome 扩展或 Node 专属 API。
- 原生能力放 `src-tauri`，本地处理能力放 `apps/engine`，平台代码留在对应应用。
- 跨应用复用只通过 `packages/*`，新增导出同步写入该包 `exports`。
- React 只通过 Tauri IPC 操作引擎；进程句柄、端口、认证信息与生命周期留在 Rust。引擎只监听 `127.0.0.1`，认证后才接受控制请求。

## 文件组织与命名

- 文件与目录使用 kebab-case；React 组件与类型使用 PascalCase，函数与变量使用 camelCase。
- 桌面端页面放 `apps/desktop/src/routes`，路由注册在 `src/routeTree.ts`；布局组件放 `src/components`。
- 共享组件放 `packages/ui/src/components/<name>.tsx`，包内引用用 `#lib/*`，桌面端内部引用用 `@/*`。
- 依赖声明在实际使用它的 workspace；开发工具放 `devDependencies`；版本固定为精确版本（`.npmrc`）。

## TypeScript / React / Rust

- TypeScript 严格模式（`tsconfig.base.json`：`strict`、`noUncheckedIndexedAccess`、`verbatimModuleSyntax`），类型导入使用 `import type`。
- React 使用函数组件与 hooks，不引入 class 组件；组件 props 用 `React.ComponentProps<...>` 组合。
- Rust 遵循 `cargo fmt` 与 `cargo clippy -D warnings`；新增 Tauri 命令时同步更新 `capabilities`。
- 遵循 KISS、DRY、YAGNI：信任内部类型与接口契约，只在外部输入和系统边界做必要校验；不写重复检查、无意义 try-catch、吞错兜底或没有复用需求的抽象。细则见 [.cursor/rules/minimal-api-surface.mdc](.cursor/rules/minimal-api-surface.mdc)。
- 不用假字幕、模拟识别结果或空实现伪装业务功能已完成。

## shadcn / Tailwind / 主题

- 组件按需用 `pnpm dlx shadcn@latest add <component>` 添加到 `packages/ui`（`components.json` 已配置于 `packages/ui` 与 `apps/desktop`）；优先官方组件与组合方式，不建立重复封装体系。
- 样式只用 Tailwind 工具类与 `globals.css` 中的 CSS 变量；新增颜色先加变量并在 `@theme inline` 中映射，不写硬编码色值。
- 主题切换通过 `@sonasub/ui/theme` 的 store 与 `<html class="dark">`。
- 图标用 `@iconify/react` 搭配 `@iconify-icons/lucide/<name>` 静态导入，不使用运行时在线加载。

## 状态职责

- TanStack Query：引擎状态与操作结果；本地 IPC 查询和操作使用 `networkMode: 'always'`，Rust 状态事件更新/失效缓存。
- TanStack Form：表单字段与校验状态（依赖已固定，尚无表单）。
- Zustand：本地交互状态，如主题；不存放异步资源缓存。

## 命令

在仓库根目录执行：

```bash
pnpm install                 # 安装依赖（会触发 wxt prepare）
pnpm check                   # Vite+ 格式 + 代码检查 + 类型检查
pnpm check:fix               # 自动修复格式与可修复的 lint 问题
pnpm typecheck               # 仅类型检查
pnpm lint                    # 仅代码检查
pnpm fmt                     # 仅格式化
pnpm check:rust              # cargo fmt --check + clippy -D warnings
pnpm build                   # vp run -r build：desktop(Vite+)、engine(vp pack)、extension(WXT)
pnpm test                    # 协议与真实 Node.js 引擎测试
pnpm dev:desktop             # 桌面前端开发服务器（http://localhost:5173）
pnpm dev:desktop:app         # Tauri 桌面应用（编译 Rust）
pnpm dev:extension           # WXT 扩展开发（Chrome）
pnpm start:engine            # 无 Rust 认证信息时拒绝独立启动（预期失败）
pnpm --filter @sonasub/desktop prepare:engine # 准备分发 Node.js 与引擎产物
pnpm --filter @sonasub/desktop tauri:build    # 构建当前平台安装包
```

## Skills

项目 Skills 位于 [.agents/skills](.agents/skills)，按任务选择，不要求全部读取：

| 任务                                       | Skill                                                                |
| ------------------------------------------ | -------------------------------------------------------------------- |
| workspace、依赖、工具链、导出、脚本        | [project-workspace](.agents/skills/project-workspace/SKILL.md)       |
| 页面、共享组件、主题、表单、前端状态       | [project-frontend](.agents/skills/project-frontend/SKILL.md)         |
| 桌面端/Rust/引擎/扩展/共享包职责或依赖变更 | [project-boundaries](.agents/skills/project-boundaries/SKILL.md)     |
| 按改动范围选择检查并整理交付               | [project-verification](.agents/skills/project-verification/SKILL.md) |

只有形成稳定、重复、项目特有的判断规则时才新增 Skill，不为每个第三方库单独建 Skill。

## 交付要求

- 说明改动范围与所属模块；列出实际执行的验证命令与结果，以及当前环境未验证的内容。
- 修改前后检查 `git status`，保留已有 staged、unstaged 与 untracked 改动，不做无关恢复、删除或格式化。
- 仅文档或 Skills 改动：检查 frontmatter、引用路径、`git diff --check`，不要求编译。

## 当前阶段（临时，非永久规则）

- 本阶段已实现引擎启动、连接、状态查询、停止、重启、失败提示与退出清理。
- 待后续任务确定：音频采集、语音识别、翻译、模型下载、多标签页处理与扩展采集。
- `packages/protocol` 只包含当前引擎协议；桌面端尚无 TanStack Form 表单；扩展仅有 popup。
- 这些限制随任务推进解除，不作为长期约束。
