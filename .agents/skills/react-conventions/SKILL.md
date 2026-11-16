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
- Hooks are named `useXxx`; file names follow the AGENTS.md kebab-case rule (for example `use-now.ts`), not mall's camelCase file names.
- Module-level semantic constants use `UPPER_SNAKE_CASE`.
- Component props types are named `XxxProps` and compose native element or existing component props with `React.ComponentProps<...>`.

## Type Files

- Handwritten business types live in an adjacent `*.type.ts` (`ComponentName.type.ts` for TSX components, `use-xxx.type.ts` for hooks), imported at runtime files with `import type`.
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
