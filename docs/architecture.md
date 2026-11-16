# Module Responsibilities and Dependency Boundaries

This document describes the modules that exist today, the dependencies they may take and the desktop/local-engine communication path. Audio, recognition, translation and extension-capture flows are not covered because they are not connected yet.

## Module Responsibilities

| Path                | Responsibility                                                                                          | Build entry                                                            |
| ------------------- | ------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------- |
| `apps/desktop`      | React state and control UI; Rust manages the engine process, connection, auth and exit cleanup          | Vite+ (`vp dev` / `vp build`), Tauri CLI (`tauri dev` / `tauri build`) |
| `apps/engine`       | Node.js 24 loopback WebSocket service with real runtime information and authenticated lifecycle control | `node src/main.ts`, Vite+ (`vp pack`)                                  |
| `apps/extension`    | Chrome extension (Manifest V3), popup only for now                                                      | WXT (`wxt` / `wxt build`)                                              |
| `packages/ui`       | Shared shadcn/ui components, theme variables, icons and styles                                          | No standalone build; consumers bundle the source                       |
| `packages/protocol` | Current desktop-backend/engine communication contract                                                   | TypeScript source parsed and bundled by consumers                      |

Native capabilities (windows, system interaction and similar) belong to `apps/desktop/src-tauri`; local processing belongs to `apps/engine`; browser pages and extension capabilities belong to their own apps.

## Allowed Dependency Direction

```mermaid
graph TD
  desktop[apps/desktop] --> ui[packages/ui]
  extension[apps/extension] --> ui
  desktop --> protocol[packages/protocol]
  engine[apps/engine] --> protocol
  desktop -.Tauri IPC.-> rust[src-tauri]
  rust -.127.0.0.1 WebSocket.-> engine
```

- Solid arrows are source dependencies; dashed arrows are runtime communication. React never connects to the engine directly: the process handle, port, connection credentials and lifecycle are owned by Rust alone.
- Apps do not import each other's internal source; cross-app reuse goes through `packages/*` only.
- Packages do not depend on any app.
- `packages/ui` uses browser-standard APIs only; it never calls Tauri, Chrome extension or Node-only APIs.
- Platform-specific code stays inside the owning app.

## Engine Lifecycle

```text
stopped/failed → starting → running → stopping → stopped
                     ↘ failed (startup, handshake, auth or information failure)
running → failed (process exited unexpectedly)
```

- Rust serializes start, stop and restart behind a single transition lock; a repeated start reuses the running engine, and it never scans ports or restarts automatically in a loop.
- A successful start must complete process creation, the stdout handshake, WebSocket authentication and the `engine.info` query.
- A failed start cleans up that child process and keeps the most recent engine stderr output in the failure reason; regular logs and the startup handshake stay separate.
- Rust watches the managed child process and notifies the frontend through `engine://state` so TanStack Query caches are updated or invalidated.
- Stopping sends an authenticated `engine.shutdown` and waits for the process to exit; on timeout it force-kills only the child it created. App exit runs the same cleanup path.
- Frontend queries and actions use `networkMode: 'always'`; Zustand still holds only local interaction state such as the theme.

## Development and Distribution Resources

- Development: Rust runs `apps/engine/src/main.ts` with the current Node.js 24, using `apps/engine` as the working directory.
- Build: `pnpm --filter @sonasub/desktop prepare:engine` verifies and prepares the pinned Node.js version, builds the self-contained engine artifact and writes it into `apps/desktop/src-tauri/resources/`.
- macOS bundle: the runtime and engine live in `SonaSub.app/Contents/Resources/`; Rust resolves resource paths in one place.
- Runtime binaries and generated artifacts are not committed; versions, download file names and SHA-256 values are recorded in `apps/desktop/src-tauri/scripts/node-runtime.json`.
- The current default packaging target is the macOS `.app`; runtime configurations for other platforms are listed, but installer execution still needs verification on each platform.

## Imports

- Shared UI: `@sonasub/ui/components/*`, `@sonasub/ui/lib/*`, `@sonasub/ui/theme`, `@sonasub/ui/globals.css`.
- `packages/ui` uses the `#lib/*` package-internal alias; the desktop uses `@/*` pointing at `apps/desktop/src`.
- New exports must be added to the package's `package.json` `exports`, otherwise consumers cannot resolve them.
- Workspaces that declare `vite` use the `npm:@voidzero-dev/vite-plus-core@<vite-plus version>` alias to match the Vite+ core; WXT and Tauri keep their own CLIs.

## Styles and Theme

- `packages/ui/src/styles/globals.css` is the single entry for Tailwind 4 and the CSS variables, including the `@theme inline` mapping and `.dark` variables.
- Each app imports `@sonasub/ui/globals.css` in its own CSS entry and then scans its own source with `@source`; the shared package scans itself through `@source` inside `globals.css`. JS imports only the app's own CSS entry.
- Theme state is managed by the Zustand store from `@sonasub/ui/theme` and takes effect through `<html class="dark">`.
- The app icon lives at `apps/desktop/src-tauri/icons/icon.png` and is referenced by `tauri.conf.json`.
