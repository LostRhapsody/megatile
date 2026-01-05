# MegaTile

An opinionated tiling manager for windows.

## Overview

MegaTile is a minimalist tiling manager that does the bare minimum to be effective. It has a standardized set of keybinds, a very small status bar, and a single tiling algorithm.

MegaTile will **not** suite everyone.

## MegaTile Features and Goals

MegaTile aims to be simple, fast, and effective.

- **Fast**: Rust with bindings for the Window's API via `windows-rs`. Minimal dependencies and responsibilities.
- **Simple**: We manage windows, workspaces, and keybinds to control them, nothing else.
- **Effective**: Manage 1-9 workspaces. Near instant response time. No animations, no lag.
- **System Tray**: Runs in the system tray, right click the icon to exit.
- **Filtering**: Doing our best to filter windows we don't want to tile.
- **Mutliple Monitors**: Full support for multiple monitors. All monitors share the same workspace.
- **Dwindle Tiling**: Efficient binary space partitioning. Just the one algorithm.

## Keybindings

| Keybinding | Action |
|------------|--------|
| `Alt + Arrows` | Move focus between windows |
| `Alt + Shift + Arrows` | Move windows to different tiles |
| `Alt + 1-9` | Switch to workspace 1-9 |
| `Alt + Shift + 1-9` | Move focused window to workspace 1-9 |
| `Alt + W` | Close current window |
| `Alt + T` | Toggle current window's tiling state |
| `Alt + F` | Toggle fullscreen |

## Status

**Beta**: Feel free to use it, it's nearly feature complete, but most likely buggy.

## Installation

There is no installer or binary distribution at the moment. 

Prequisites:

- Rust installed, with dependencies
- Windows OS
- Git installed

Follow these steps:

- `git clone` the repository
- `cd megatile`
- `cargo build --release`
- `./target/release/megatile.exe`

MegaTile will now be running in the background, you can turn it off by finding the orange square in the system tray, right clicking it, and selecting exit.

## Contributing

Contributions are welcome, with the caveat that the project is intentionally opinionated and minimal.

- Bug reports and issue descriptions with clear reproduction steps are very helpful.
- Pull requests should stay aligned with the project's direction.
- Features that add significant complexity or configurability may be declined if they conflict with the project’s philosophy.

Before working on a larger change, consider opening a discussion to talk about the idea.

## License

This project is **source‑available, non‑commercial**.

- The source is freely accessible and may be used and modified for non‑commercial purposes.
- Commercial use (including using MegaTile as part of a paid product or service) requires a separate commercial license from the author.

If you are interested in commercial use, please contact the author to discuss licensing terms.
