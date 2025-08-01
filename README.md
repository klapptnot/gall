# Gall 🚀

A simple GTK-based picker daemon.

## Overview

Gall is a picker manager that provides a clean GTK interface for quick actions. It runs as a daemon in the background and can be toggled on demand.

## Installation

```bash
# Build from source to ~/.cargo/bin/gall
cargo install --path .
```

## Usage

### Starting the Daemon

```bash
# Start with default configuration (paths)
gall start

# Start with custom styles and config
gall start --styles ./my-styles.css --config ./my-config.toml

# Keep app attached to the caller process (no fork)
gall start --keep-open
```

### Managing the Daemon

```bash
# Show/hide the app launcher
gall apps

# Reload configuration without restarting
gall reload

# Stop the daemon
gall stop
```

## Configuration

Create a configuration file (default: `~/.config/gall/config.toml`):

```toml
# Reload CSS file for every time window is visible
css_reload = false
# Apps with Terminal=true will launch `kitty exec ...[args]`
# These apps will be ignored if this is unset or empty
terminal = "kitty"

[[apps]]
name = "Hatsune Miku"
gend = "CV01 - 初音ミク"
desc = "It's Hatsune Miku, what do you expect?"
exec = "bash -c 'echo \"むかえにゆくよ！\"; sleep 1; echo \"信じてないんだよ、ね？\" >&2; exit 1'"
icon = "~/Downloads/Hatsune_Miku.png"

[[apps]]
name = "Firefox"
gend = "Web Browser"
desc = "Browse the web with Firefox"
exec = "firefox"
icon = "firefox"
```

### Configuration Fields

- `name` - Display name for the application
- `gend` - Generic name or category
- `desc` - Brief description of what the app does
- `exec` - Command to execute when launched
- `icon` - Path to the application icon

## Styling

Customize the appearance by providing a CSS file:

```bash
gall start --styles ./custom-theme.css
```

## License

This project is licensed under the MIT License.
