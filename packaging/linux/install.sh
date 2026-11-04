#!/bin/sh
# Installs the desktop entry and icons for the current user so the window
# and launcher show Rivet's icon. Wayland compositors take the window icon
# from the .desktop file matching the app id, not from the window itself.
#
# usage: packaging/linux/install.sh [path/to/rivet]
set -eu
here=$(cd "$(dirname "$0")" && pwd)
bin=${1:-$(command -v rivet || true)}
if [ -z "$bin" ]; then
    echo "pass the rivet binary path, e.g. packaging/linux/install.sh target/release/rivet" >&2
    exit 1
fi
bin=$(cd "$(dirname "$bin")" && pwd)/$(basename "$bin")
data=${XDG_DATA_HOME:-$HOME/.local/share}

mkdir -p "$data/applications"
cp -r "$here/icons/." "$data/icons/"
sed "s|^Exec=.*|Exec=\"$bin\"|" "$here/dev.rivet.Rivet.desktop" > "$data/applications/dev.rivet.Rivet.desktop"

gtk-update-icon-cache -q "$data/icons/hicolor" 2>/dev/null || true
update-desktop-database -q "$data/applications" 2>/dev/null || true
echo "installed Rivet launcher for $bin"
