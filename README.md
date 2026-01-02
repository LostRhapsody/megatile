# MegaTile

A blazingly fast, lightweight, and strongly opinionated tiling window manager for Windows.

## Overview

MegaTile is a minimalist tiling window manager built with Rust that uses the Windows API directly for maximum performance and minimal dependencies. It's designed for power users who want a streamlined window management experience without the bloat of full-featured alternatives.

## Goals

- **Speed**: Instant response times with minimal overhead
- **Lightweight**: No unnecessary dependencies, just Windows API + Rust
- **Opinionated**: Fixed keybindings and behaviors, no configuration needed
- **Simplicity**: Focus on core tiling functionality without feature creep

## Technologies

- **Language**: Rust
- **Window Management**: Windows API (winapi/windows-rs)
- **Tiling Algorithm**: Dwindle (binary space partitioning)
- **System Integration**: System tray application
- **Hotkeys**: Windows key (Super) as primary modifier

## Keybindings

| Keybinding | Action |
|------------|--------|
| `Win + Arrows` | Move focus between windows |
| `Win + Shift + Arrows` | Move windows to different tiles |
| `Win + 1-9` | Switch to workspace 1-9 |
| `Win + Shift + 1-9` | Move focused window to workspace 1-9 |
| `Win + W` | Close current window |
| `Win + T` | Toggle tiling algorithm |
| `Win + F` | Toggle fullscreen |

## Features

- **9 Workspaces**: Shared across all monitors
- **Dwindle Tiling**: Efficient binary space partitioning
- **Multi-Monitor**: Independent tiling on each monitor within same workspace
- **Window Filtering**: Only normal windows are tiled (excludes taskbar, dialogs, popups)
- **Workspace Isolation**: Windows from inactive workspaces are completely hidden (not visible in taskbar)
- **System Tray**: Easy access to exit and auto-start options

## Architecture

- **Window Monitor**: Tracks and filters all normal windows
- **Workspace Manager**: Manages 9 workspaces with per-monitor state
- **Tiling Engine**: Implements Dwindle algorithm for efficient space usage
- **Input Handler**: Registers and processes global hotkeys
- **Window Hider**: Shows/hides windows on workspace switches
- **System Tray**: Provides tray icon and menu

## Status

**Development Phase**: Implementation (STEP_7)

See [PLAN.md](PLAN.md) for the implementation roadmap.

## Getting Started

Once complete, MegaTile will be available as a standalone Windows executable. Run it to start the system tray icon, and it will automatically begin tiling windows.

## License

TBD
