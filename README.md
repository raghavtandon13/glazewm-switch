# GlazeWM Switch

![](assets/demo.png)

A native Windows workspace switcher widget for [GlazeWM](https://github.com/glzr-io/glazewm) that integrates directly into your taskbar.
A lightweight alternative (~2MB RAM) that shows all workspaces and lets you click to switch.

## Showcase

![Demo](assets/demo.gif)

## Features

- **Taskbar Integration**: Seamlessly embeds into the Windows taskbar
- **Live Workspace Display**: Shows all workspaces with real-time updates
- **Visual Status Indicators**: Different colors for focused, displayed, and empty workspaces
- **Two Visual Styles**: Choose between "windows" (square background) or "classic" (bars below)
- **Click to Switch**: Click on any workspace to instantly switch to it
- **Theme Aware**: Automatically adapts to Windows light/dark theme
- **Right-click Menu**: Right click to quit
- **Configurable Position**: Position the widget anywhere on the taskbar via config file

## Prerequisites

- Windows 10/11
- [GlazeWM](https://github.com/glzr-io/glazewm) window manager running with IPC enabled
- Rust toolchain (for building from source)

## Installation

### From Source

```bash
git clone https://github.com/raghav/glazewm-switch.git
cd komoswitch
cargo build --release
```

The executable will be at `./target/release/komoswitch.exe`

## Configuration

Create a `komoswitch.toml` file next to the executable:

```toml
[position]
# X offset from left edge (use -1 for center)
x = 64
# Y offset from top edge
y = 0

# Visual style: "windows" (square bg) or "classic" (bars below)
style = "windows"
```

### Config Options

| Option       | Description                             | Default   |
| ------------ | --------------------------------------- | --------- |
| `position.x` | X offset from left edge (-1 for center) | 64        |
| `position.y` | Y offset from top edge                  | 0         |
| `style`      | Visual style: "windows" or "classic"    | "windows" |

### Style Examples

- **windows**: Square background with color difference for focused/active workspaces
- **classic**: Bars below workspace numbers

## Usage

1. Launch the application
2. The widget appears on your taskbar
3. Left-click any workspace to switch to it
4. Right-click to quit

## Building

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Run
cargo run
```

## Acknowledgments

- [GlazeWM](https://github.com/glzr-io/glazewm) - The tiling window manager this widget is designed for
- [winsafe](https://github.com/rodrigocfd/winsafe) - Safe Rust bindings for Windows API
