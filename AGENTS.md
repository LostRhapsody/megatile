# AGENTS.md - Megatile Development Guide

This document provides essential information for agentic coding assistants working on the Megatile project. Follow these guidelines to maintain consistency and quality.

## Build, Lint, and Test Commands

### Building
- **Full build**: `cargo build`
- **Release build**: `cargo build --release`
- **Check compilation**: `cargo check`

### Linting
- **Lint code**: `cargo clippy`
- **Fix lint issues**: `cargo clippy --fix`
- **Lint without dependencies**: `cargo clippy --no-deps`

### Formatting
- **Format code**: `cargo fmt`
- **Check formatting**: `cargo fmt --check`
- **Format specific package**: `cargo fmt -p megatile`

### Testing
- **Run all tests**: `cargo test`
- **Run specific test**: `cargo test test_name`
- **Run tests with output**: `cargo test -- --nocapture`
- **Run benchmarks**: `cargo bench`

> **Note**: Currently, the project uses manual testing only.

### Pre-commit Checklist
Before committing changes, always run:
```bash
cargo fmt --check
cargo clippy
cargo build
cargo test  # When tests are available
```

## Code Style Guidelines

### General Principles
- **Minimalism**: Keep code simple and focused. Avoid unnecessary abstractions.
- **Performance First**: This is a window manager - speed and low latency are critical.
- **Safety**: Use `unsafe` blocks only for Windows API calls. All other code should be safe Rust.
- **Error Handling**: Use `Result<T, String>` for fallible operations. Keep error messages descriptive but concise.
- **Code over Config**: Try to avoid introducing any sort of 'optional' features that would require new CLI flags or configs. Keep it singlular in focus.

### Naming Conventions
- **Functions/Methods**: `snake_case`
- **Variables**: `snake_case`
- **Constants**: `SCREAMING_SNAKE_CASE`
- **Types/Structs/Enums**: `PascalCase`
- **Modules**: `snake_case`
- **Traits**: `PascalCase`

### Code Structure
- **Imports**: Group in this order:
  1. Standard library (`std::*`)
  2. External crates (alphabetical)
  3. Local modules (alphabetical)
- **Struct Fields**: Public fields use `pub`, private when appropriate
- **Methods**: Group logically (constructors first, then main functionality, then utilities)
- **Unsafe Code**: Isolate Windows API calls in dedicated functions with clear unsafe blocks

### Formatting Rules
- **Indentation**: 4 spaces (follow rustfmt defaults)
- **Line Length**: Keep lines under 100 characters when possible
- **Braces**: Same line for functions/structs, new line for match arms and closures
- **Trailing Commas**: Always use in multi-line structures

### Examples

**Function Definition**:
```rust
pub fn some_function(param: Type) -> Result<(), String> {
    // Implementation
    Ok(())
}
```

**Struct Definition**:
```rust
#[derive(Debug, Clone)]
pub struct SomeStruct {
    pub field_name: FieldType,
    private_field: PrivateType,
}
```

**Error Handling**:
```rust
pub fn operation() -> Result<(), String> {
    match some_fallible_call() {
        Ok(result) => Ok(result),
        Err(e) => Err(format!("Operation failed: {}", e)),
    }
}
```

**Unsafe Windows API Usage**:
```rust
unsafe {
    // Single Windows API call per unsafe block when possible
    SomeWindowsApiCall(param);
}
```

**Thread-Safe State Management**:
```rust
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub struct Manager {
    state: Arc<Mutex<State>>,
}
```

### Import Organization
```rust
// Standard library imports
use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

// External crate imports (alphabetical)
use tray_icon::TrayIcon;
use windows::Win32::Foundation::HWND;

// Local module imports (alphabetical)
mod hotkeys;
mod tiling;
mod workspace;

use hotkeys::HotkeyManager;
use workspace::Workspace;
```

## Architecture Patterns

### Module Organization
- **windows_lib.rs**: Windows API abstractions and window management utilities
- **workspace.rs**: Core data structures (Window, Workspace, Monitor)
- **workspace_manager.rs**: High-level workspace operations and state management
- **tiling.rs**: Tiling algorithms and layout calculations
- **hotkeys.rs**: Hotkey registration and action mapping
- **tray.rs**: System tray integration
- **main.rs**: Application entry point and event loop

