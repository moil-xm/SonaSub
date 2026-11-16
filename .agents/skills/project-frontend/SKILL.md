---
name: project-frontend
description: Use when adding or changing SonaSub pages, routes, shared shadcn/ui components, theme variables, icons, TanStack Form forms, live subtitle rendering, or Query/Zustand frontend state. Not for Rust inference, native audio, or workspace tooling.
---

# project-frontend

## Read First

- [AGENTS.md](../../../AGENTS.md): shadcn/Tailwind/theme principles, state responsibilities, current stage.
- [react-conventions](../react-conventions/SKILL.md): naming, type files, conditional expressions and Query/Zustand habits for handwritten React/TSX.
- [docs/architecture.md](../../../docs/architecture.md): imports and style scanning.
- [docs/models.md](../../../docs/models.md): model readiness and capability messages.
- Desktop: [routeTree.ts](../../../apps/desktop/src/routeTree.ts), [main.tsx](../../../apps/desktop/src/main.tsx), [session.ts](../../../apps/desktop/src/lib/session.ts), [session-cache.ts](../../../apps/desktop/src/lib/session-cache.ts), [app-layout.tsx](../../../apps/desktop/src/components/app-layout.tsx), `apps/desktop/components.json`.
- Shared UI: `packages/ui/components.json`, [globals.css](../../../packages/ui/src/styles/globals.css), [theme/index.ts](../../../packages/ui/src/theme/index.ts), the target component file.

## Steps

1. Decide ownership: cross-app base components go to `packages/ui/src/components/<name>.tsx`; desktop-only pages and layouts go to `apps/desktop/src/routes` or `src/components`; extension UI goes to `apps/extension/entrypoints`.
2. Adding a shadcn component: run `pnpm dlx shadcn@latest add <component>` from `apps/desktop` so `components.json` routes it into `packages/ui`; check that generated imports use `#lib/utils` and that `packages/ui/package.json` `exports` covers the new path and dependencies. Prefer official components and composition; do not add another wrapper layer.
3. Adding a route: write the page component in `src/routes`, register it in `routeTree.ts` with `createRoute` and add it to `rootRoute.addChildren`.
4. Styling: use Tailwind utilities and the variables in `globals.css` only; new colors start as CSS variables mapped in `@theme inline`. The app CSS entry does `@import '@sonasub/ui/globals.css'` and then covers its own source with `@source`; JS imports only the app CSS entry, never `globals.css` directly.
5. Icons: `import icon from '@iconify-icons/lucide/<name>'`, then `<Icon icon={icon} />`; add new icon-set dependencies to the workspace that uses them.
6. State: async resources go to TanStack Query (`QueryClientProvider` is already in `main.tsx`); forms use TanStack Form; local interaction state uses Zustand, following `useThemeStore`. Never put async caches into Zustand. Local Tauri queries and mutations use `networkMode: 'always'`.
7. Live subtitles: apply Rust patches by stable subtitle ID. Source text, speaker attribution and translation update independently; partial text is replaced by revision, never appended as a new row. Remove Tauri event listeners in effect cleanup and keep subtitle history bounded by the snapshot/config limit.
8. Configuration: render only backend-declared models, languages and modes. Disable unsupported explicit modes with the backend reason; never infer capability from UI state. Save before starting and lock edits while a session is starting/running/stopping.
9. Platform APIs (Tauri, `chrome.*`, Node) are called only inside the owning app; shared components receive data and callbacks through props.

## Constraints

- Follow the naming, strict typing and KISS/DRY/YAGNI rules in AGENTS.md; never fake business functionality with placeholder data.
- Never display a session as ready before the backend reports readiness, and never fabricate partial/final/translation/speaker states.

## Verification

- `pnpm check`; desktop changes also run `pnpm --filter @sonasub/desktop build`, extension changes also run `pnpm --filter @sonasub/extension build`.
- Visual/interaction changes: check them for real with `pnpm dev:desktop` or the packaged `.app`; for caption changes verify real patch ordering, translation and speaker updates. State clearly when the UI cannot be opened.

## Delivery

Added/changed components, routes and state ownership, shadcn-generated files and `exports` changes, commands run with results, and unverified visual items.
