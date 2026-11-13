# KeepItInCheck

Edge-docked Windows panel (Tauri 2 + Rust + React/TS/Tailwind 4/Framer Motion) that shows RAM/CPU
per app, flags idle "dead weight" apps and closes them safely. Proprietary (see LICENSE): keep
`"license": "UNLICENSED"`, `publish = false` and the copyright headers in `src-tauri/src/lib.rs`,
`src-tauri/src/main.rs` and `src/main.tsx`.

## Commands

- Install: `npm install`
- Dev (Tauri + Vite HMR): `npm run app:dev`
- UI only, mock data in a browser: `npm run dev` then http://localhost:1420
- Rust tests: `npm run test:rust` (classifier, grouping, history, dock, settings, Authenticode)
- TS tests: `npm test` (vitest) and `npm run typecheck`
- Release exe only: `npx tauri build --no-bundle` -> `src-tauri/target/release/keepitincheck.exe`
- Installers: `npm run app:build` -> `src-tauri/target/release/bundle/{nsis,msi}/`
- Toolchain lives in `%USERPROFILE%\.cargo\bin`; in Git Bash prepend it: `export PATH="$HOME/.cargo/bin:$PATH"`.

## Layout

- `src-tauri/src/sampler.rs`: single background thread; `Shared` state; emits `snapshot` (open) or `pulse` (collapsed)
- `classifier.rs` (pure, tested), `history.rs`, `grouping.rs`, `protect.rs`: dead-weight logic
- `cleanup.rs`: close flow, log, relaunch; `dock.rs` + `shell.rs`: window/tray/hotkey/backdrop
- `platform/`: `trait Platform`; Windows impl in `platform/windows/`, stub in `platform/other.rs`
- `src/`: React UI; `lib/api.ts` wraps IPC and lazy-loads `lib/mock.ts` only outside Tauri
- `docs/dead-weight-score.md`: user doc for the classifier. Keep it in sync with `classifier.rs`.

## Rules and gotchas

- Performance is a feature: keep the tick to one `NtQuerySystemInformation` call; no per-process
  syscalls per tick except cached first-sight lookups (exe path, version info, signature).
- Never force-kill a process that owns a visible window from the automatic cleanup path; only
  `cleanup::force_close` (explicit per-app confirmation) may do that. Always kill via
  `Platform::kill(pid, start_time)` so recycled PIDs are never hit.
- Signature checks must stay offline (`WTD_REVOKE_NONE` + `WTD_CACHE_ONLY_URL_RETRIEVAL`).
- `WTHelper*` functions need the `Win32_Security_Cryptography_Sip` feature of the `windows` crate.
- The UI owns the expand/collapse animation: Rust emits `toggle`, the UI animates and then calls
  `set_expanded`. Hover-expand must not steal focus (`focus: false`).
- Debug: `KIC_DUMP=<file>` writes every full snapshot as JSON (none while collapsed: idle ticks only update history).
- Settings change only through `Shared::update_settings` (+ `shell::apply_settings`); pause only via `shell::set_paused`.
- Drag-to-snap is explicit (`begin_drag` command from the title bar), never inferred from window `Moved` events.
- Measuring footprint: build the UI (`npx vite build`), serve it with `npx vite preview --port 1420`, run the debug exe
  with `KIC_DUMP` and average `selfCpu`/`selfMem`. Release numbers (2026-11): collapsed ~0.01% CPU / ~25-35 MB, open ~0.5%.
- Startup apps: only ever write the `Explorer\StartupApproved` flag (byte 0: 2 on, 3 off), never the
  Run value or shortcut. Tests use a scratch key `HKCU\Software\KeepItInCheck-test`, never the real Run key.
- Weekly report data lives in `usage.json` (Shared::usage); flushed every 5 min and on quit (Shared::quit waits).
- PowerShell 5 `Set-Content -Encoding utf8` writes a BOM; settings loading tolerates it, but prefer `[IO.File]::WriteAllText`.
- Desktop screenshots for visual checks: PowerShell `System.Drawing` `CopyFromScreen` (call
  `SetProcessDPIAware` first) and read the PNG.
