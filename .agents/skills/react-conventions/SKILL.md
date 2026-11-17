---
name: react-conventions
description: Use when writing, changing, or reviewing handwritten React/TypeScript/TSX code in SonaSub apps or packages - component and hook naming, colocated type files, conditional rendering, component splitting, public exports, and Query/Zustand usage habits. Not for generated files or shadcn CLI output.
---

# react-conventions

React/TSX writing habits derived from mall-platform's code-conventions, use-request and zustand skills, rewritten for SonaSub's stack and naming rules. They apply to all handwritten React code; where they conflict with AGENTS.md, AGENTS.md wins.

They do not apply to generated artifacts (`routeTree.gen.ts`, WXT-generated files, `dist`), shadcn CLI generated components or third-party code; never refactor untouched legacy code just to apply this Skill.

## Naming and Callbacks

- Boolean values prefer the `is`, `has`, `can`, `should` prefixes.
- Callback props use `onXxx`; event handlers inside a component use `handleXxx`.
- Hooks are named `useXxx`; non-component module files follow the AGENTS.md kebab-case rule (for example `use-now.ts`), not mall's camelCase file names.
- Module-level semantic constants use `UPPER_SNAKE_CASE`.
- Component props types are named `XxxProps` and compose native element or existing component props with `React.ComponentProps<...>`.

## Component Directories

- Every React component owns a directory named after it in kebab-case, mirroring the Ant Design `components/<name>/` layout: `src/components/live-subtitles/`.
- The directory holds the implementation file in PascalCase (`LiveSubtitles.tsx`), the named sub-components of that component (`SubtitleRow.tsx`), the colocated `LiveSubtitles.test.tsx` and `LiveSubtitles.type.ts`, and an `index.ts` public entry.
- `index.ts` only re-exports the component, its props types and the sub-components that form the public surface; it holds no implementation logic. Private sub-components stay unexported.
- Consumers import the directory (`@/components/live-subtitles`), never a file inside it. Sibling files inside one directory use relative paths (`./SubtitleRow`).
- One named React component per file. Plain helper functions stay in the file that uses them.
- Keep a component directory flat; add `style/`, `demo/` or `__tests__/` only when a real need appears.
- WXT entrypoints own their directory shape (`index.html`, `main.tsx`); the component file inside is PascalCase and imported directly by the entry.

## Module Directories

- A `.ts` module owns a directory named after it in kebab-case, with the same public-entry rule as a component: `src/lib/session/` holds `session.ts`, the colocated `session.type.ts` and an `index.ts` that re-exports the public API.
- A module keeps its kebab-case file name; PascalCase stays reserved for React components and types.
- Imports point at the directory (`@/lib/session`), never at a file inside it. A module that needs another module's types imports them from that module's public entry.
- A barrel may use `export type * from './module.type'` when the type file is the unit's whole type surface; otherwise list the types explicitly.
- A directory that groups sibling files of one kind is a collection, not a module: it keeps its own `index.ts` entry (`packages/i18n/src/resources/`).
- App entry files (`main.tsx`, `routeTree.ts`) stay at the workspace root.
- `packages/ui/src/components/*.tsx` and `packages/ui/src/lib/utils.ts` stay flat because the shadcn CLI writes flat files.

## Type Files

- Handwritten business types live in an adjacent `*.type.ts` inside the same directory (`ComponentName.type.ts` for TSX components, `module.type.ts` for a `module.ts` module, `use-xxx.type.ts` for hooks), imported at runtime files with `import type`.
- `*.type.ts` may contain only type declarations, `import type` and `export type`; it must not emit runtime code.
- Handwritten business types and component props use `type`, not `interface`; reserve `interface` for generated declarations or required third-party declaration merging.
- A component-private props type of two or three lines may stay in the component file; split it into `*.type.ts` once it is consumed across files or clearly grows.

## Conditional Expressions

- Handwritten TS/TSX does not use the ternary operator (`condition ? a : b`), including JSX children, JSX attributes, callback return values, object construction and collection transforms.
- Use early returns, `if`/`else`, or a named variable assigned through explicit branches instead.
- Optional chaining (`value?.field`), nullish coalescing (`value ?? fallback`) and JSX `&&` conditional rendering are allowed.

## Components and Abstraction

- Split a component or extract a hook only when independent behavior, state or rendering can be named and tested on its own; never create empty folders to satisfy a directory template.
- Inline one-off functions that only forward arguments, rename a single call or wrap a single callback without expressing a distinct concept.
- Build a shared abstraction only when at least two consumers need the same behavior and the abstraction removes real duplication; cross-app sharing goes to `packages/*`.
- Do not export implementation details for unit tests; test behavior through the public boundaries of components, hooks, utilities or services.
- Comment only non-obvious constraints and counter-intuitive behavior; skip formulaic lines such as "run xx" or "render xx component".

## Public Entry

- A module `index.ts` is a public entry only and holds no implementation logic; cross-module consumers import from the nearest public entry instead of referencing another module's implementation, hooks or type leaf files directly.
- Every component or module directory exposes its public API through `index.ts`; consumers import the directory, never the implementation file inside it.
- New public exports from `packages/ui` update the `package.json` `exports` (see [project-workspace](../project-workspace/SKILL.md)).

## TanStack Query (mall's useRequest Habit)

- Destructure the return value of `useQuery` / `useMutation`; never store the whole result object in a variable.
- Control queries that depend on other data with `enabled`; include only inputs that truly determine the data in `queryKey`.
- Derive rendered data from `data`; never mirror server data into local state or Zustand unless the user is editing a form snapshot.
- Command-style operations (start/stop/restart session and similar) use `useMutation`; use `mutateAsync` when the caller must await the result, `mutate` for pure UI triggers.
- When a child command affects parent data, pass an explicit refresh callback or invalidate the matching `queryKey` instead of letting the child mutate parent cache.
- Local IPC queries and actions use `networkMode: 'always'`; Rust state events update or invalidate the cache (see AGENTS.md).

## Zustand

- Define a store as `State & Actions` with domain actions as top-level properties, never nested under `actions`, and no generic `setState` / `setData` escape hatch.
- Keep the `create` result module-private; export narrow selector hooks and intention-revealing non-React functions, never the raw store or a whole-store subscription.
- Use atomic selectors for single values; read several values or actions with `useShallow` from `zustand/react/shallow` instead of returning new objects/arrays without an equality strategy.
- Name actions after domain events and keep validation plus related state transitions inside the action.
- One store carries one business domain and lifecycle; split when ownership, persistence or update frequency differs.
- Enable only the middleware actually needed: `persist(..., { name })` only for surviving restarts, `immer` only for deep nested updates; plain immutable `set` for shallow objects.
- Zustand holds local interaction state only; async resource caches belong to TanStack Query.

## Debounce/Throttle and Side Effects

- Debounce/throttle wrappers stay stable across renders and are cancelled on unmount; never create them inline in JSX props or on every render.
- Timers, subscriptions and event listeners are released in effect cleanup functions.

## Verification

Choose checks per [project-verification](../project-verification/SKILL.md); this Skill adds no extra full test or build gate.
