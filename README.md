# spacemouse-proxy

WebSocket proxy for 3DConnexion SpaceMouse on macOS. Reads 6DOF data via the native 3DConnexion framework and exposes it over `ws://127.0.0.1:18944`.

## Requirements

- **macOS** (Apple Silicon or Intel)
- **3DConnexion driver** — [3DxWare 10](https://3dconnexion.com/us/drivers-application/3dxware-10/)
- **Rust toolchain** (for building from source)

## Install

```sh
cargo install spacemouse-proxy
```

## Usage

```sh
spacemouse-proxy
```

The proxy starts a WebSocket server on `ws://127.0.0.1:18944` and automatically:

- Detects when Figma (desktop or browser) is in the foreground
- Captures SpaceMouse input and streams it over WebSocket
- Releases SpaceMouse back to other 3D apps (Blender, Maya, etc.) when Figma loses focus

The 3DConnexion driver is **never stopped or killed** — the proxy registers/unregisters as a client dynamically.

## Run as a service

Install as a launchd service to start automatically on login:

```sh
spacemouse-proxy --install
```

This creates a plist at `~/Library/LaunchAgents/com.spacemouse-proxy.plist`, starts the service immediately, and configures it to restart on crash.

To stop and remove:

```sh
spacemouse-proxy --uninstall
```

Logs: `/tmp/spacemouse-proxy.log`

## How it works

1. Connects to the 3DConnexion framework as a manual client
2. Polls `NSWorkspace.frontmostApplication` every ~128ms to detect Figma Desktop or major browsers (Chrome, Safari, Firefox, Arc, Edge, Brave, Opera, Vivaldi)
3. When a target app is frontmost **and** at least one WebSocket client is connected, the proxy registers with the framework and captures SpaceMouse data
4. When focus leaves, it fully unregisters (calls `UnregisterConnexionClient` + `CleanupConnexionHandlers`), allowing other apps to receive SpaceMouse input natively
5. Axis data is processed with deadzone filtering, EMA smoothing, and normalization to [-1, 1]

## Protocol

JSON messages sent at ~60fps:

```json
{ "axes": [0.123, -0.456, 0.0, 0.0, 0.089, -0.023], "buttons": 0 }
```

- `axes`: `[x, y, z, rx, ry, rz]` — normalized to [-1.0, 1.0]
- `buttons`: bitmask of pressed buttons

## Used by

- [SpaceMouse Anywhere](https://github.com/gocivici/spacemouse-anywhere) — Figma plugin for navigating designs with a SpaceMouse

## License

MIT
