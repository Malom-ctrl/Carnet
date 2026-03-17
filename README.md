# Carnet
## A secure, lightweight and extendable clipboard manager written in Rust.

### Features:
- **Sandboxed process**: You copy data from all around the internet, emails and documents, all the time. To prevent this untrusted data from performing exploits on your system, Carnet runs in a strong sandbox where it can't see your files and can't access the network. In fact it only has access to the exact permissions required for it to function. _Carnet uses the battle-tested Bubblewrap for sandboxing._
- **Secure handling of images**: Carnet uses **Glycin** to decode images. This means it only sees the raw data after it has been processed by a specialized, sandboxed image decoder.
- **Lightweight**: Carnet has **no external crate dependencies*** which makes it super lightweight and removes the concern of supply chain attacks. It interacts directly with system libraries via FFI and uses standard utilities for IPC.
- **Terminal UI**: Carnet lives in your terminal as a **TUI**. It is responsive, which means it will resize with your terminal. It supports image rendering using the **Kitty graphics protocol**.
- **Ricing ready**: Using the config file you can make Carnet your own, from the colors down to the style of the borders!
- **Tools**: Carnet lets you create custom tools that you can run on the fly on your clipboard content. For example, you can encode/decode in base64, apply a black and white filter to images, or anything else you can imagine. Each tool is just a terminal command in the config that you can customize to be anything!
- **Sensitive mode**: Carnet supports masking sensitive data in it's UI, such as passwords and keys.
- **Clipboard history**
- **Fuzzy search**

_* No external crates used for core logic, only `libc` and a small internal TUI library. Still depends on system libraries like `glib`, `glycin`, and utilities like `wl-clipboard` and `bwrap`._

## Requirements

Carnet relies on several system utilities and libraries to maintain its security model and functionality:

- **a Wayland Window Manager with support for the wlroots data-control protocol**: Such as Hyprland, Niri or Sway.
- **bubblewrap**: For process sandboxing (`bwrap`).
- **wl-clipboard**: For Wayland clipboard interaction (`wl-copy`, `wl-paste`).
- **libglycin**: For secure, sandboxed image decoding (usually `libglycin-2` or `glycin-2`).

### Optional but Recommended:
- **Kitty-compatible terminal**: Required to view image previews (e.g., Kitty, WezTerm, or any terminal supporting the Kitty graphics protocol). Without it, all image related feature will still function but you won't be able to see the images in the TUI.
- **jq**: For the default JSON Pretty Print tool.
- **ImageMagick**: If you want to create advanced image-processing tools.

### Installation
You can install from the source by running: 
~~~bash
git clone https://github.com/Malom-ctrl/Carnet.git
cd Carnet/
cargo install --path apps/carnet/
~~~
This will put all the Carnet executables inside of your `~/.cargo/bin`.

### Setup

**IMPORTANT**: The paths below are assuming you installed from source using `cargo install`. If you downloaded the binaries directly just change the paths to where you've placed the executables instead.

First, you will need to setup `carnet-watch` to run in the background. 

On **Hyprland**, add to your `hyprland.conf`:
~~~hyprlang
exec-once = ~/.cargo/bin/carnet-watch
~~~

On **Niri**, add to your `config.kdl`:
~~~kdl
spawn-at-startup "~/.cargo/bin/carnet-watch"
~~~

Or similar on other window managers.

Then, to open the TUI you simply call `~/.cargo/bin/carnet-sandbox`.

### Recommended Keybinds and Window Rules

**Hyprland:**
~~~hyprlang
# Keybind to open the TUI
bind = SUPER, V, exec, kitty --class carnet-tui --override close_on_child_death=yes -e carnet-sandbox

# Window rule to make it float and look nice
windowrulev2 = float, initialclass:carnet-tui
windowrulev2 = size 800 1000, initialclass:carnet-tui # You can ajust to any size here
windowrulev2 = center, initialclass:carnet-tui
windowrulev2 = noborder, initialclass:carnet-tui
~~~

**Niri:**
~~~kdl
# Keybind to open the TUI
binds {
    Mod+V { spawn "kitty" "--class" "carnet-tui" "--override" "close_on_child_death=yes" "-e" "carnet-sandbox"; }
}

# Window rule to make it float and look nice
window-rule {
    match { app-id "carnet-tui"; }
    open-floating true
    focus-ring { off }
    border { off }
}
~~~

## Tools

Now the best part: the tools! You define them in your config file (`~/.config/carnet/config`). Each tool takes the clipboard content via `stdin` and if it outputs anything to `stdout`, it will be copied back to your clipboard.

Format: `TOOL_NAME = Display Name | context | preview_flag | command`

Context can be `text`, `image`, or `both`.
Preview flag can be `preview` or `no-preview`.

**Example Tools**
~~~ini
TOOL_UPPER = Upper Case | text | preview | tr '[:lower:]' '[:upper:]'
TOOL_LOWER = Lower Case | text | preview | tr '[:upper:]' '[:lower:]'
TOOL_B64_ENC = Base64 Encode | text | preview | base64
TOOL_JSON_PP = JSON Pretty Print | text | preview | jq .
TOOL_WC = Word Count | text | preview | wc -w
TOOL_MAGICK = Resize (50%) | image | no-preview | magick - -resize 50% -
~~~

## Configuration

Carnet looks for a configuration file at `~/.config/carnet/config`. It is automatically generated with default values and comments to help you use it.

## Architecture

Carnet uses a multi-process architecture to ensure security:
1. Less than 200 lines of safe Rust are running outside the sandbox to listen to clipboard events and setup the required permissions for the sandbox.
2. Everything else happens inside a secure **Bubblewrap** sandbox that has no network access, no file access* and no privileges.

**Please contact me or open an issue if you think there is a way to improve the way Carnet works. The project is new and is very much open to ideas.**

_*Some things such as libraries are mounted in read-only mode._
