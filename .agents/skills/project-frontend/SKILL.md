---
name: project-frontend
description: Use when adding or changing SonaSub pages, routes, shared shadcn/ui components, theme variables, icons, TanStack Form forms, or Query/Zustand frontend state. Not for Rust, engine, or workspace tooling.
---

# project-frontend

## 先读

- [AGENTS.md](../../../AGENTS.md)：shadcn/Tailwind/主题原则、状态职责、当前阶段。
- [docs/architecture.md](../../../docs/architecture.md)：导入方式与样式扫描。
- 桌面端：[routeTree.ts](../../../apps/desktop/src/routeTree.ts)、[main.tsx](../../../apps/desktop/src/main.tsx)、[app-layout.tsx](../../../apps/desktop/src/components/app-layout.tsx)、`apps/desktop/components.json`。
- 共享 UI：`packages/ui/components.json`、[globals.css](../../../packages/ui/src/styles/globals.css)、[theme/index.ts](../../../packages/ui/src/theme/index.ts)、目标组件文件。

## 步骤

1. 判断归属：跨应用复用的基础组件放 `packages/ui/src/components/<name>.tsx`；只服务桌面端的页面/布局放 `apps/desktop/src/routes` 或 `src/components`；扩展 UI 放 `apps/extension/entrypoints`。
2. 新增 shadcn 组件：在 `apps/desktop` 目录执行 `pnpm dlx shadcn@latest add <component>`，由 `components.json` 路由到 `packages/ui`；核对生成文件的导入用 `#lib/utils`，并确认 `packages/ui/package.json` 的 `exports` 覆盖新路径与新依赖。优先官方组件与组合，不再包一层。
3. 新增路由：在 `src/routes` 写页面组件，在 `routeTree.ts` 用 `createRoute` 注册并加入 `rootRoute.addChildren`。
4. 样式：只用 Tailwind 工具类与 `globals.css` 变量；新颜色先加 CSS 变量并在 `@theme inline` 映射。应用 CSS 入口 `@import '@sonasub/ui/globals.css'` 后用 `@source` 覆盖本应用源码；JS 只导入应用 CSS 入口，不再单独导入 `globals.css`。
5. 图标：`import icon from '@iconify-icons/lucide/<name>'` 后 `<Icon icon={icon} />`；新增图标集时依赖加到使用它的 workspace。
6. 状态：异步资源用 TanStack Query（`QueryClientProvider` 已在 `main.tsx`）；表单用 TanStack Form；本地交互状态用 Zustand，参考 `useThemeStore`。不把异步缓存放进 Zustand。
7. 平台 API（Tauri、`chrome.*`、Node）只在对应应用内调用；共享组件通过 props 接收数据与回调。

## 约束

- 遵循 AGENTS.md 的命名、严格类型与 KISS/DRY/YAGNI；不用假数据伪装业务功能。
- 当前阶段无字幕/识别/翻译 UI 需求，不预建相关页面或状态。

## 验证

- `pnpm check`；桌面端改动再 `pnpm --filter @sonasub/desktop build`，扩展改动再 `pnpm --filter @sonasub/extension build`。
- 视觉/交互改动：`pnpm dev:desktop` 或 `pnpm dev:extension` 实际查看；无法打开界面时明确说明。

## 交付

新增/修改的组件、路由与状态归属，shadcn 生成文件与 `exports` 变更，执行命令与结果，未验证的视觉项。
