# NEXUS Overlay — preview 0.1.0

A local-first Windows gaming overlay prototype. This version is the **UI shell only**: system telemetry, global hotkey, launcher detection, SoundCloud controls, Discord call controls, and installer integration are not connected yet. The UI intentionally shows dashes rather than fake live readings.

## Stack
- Tauri 2 + Rust (Windows host)
- React + TypeScript + Vite
- Geist Sans (bundled locally through `@fontsource/geist-sans`, no font CDN)
- Lucide React icons (no emoji icons)

## Requirements for Windows development
1. Install Node.js LTS.
2. Install Rust using rustup.
3. Install Microsoft C++ Build Tools (Desktop development with C++) and WebView2 Runtime if missing.
4. Extract this project, open a terminal in this folder, and run `npm install`.
5. Run `npm run tauri dev` after the Tauri Rust shell has been added/configured.

## Current prototype
For a quick UI preview without the native host, run `npm install` then `npm run dev` and open the local URL Vite prints.

## Planned implementation order
1. Native Tauri window and global Insert hotkey.
2. Launcher detection and explicit lifecycle rules.
3. Native performance telemetry, with measured overhead.
4. Windows media sessions and SoundCloud capability check.
5. Supported Discord call status/actions.
6. Russian/English localization, preferences, and Windows installer.

No credentials or tokens belong in source control. Do not commit local API credentials.
