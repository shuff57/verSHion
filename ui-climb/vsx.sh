#!/bin/bash
# The climb build on its own large Xvfb screen at the desktop's 1.7 scale, never on the real Wayland
# desktop: there a capture opens a visible window and, when the screen is idle, never reaches its frame.
for try in 1 2 3 4; do
  xvfb-run -a -s "-screen 0 2600x1700x24" env -u WAYLAND_DISPLAY WINIT_X11_SCALE_FACTOR=1.7 /home/shuff57/Documents/GitHub/verSHion-climb/target/debug/vershion "$@" && exit 0
  sleep $((RANDOM % 3 + 1))
done
exit 1
