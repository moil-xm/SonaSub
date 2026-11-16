# 模块职责与依赖边界

本文描述当前已实现的模块职责、允许依赖与桌面端/本地引擎通信链路，不包含尚未接入的音频、识别、翻译或扩展采集流程。

## 模块职责

| 路径                | 职责                                                                   | 构建入口                                                               |
| ------------------- | ---------------------------------------------------------------------- | ---------------------------------------------------------------------- |
| `apps/desktop`      | React 状态与操作界面；Rust 管理引擎进程、连接、认证和退出清理          | Vite+ (`vp dev` / `vp build`)、Tauri CLI (`tauri dev` / `tauri build`) |
| `apps/engine`       | Node.js 24 本机 WebSocket 服务，提供真实运行信息与受认证的生命周期控制 | `node src/main.ts`、Vite+ (`vp pack`)                                  |
| `apps/extension`    | Chrome 扩展（Manifest V3），当前仅 popup                               | WXT (`wxt` / `wxt build`)                                              |
| `packages/ui`       | 共享 shadcn/ui 组件、主题变量、图标与样式                              | 无独立构建，源码由消费方打包                                           |
| `packages/protocol` | 当前阶段桌面后端与引擎的通信契约                                       | TypeScript 源码由消费方解析与打包                                      |

原生能力（窗口、系统交互等）归属 `apps/desktop/src-tauri`；本地处理能力归属 `apps/engine`；浏览器页面与扩展能力归属各自应用。

## 允许的依赖方向

```mermaid
graph TD
  desktop[apps/desktop] --> ui[packages/ui]
  extension[apps/extension] --> ui
  desktop --> protocol[packages/protocol]
  engine[apps/engine] --> protocol
  desktop -.Tauri IPC.-> rust[src-tauri]
  rust -.127.0.0.1 WebSocket.-> engine
```

- 箭头表示源码依赖；虚线表示运行时通信。React 不直接连接引擎，进程句柄、端口、连接凭据与生命周期只由 Rust 持有。
- apps 之间不直接导入彼此的内部源码；跨应用复用只能通过 `packages/*`。
- packages 不依赖任何 app。
- `packages/ui` 只使用浏览器通用 API，不调用 Tauri、Chrome 扩展或 Node 专属 API。
- 平台相关代码留在对应应用内。

## 引擎生命周期

```text
stopped/failed → starting → running → stopping → stopped
                     ↘ failed（启动、握手、认证或信息获取失败）
running → failed（进程意外退出）
```

- Rust 用单一转换锁串行化启动、停止和重启；重复启动复用已运行的引擎，不扫描端口、不自动无限重启。
- 启动成功必须完成进程创建、stdout 握手、WebSocket 认证和 `engine.info` 查询。
- 启动失败会清理本次子进程，并把引擎 stderr 的最近输出保留在失败原因中；普通日志与启动握手分离。
- Rust 监听受管理子进程退出，并通过 `engine://state` 通知前端更新或失效 TanStack Query 缓存。
- 停止先发送受认证的 `engine.shutdown`，等待进程退出；超时只强制终止本次创建的子进程。应用退出执行同一清理路径。
- 前端查询与操作使用 `networkMode: 'always'`；Zustand 仍只保存主题等局部交互状态。

## 开发与分发资源

- 开发：Rust 使用当前 Node.js 24 运行 `apps/engine/src/main.ts`，工作目录为 `apps/engine`。
- 构建：`pnpm --filter @sonasub/desktop prepare:engine` 校验并准备固定版本 Node.js，构建自包含引擎产物，写入 `apps/desktop/src-tauri/resources/`。
- macOS 安装包：运行时与引擎位于 `SonaSub.app/Contents/Resources/`；Rust 统一解析资源路径。
- 运行时二进制与生成产物不提交仓库；版本、下载文件名与 SHA-256 记录在 `apps/desktop/src-tauri/scripts/node-runtime.json`。
- 当前默认打包目标为 macOS `.app`；其他平台的运行时配置已列出，但安装包运行仍需各自平台验证。

## 导入方式

- 共享 UI：`@sonasub/ui/components/*`、`@sonasub/ui/lib/*`、`@sonasub/ui/theme`、`@sonasub/ui/globals.css`。
- `packages/ui` 内部使用 `#lib/*` 包内别名；桌面端使用 `@/*` 指向 `apps/desktop/src`。
- 新增导出必须同时写入该包 `package.json` 的 `exports`，否则消费方无法解析。
- 声明 `vite` 的 workspace 使用 `npm:@voidzero-dev/vite-plus-core@<vite-plus 版本>` 别名，与 Vite+ 内置内核保持一致；WXT 与 Tauri 仍走各自 CLI。

## 样式与主题

- `packages/ui/src/styles/globals.css` 是 Tailwind 4 与 CSS 变量的唯一入口，包含 `@theme inline` 映射与 `.dark` 变量。
- 各应用在自身 CSS 入口中 `@import '@sonasub/ui/globals.css'`，再用 `@source` 扫描本应用源码；共享包通过 `globals.css` 内的 `@source` 扫描。JS 只导入应用自己的 CSS 入口。
- 主题状态由 `@sonasub/ui/theme` 的 Zustand store 管理，通过 `<html class="dark">` 生效。
- 应用图标位于 `apps/desktop/src-tauri/icons/icon.png`，由 `tauri.conf.json` 引用。
