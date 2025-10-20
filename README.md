# SwayOSD

A OSD window for common actions like volume and capslock.

This is my first time coding in Rust so fixes and improvements are appreciated :)

## Features:

- LibInput listener Backend for these keys:
  - Caps Lock
  - Num Lock
  - Scroll Lock
- Input and output volume change indicator
- Input and output mute change indicator
- Customizable maximum Volume
- Capslock change (Note: doesn't change the caps lock state)
- Brightness change indicator

## Images

![image](https://user-images.githubusercontent.com/35975961/200685357-fb9697ae-a32d-4c60-a2ae-7791e70097b9.png)

![image](https://user-images.githubusercontent.com/35975961/200685469-96c3398f-0169-4d13-8df0-90951e30ff33.png)

## Install:

There's a new LibInput watcher binary shipped with SwayOSD (`swayosd-libinput-backend`)
which can automatically detect key presses, so no need for binding key combos.
The supported keys are listed above in [Features](#features)

### Through Meson

```zsh
# Please note that the command below might require `--prefix /usr` on some systems
meson setup build
ninja -C build
meson install -C build
```

### Arch Linux

- extra: [swayosd](https://archlinux.org/packages/extra/x86_64/swayosd/)
- AUR: [swayosd-git](https://aur.archlinux.org/packages/swayosd-git) (thanks to @jgmdev! Don't report AUR packaging issues here)

### Debian / Ubuntu

Starting with Debian trixie and Ubuntu Plucky swayosd is available via apt.

- [swayosd](https://tracker.debian.org/swayosd)

## Usage:

### SwayOSD LibInput Backend

Using Systemd: `sudo systemctl enable --now swayosd-libinput-backend.service`

Other users can run: `pkexec swayosd-libinput-backend`

### SwayOSD Frontend

#### Sway examples

##### Start Server

```zsh
# OSD server
exec swayosd-server
```

##### Add Client bindings

```zsh
# Sink volume raise optionally with --device
bindsym XF86AudioRaiseVolume exec swayosd-client --output-volume raise
# Sink volume lower optionally with --device
bindsym XF86AudioLowerVolume exec  swayosd-client --output-volume lower --device alsa_output.pci-0000_11_00.4.analog-stereo.monitor
# Sink volume toggle mute
bindsym XF86AudioMute exec swayosd-client --output-volume mute-toggle
# Source volume toggle mute
bindsym XF86AudioMicMute exec swayosd-client --input-volume mute-toggle

# Volume raise with custom value
bindsym XF86AudioRaiseVolume exec swayosd-client --output-volume 15
# Volume lower with custom value
bindsym XF86AudioLowerVolume exec swayosd-client --output-volume -15

# Volume raise with max value
bindsym XF86AudioRaiseVolume exec swayosd-client --output-volume raise --max-volume 120
# Volume lower with max value
bindsym XF86AudioLowerVolume exec swayosd-client --output-volume lower --max-volume 120

# Sink volume raise with custom value optionally with --device
bindsym XF86AudioRaiseVolume exec  swayosd-client --output-volume +10 --device alsa_output.pci-0000_11_00.4.analog-stereo.monitor
# Sink volume lower with custom value optionally with --device
bindsym XF86AudioLowerVolume exec  swayosd-client --output-volume -10 --device alsa_output.pci-0000_11_00.4.analog-stereo.monitor

# Capslock (If you don't want to use the backend)
bindsym --release Caps_Lock exec swayosd-client --caps-lock
# Capslock but specific LED name (/sys/class/leds/)
bindsym --release Caps_Lock exec swayosd-client --caps-lock-led input19::capslock

# Brightness raise
bindsym XF86MonBrightnessUp exec swayosd-client --brightness raise
# Brightness lower
bindsym XF86MonBrightnessDown exec swayosd-client --brightness lower

# Brightness raise with custom value('+' sign needed)
bindsym XF86MonBrightnessUp  exec swayosd-client --brightness +10
# Brightness lower with custom value('-' sign needed)
bindsym XF86MonBrightnessDown exec swayosd-client --brightness -10
```

### Notes on using `--device`:

- It is for audio devices only.
- If it is omitted the default audio device is used.
- It only changes the target device for the current action that changes the volume.
- You can list your input audio devices using `pactl list short sources`, for outputs replace `sources` with `sinks`.

### Notes on using `--monitor`:

- By default, without using --monitor the osd will be shown on all monitors
- On setups with multiple monitors, if you only want to show the osd on the focused monitor, you can do so with the help of window manager specific commands:
```sh
# Sway
swayosd-client --monitor "$(swaymsg -t get_outputs | jq -r '.[] | select(.focused == true).name')" --output-volume raise

# Hyprland
swayosd-client --monitor "$(hyprctl monitors -j | jq -r '.[] | select(.focused == true).name')" --output-volume raise
```

## Theming

Since SwayOSD uses GTK, its appearance can be changed. Initially scss is used, which GTK does not support, so we need to use plain css. 
The style conifg file is in `~/.config/swayosd/style.css` (it is not automatically generated). For reference you can check [this](https://github.com/ErikReider/SwayOSD/blob/main/data/style/style.scss) and [this](https://github.com/ErikReider/SwayOSD/issues/36).

## Brightness Control

Some devices may not have permission to write `/sys/class/backlight/*/brightness`.
So using the provided packaged `udev` rules + adding the user to `video` group
by running `sudo usermod -a -G video $USER`, everything should work as expected.