### State Management
- Use `Arc<Mutex<T>>` for shared mutable state across threads
- Prefer immutable borrows when possible
- Keep critical sections as small as possible

### Windows API Integration
- Wrap all Windows API calls in `unsafe` blocks
- Create safe abstractions over unsafe operations
- Document the safety invariants for each unsafe block
- Use the `windows` crate types consistently

### Event Handling
- Use a centralized event queue with `OnceLock<Mutex<VecDeque<Event>>>`
- Process events in batches to avoid race conditions
- Handle cleanup on application exit

## Development Workflow

### Implementation Steps
1. Read the relevant STEP_N.md file carefully
2. Implement the specified functionality
3. Run `cargo clippy` and `cargo fmt` to ensure code quality
4. Run `cargo build` to verify compilation
5. Test manually according to step instructions
6. Update AGENTS.md progress tracking
7. Commit with clear, focused commit messages

### Code Review Checklist
- [ ] Code compiles without warnings
- [ ] `cargo clippy` passes
- [ ] `cargo fmt --check` passes
- [ ] Error handling is appropriate
- [ ] No unnecessary unsafe code
- [ ] Naming follows conventions
- [ ] No commented-out code
- [ ] Logic matches specification in STEP_N.md

### Common Patterns to Avoid
- **Over-engineering**: Keep solutions simple and direct
- **Premature optimization**: Profile before optimizing
- **Feature creep**: Stick to the minimal feature set defined in PLAN.md
- **Platform assumptions**: Test on Windows 10/11 as specified
- **Memory leaks**: Ensure proper cleanup in destructors and exit handlers

## Testing Strategy

### Current State
- Manual testing only (no automated tests yet)
- Each STEP_N.md includes specific testing instructions
- Focus on functional verification rather than unit tests

### Future Testing (When Implemented)
- Unit tests for pure functions (tiling algorithms, data structures)
- Integration tests for Windows API interactions
- Performance benchmarks for critical paths
- Fuzz testing for input validation

### Manual Testing Guidelines
- Test on multiple monitor setups when possible
- Verify window state persistence across workspace switches
- Test edge cases (minimized windows, dialogs, etc.)
- Use Task Manager to verify no resource leaks
- Test hotkey responsiveness under load

## Tooling and Dependencies

### Required Tools
- **Rust**: Latest stable (edition 2024)
- **Windows SDK**: For Windows API access
- **Git**: For version control

### Key Dependencies
- **windows**: Windows API bindings (core dependency)
- **tray-icon**: System tray integration
- **winit**: Event loop handling
- **ctrlc**: Graceful shutdown handling

### Dependency Management
- Keep dependencies minimal
- Pin versions for stability
- Review dependency updates for security and compatibility

## Troubleshooting

### Common Issues
- **Borrow checker errors**: Review Arc<Mutex<>> usage patterns
- **Windows API failures**: Check unsafe blocks and parameter types
- **Hotkey conflicts**: Test with different key combinations
- **Window filtering issues**: Adjust logic in `is_normal_window`
- **Performance problems**: Profile with Windows Performance Toolkit

### Debug Tips
- Use `println!` for temporary debugging (remove before commit)
- Check Windows Event Viewer for system-level issues
- Use `cargo build --verbose` for detailed compilation info
- Test on clean Windows installs for compatibility

## Project Constraints

- **Target Platform**: Windows 10/11 only
- **Architecture**: 64-bit only
- **Dependencies**: Minimal external crates
- **Performance**: Sub-millisecond response times required
- **Memory**: Minimal memory footprint
- **Stability**: No crashes or undefined behavior

## Communication

- **Issues**: Report bugs in GitHub issues
- **Documentation**: Update README.md and PLAN.md for architecture changes
- **Progress**: Update AGENT.md with completed steps
- **Decisions**: Document significant design decisions in commit messages

---

**Version**: 1.0
**Last Updated**: Agent-generated
**Project Phase**: Beta, feature complete but still bug squashing or adding QoL improvements
