#!/bin/sh

set -eu

create_node() {
  path="$1"
  type="$2"
  major="$3"
  minor="$4"
  mode="$5"

  if [ ! -e "$path" ]; then
    dir=$(dirname "$path")
    mkdir -p "$dir"
    mknod "$path" "$type" "$major" "$minor"
    chmod "$mode" "$path"
  fi
}

create_char_node() {
  create_node "$1" c "$2" "$3" "$4"
}

create_block_node() {
  create_node "$1" b "$2" "$3" "$4"
}

if [ ! -e /dev/fb0 ]; then
  # Provide basic framebuffer and input nodes for Xorg.
  create_char_node /dev/fb0          29   0  666
  # create_char_node /dev/input/mice   13  63  666
  create_char_node /dev/input/mouse0 13  32  666
  create_char_node /dev/input/event0 13  64  640
  create_char_node /dev/input/event1 13  65  640
fi

# if [ -d /Desktop ]; then
#   chmod a+rw /Desktop/*.desktop 2>/dev/null || true
# fi

# if [ ! -e /bin/Thunar ] && [ -e /bin/thunar ]; then
#   ln -sf /bin/thunar /bin/Thunar
# fi

# Relax permissions for desktop launchers when present
chmod a+rw /Desktop/*.desktop 2>/dev/null || true

# Select a writable directory for runtime state and PID files
RUNTIME_DIR=/run
if ! (mkdir -p "$RUNTIME_DIR" >/dev/null 2>&1 && : > "$RUNTIME_DIR/.xfce-write-test" 2>/dev/null); then
  RUNTIME_DIR=/tmp/run
  mkdir -p "$RUNTIME_DIR"
fi
rm -f "$RUNTIME_DIR/.xfce-write-test"
chmod 700 "$RUNTIME_DIR" 2>/dev/null || true
export XDG_RUNTIME_DIR="$RUNTIME_DIR"

# Step 1: run dbus
export NO_AT_BRIDGE=1
mkdir -p /run/dbus
chmod 755 /run/dbus
eval "$(/usr/bin/dbus-launch --sh-syntax)"

if command -v dconf-service >/dev/null 2>&1; then
  dconf-service > ~/dconf.log 2>&1 & echo $! > "$RUNTIME_DIR/dconf-service.pid" &
fi

# Step 2: run Xorg
Xorg :0 -modulepath /usr/lib/xorg/modules -config /usr/share/X11/xorg.conf.d/10-fbdev.conf -logverbose 6 -logfile /var/xorg_debug.log -novtswitch -keeptty -keyboard keyboard -pointer mouse0 -xkbdir /usr/share/X11/xkb & echo $! > "$RUNTIME_DIR/xorg.pid" &

# Step 3: run xfconfd
export DISPLAY=:0
export GDK_BACKEND=x11
export GDK_CORE_DEVICE_EVENTS=1

export GTK_THEME="Adwaita"
export ICON_THEME="hicolor"
export XDG_DATA_DIRS="/usr/share:/usr/local/share:/run/current-system/sw/share"
export GSETTINGS_SCHEMA_DIR="/usr/share/glib-2.0/schemas"

export GDK_PIXBUF_MODULE_FILE=/usr/lib/gdk-pixbuf-2.0/2.10.0/loaders.cache
export GDK_PIXBUF_MODULEDIR=/usr/lib/gdk-pixbuf-2.0/2.10.0/loaders
export GIO_MODULE_DIR=/usr/lib/gio/modules
export GIO_EXTRA_MODULES=/usr/lib/gio/modules

#for debug
export G_MESSAGES_DEBUG=all

xfce4-session &

# Start tumbler (thumbnails used by settings dialog)
if command -v tumblerd >/dev/null 2>&1; then
  tumblerd -n > ~/tumblerd.log 2>&1 &
fi
xfsettingsd > ~/xfsettingsd.log 2>&1 & echo $! > "$RUNTIME_DIR/xfsettingsd.pid" &

#Step 4: run xfwm4
export XFWM4_LOG_FILE="/xfwm4.log"
xfwm4 --compositor=off & echo $! > "$RUNTIME_DIR/xfwm4.pid" &
#strace -o xfwm4_strace.log /usr/bin/xfwm4 --compositor=off -d &
#In asterinas /dev/null seems not working well. So needs to use "-d"

# Wait for EWMH props so xfdesktop doesn’t start “too early”
for i in $(seq 1 50); do
  if xprop -root _NET_NUMBER_OF_DESKTOPS >/dev/null 2>&1; then break; fi
  sleep 0.1
done

#Step 5: run xfdesktop
xfdesktop --enable-debug > ~/xfdesktop.log 2>&1 & echo $! > "$RUNTIME_DIR/xfdesktop.pid" &
#strace -o xfdesktop_strace.log /usr/bin/xfdesktop --enable-debug > ~/xfdesktop.log 2>&1 &

#Step 6: run xfce4-panel
xfce4-panel > ~/xfce4-panel.log 2>&1 & echo $! > "$RUNTIME_DIR/xfce4-panel.pid" &
