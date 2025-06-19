# Gall 🚀

A simple GTK-based application selector and launcher daemon.

## Overview

Gall is a application launcher that provides a clean GTK interface for quickly finding and launching your favorite applications. It runs as a daemon in the background and can be toggled on demand.

## Installation

```bash
# Build from source to ~/.cargo/bin/gall
cargo install --path .
```

## Usage

### Starting the Daemon

```bash
# Start with default configuration
gall start

# Start with custom styles and config
gall start --styles ./my-styles.css --config ./my-config.toml

# Enable CSS hot reloading for development
gall start --reload-css

# Start and immediately show the launcher
gall start --open
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
css_reload = false

[[apps]]
name = "Hatsune Miku"
generic = "CV01 - 初音ミク"
description = "It's Hatsune Miku, what do you expect?"
exec = "bash -c 'sleep 1; echo \"信じてないんだよ、ね？\" >&2; echo \"むかえにゆくよ\"; exit 1'"
icon = "~/Downloads/Hatsune_Miku.png"

[[apps]]
name = "Terminal"
generic = "System Terminal"
description = "Launch your default terminal emulator"
exec = "gnome-terminal"
icon = "/usr/share/icons/hicolor/48x48/apps/terminal.png"

[[apps]]
name = "Firefox"
generic = "Web Browser"
description = "Browse the web with Firefox"
exec = "firefox"
icon = "firefox"

[[apps]]
name = "File Manager"
generic = "Files"
description = "Browse your files and folders"
exec = "nautilus"
icon = "file-manager"
```

### Configuration Fields

- `name` - Display name for the application
- `generic` - Generic name or category
- `description` - Brief description of what the app does
- `exec` - Command to execute when launched
- `icon` - Path to the application icon

## Styling

Customize the appearance by providing a CSS file:

```bash
gall start --styles ./custom-theme.css
```

<details>
<summary><b>Example CSS</b></summary>

```css
/* Catppuccin Mocha GTK Theme */
@define-color rosewater #f5e0dc;
@define-color flamingo #f2cdcd;
@define-color pink #f5c2e7;
@define-color mauve #cba6f7;
@define-color red #f38ba8;
@define-color maroon #eba0ac;
@define-color peach #fab387;
@define-color yellow #f9e2af;
@define-color green #a6e3a1;
@define-color teal #94e2d5;
@define-color sky #89dceb;
@define-color sapphire #74c7ec;
@define-color blue #89b4fa;
@define-color lavender #b4befe;
@define-color text #cdd6f4;
@define-color subtext1 #bac2de;
@define-color subtext0 #a6adc8;
@define-color overlay2 #9399b2;
@define-color overlay1 #7f849c;
@define-color overlay0 #6c7086;
@define-color surface2 #585b70;
@define-color surface1 #45475a;
@define-color surface0 #313244;
@define-color base #1e1e2e;
@define-color mantle #181825;
@define-color crust #11111b;

@define-color accent @mauve;

/* Window styling */
window {
    background-color: @base;
    border-radius: 34px;
    border: 2px solid @mauve;
}

window decoration {
    background-color: @base;
    border-radius: 10px 10px 0 0;
}

/* Entry widgets (text inputs) */
#search-box {
    border-radius: 8px 8px 26px 26px;
}
#search-input {
    background-color: @surface0;
    color: @text;
    border: 1px solid @surface1;
    border-radius: 22px 5px 5px 5px;
    padding: 8px;
    font-size: 16px;
    transition: all 200ms ease;
}

#search-input:focus {
    border-color: @mauve;
    box-shadow: inset 0 0 0 1px @mauve;
    background-color: @surface1;
}

#search-input:disabled {
    background-color: @mantle;
    color: @overlay0;
    border-color: @surface0;
}

/* Button styling */
#toggle-button {
    background-color: @surface0;
    color: @text;
    border: 1px solid @mauve;
    border-radius: 5px 22px 5px 5px;
    padding: 6px 12px;
    font-size: 12px;
    transition: all 200ms ease;
    min-height: 24px;
}

#toggle-button:hover {
    background-color: @surface1;
    border-color: @surface2;
    color: @lavender;
}

#toggle-button:active {
    background-color: @surface2;
}

#toggle-button:disabled {
    background-color: @mantle;
    color: @overlay0;
    border-color: @surface0;
}

#apps-scroll {
    border-radius: 8px 8px 26px 26px;
    padding-bottom: 20px;
}

/* ListBox styling */
#apps-list {
    background-color: transparent;
    border-radius: 8px 8px 26px 26px;
}

#app-row {
    background-color: transparent;
    color: @text;
    border-bottom: 1px solid alpha(@surface0, 0.5);
    border-radius: 8px;
    transition: all 150ms ease;
    min-height: 32px;
}

#app-row:hover {
    background-color: @surface0;
    color: @lavender;
}

#app-row:selected {
    background-color: @mauve;
}

#app-row:selected label {
    color: @crust;
    font-weight: 500;
}

#app-row:selected:hover {
    background-color: @sapphire;
}

/* Label styling */
#app-row label {
    color: @text;
}

#app-row label:disabled {
    color: @overlay0;
}

/* Scrollbar styling */
scrollbar {
    background-color: @mantle;
    border-radius: 8px;
    border: none;
}

scrollbar slider {
    background-color: @mauve;
    border-radius: 8px;
    border: 2px solid transparent;
    min-width: 12px;
    min-height: 12px;
}

scrollbar slider:hover {
    background-color: @surface2;
}

scrollbar slider:active {
    background-color: @overlay0;
}

scrollbar.horizontal slider {
    min-width: 12px;
    min-height: 12px;
}

scrollbar.vertical slider {
    min-width: 12px;
    min-height: 12px;
}

#error-reason {
    color: @mauve;
}
#error-label-stderr {
    color: @red;
}
#error-label-stdout {
    color: @green;
}

.error-copy-btn {
    background-color: @peach;
    color: @mantle;
}
.error-copy-btn:hover {
    background-color: @yellow;
}

#error-close-btn {
    background-color: @red;
    color: @mantle;
}
#error-close-btn:hover {
    background-color: @pink;
}

```

</details>

## License

This project is licensed under the MIT License.
