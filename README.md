# Carnet
## A secure, extendable and lightweight clipboard manager written in Rust.

### Features:
- Fully sandboxed process: you copy data from all around the internet, emails, documents, all the time. To prevent this arbitrary data from performing exploits on your system Carnet runs in a strong sandbox where it cannot see your files, cannot access the network, in fact it only has access to the exact permissions required for it to function. _We use the battle tested Bubblewrap for sandboxing._
- Secure handling of images: Carnet uses Glycin to decode images.
- Lightweight: Carnet has **no dependencies*** which makes it super lightweight and removes the concern of supply chain attacks.
- Minimal look: Carnet lives in your terminal as a **TUI**. It is entierly responsive and will always fit your terminal size.
- Ricing ready: using the config file you can make Carnet your own, down to the characters that make the borders in the UI!
- Tools: Carnet let's you create custom tools that you can run on the fly on your clipboard content. For example, you can encode/decode in base64, apply a black and white filter to images, or anything else you can imagine. Each tool is just a terminal command in the config that you can customize to be anything!
- Clipboard history
- Fuzzy search

## Getting started

You can install from the source by running: 
~~~
git clone ...
cd Carnet/apps/carnet/
cargo install --path .
~~~
This will put all the Carnet executables inside of your ~/.cargo/bin

First, you will need to setup carnet-watch to run in the background. To do so, simply add ~/.cargo/bin/carnet-watch to your exec-once on Hyprland or similar on other windows managers.

Then, to open the TUI you simply call ~/.cargo/bin/carnet-sandbox.

Here are recomended keybinds and window rules for Hyprland and Niri:

## Tools
