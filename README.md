# Megatile

An opinionated tiling manager for windows.

## Overview

Megatile is a minimalist tiling manager that does the bare minimum to be effective. It has a standardized set of keybinds, a very simple status bar, and a single tiling algorithm.

Megatile will **not** suite everyone. It was designed to suite my needs exactly. There is no config file, to keep Megatile as simple as possible.

## Megatile Features and Goals

Megatile aims to be simple, fast, and effective.

- **Fast**: Rust with bindings for the Window's API via `windows-rs`. Minimal dependencies and responsibilities.
- **Simple**: We manage windows, workspaces, and keybinds to control them, nothing else.
- **Effective**: Manage 1-9 workspaces. Near instant response time. No animations, no lag.
- **System Tray**: Runs in the system tray, right click the icon to exit.
- **Filtering**: Doing our best to filter windows we don't want to tile.
- **Mutliple Monitors**: Full support for multiple monitors. All monitors share the same workspace.
- **Dwindle Tiling**: Efficient binary space partitioning. Just the one algorithm.
- **Status Bar**: Incredibly simply status bar. Shows workspaces 1-5 by default, the active workspace, and the date and time (mm:hh dd/mm). Will also display workspaces 6-9 if there are any windows in them.

## Keybindings

| Keybinding | Action |
|------------|--------|
| `Alt + Arrows` | Move focus between windows |
| `Alt + Shift + Arrows` | Swap window positions |
| `Alt + Ctrl + Arrows` | Move windows between monitors  |
| `Alt + 1-9` | Switch to workspace 1-9 |
| `Alt + Shift + 1-9` | Move focused window to workspace 1-9 |
| `Alt + W` | Close focused window |
| `Alt + T` | Toggle focused window's tiling state |
| `Alt + F` | Toggle focused window to fullscreen |
| `Alt + B` | Toggle the status bar |
| `Alt + J` | Flip current region |
| `Alt +  +/-` | Resize horizontally |
| `Alt + Shift +  +/-` | Resize vertically |

## Status

**0.2.0 - Beta Release**: Feel free to use it, it's nearly feature complete, but most likely buggy.

## Planned

These features are planned, but have no timeline, as they are not a priority.

- **Website**: A Megatile website with documentation, distribution service (install.sh).
- **Keybind file**: Allow keybinds to be re-bound to new ones, in case they conflict with existing user set ups.
    - The default keybinds are, in my opinion, the best we can do on Windows.
    - For now, you could probably use Autohotkey to re-map them, though I've never tried.

## Suggestions

I'd recommend using a launcher like Raycast to enhance your experience. A fast launcher paired with speedy tiling makes for a very good desktop experience.

If you'd like more customizabilty, check out [GlazeWM](https://github.com/glzr-io/glazewm), another tiling manager for Windows that is very i3-like, with a full configuration language and status bar called [Zebar](https://github.com/glzr-io/zebar) that can be styled.

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

Megatile will now be running in the background, you can turn it off by finding the orange square in the system tray, right clicking it, and selecting exit.

## Contributing

Contributions are welcome, with the caveat that the project is intentionally opinionated and minimal.

- Bug reports and issue descriptions with clear reproduction steps are very helpful.
- Pull requests should stay aligned with the project's direction.
- Features that add significant complexity or configurability may be declined if they conflict with the project’s philosophy.

Before working on a larger change, consider opening a discussion to talk about the idea.

## License

This project is **source‑available, non‑commercial**.

- The source is freely accessible and may be used and modified for non‑commercial purposes.
- Commercial use (including using Megatile as part of a paid product or service) requires a separate commercial license from the author.

If you are interested in commercial use, please contact the author to discuss licensing terms.

## Contact

visit [evanrobertson.dev](https://evanrobertson.dev) or email me at [evan.robertson77@gmail.com](mailto:evan.robertson77@gmail.com) if you have questions.