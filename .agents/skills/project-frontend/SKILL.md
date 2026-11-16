---
name: project-frontend
description: Use when adding or changing SonaSub pages, routes, shared shadcn/ui components, theme variables, icons, TanStack Form forms, or Query/Zustand frontend state. Not for Rust, engine, or workspace tooling.
---

# project-frontend

## Read First

- [AGENTS.md](../../../AGENTS.md): shadcn/Tailwind/theme principles, state responsibilities, current stage.
- [react-conventions](../react-conventions/SKILL.md): naming, type files, conditional expressions and Query/Zustand habits for handwritten React/TSX.
- [docs/architecture.md](../../../docs/architecture.md): imports and style scanning.
- Desktop: [routeTree.ts](../../../apps/desktop/src/routeTree.ts), [main.tsx](../../../apps/desktop/src/main.tsx), [app-layout.tsx](../../../apps/desktop/src/components/app-layout.tsx), `apps/desktop/components.json`.
- Shared UI: `packages/ui/components.json`, [globals.css](../../../packages/ui/src/styles/globals.css), [theme/index.ts](../../../packages/ui/src/theme/index.ts), the target component file.

## Steps

1. Decide ownership: cross-app base components go to `packages/ui/src/components/<name>.tsx`; desktop-only pages and layouts go to `apps/desktop/src/routes` or `src/components`; extension UI goes to `apps/extension/entrypoints`.
2. Adding a shadcn component: run `pnpm dlx shadcn@latest add <component>` from `apps/desktop` so `components.json` routes it into `packages/ui`; check that generated imports use `#lib/utils` and that `packages/ui/package.json` `exports` covers the new path and dependencies. Prefer official components and composition; do not add another wrapper layer.
3. Adding a route: write the page component in `src/routes`, register it in `routeTree.ts` with `createRoute` and add it to `rootRoute.addChildren`.
4. Styling: use Tailwind utilities and the variables in `globals.css` only; new colors start as CSS variables mapped in `@theme inline`. The app CSS entry does `@import '@sonasub/ui/globals.css'` and then covers its own source with `@source`; JS imports only the app CSS entry, never `globals.css` directly.
5. Icons: `import icon from '@iconify-icons/lucide/<name>'`, then `<Icon icon={icon} />`; add new icon-set dependencies to the workspace that uses them.
6. State: async resources go to TanStack Query (`QueryClientProvider` is already in `main.tsx`); forms use TanStack Form; local interaction state uses Zustand, following `useThemeStore`. Never put async caches into Zustand.
7. Platform APIs (Tauri, `chrome.*`, Node) are called only inside the owning app; shared components receive data and callbacks through props.

## Constraints

- Follow the naming, strict typing and KISS/DRY/YAGNI rules in AGENTS.md; never fake business functionality with placeholder data.
- There is no subtitle/recognition/translation UI requirement at this stage; do not pre-build related pages or state.

## Verification

- `pnpm check`; desktop changes also run `pnpm --filter @sonasub/desktop build`, extension changes also run `pnpm --filter @sonasub/extension build`.
- Visual/interaction changes: check them for real with `pnpm dev:desktop` or `pnpm dev:extension`; state clearly when the UI cannot be opened.

## Delivery

Added/changed components, routes and state ownership, shadcn-generated files and `exports` changes, commands run with results, and unverified visual items.
