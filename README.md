# NEXUS Overlay — preview 0.2.0

A local-first Windows gaming overlay built with Tauri 2, Rust, React, and TypeScript.

## Current functionality
- Transparent, always-on-top overlay; drag from non-interactive areas. **Insert** hides/shows it globally.
- Existing FPS / frame time (PresentMon), CPU / GPU / RAM, Ping and Packet Loss telemetry is retained.
- Windows SMTC media-session detection and Play/Pause/Next/Previous plus system volume keys. Browser pages are detectable only when the browser/site publishes a Windows media session.
- Discord voice state and mute/deafen/leave controls use Discord's documented RPC-over-IPC API. Set up a Discord Developer Portal app, authorize it in the UI, and keep its secret in Windows Credential Manager; no bot token or private API is used. RPC tester/approval restrictions apply.
- Optional background launch at Windows sign-in is configured in Settings. It starts NEXUS hidden with `--background`; turn it off there to remove the per-user Run entry. If disabled, launch NEXUS once after signing in, then it can remain hidden and respond to Insert.

## Build a Windows installer
1. Install Node.js LTS, Rust (rustup), Microsoft C++ Build Tools, and WebView2 Runtime.
2. Run `npm install` once.
3. Run `npm run build:windows`. This verifies and downloads the official PresentMon 2.5.1 console binary from Intel's GameTechDev GitHub release, then invokes Tauri's Windows bundler.
4. The NSIS `.exe` installer is produced under `src-tauri/target/release/bundle/nsis/` (MSI is produced too).

The PresentMon asset is pinned by SHA-256 in `scripts/prepare-windows.mjs`; it is not committed to the repository. No PowerShell command is needed to launch the installed application.

## Privacy / integration boundaries
Telemetry and media session access run locally. No SoundCloud API key is needed for ordinary Windows media controls. Credentials for any future account-level OAuth must be stored in Windows Credential Manager, never in source or the frontend bundle. Discord RPC requires a registered app and explicit user authorization. The lightweight game watcher needs a process running in the background; fully automatic detection without adding NEXUS (or a watcher) to logon startup is not technically possible.
