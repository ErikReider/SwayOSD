# SwayOSD

A OSD window for common actions like volume and capslock.

This is my first time coding in Rust so fixes and improvements are appreciated :)

## Features:

- LibInput listener for these keys:
  - Capslock
- Input and output volume change indicator
- Input and output mute change indicator
- Customizable maximum Volume
- Capslock change (Note: doesn't change the caps lock state)
- Brightness change indicator

## Install:

There's a new LibInput watcher binary shipped with SwayOSD (`swayosd-libinput-backend`)
which can automatically detect key presses, so no need for binding key combos.
The supported keys are listed above in [Features](#features)
<br>
<br>
_Note: The watcher is optional_

### Through Meson

```zsh
meson setup build
ninja -C build
meson install -C build
```

### AUR

Available on the AUR thanks to @jgmdev! (Don't open a issue here about AUR package)

- [swayosd-git](https://aur.archlinux.org/packages/swayosd-git)

## Usage:

```zsh
# OSD window
exec swayosd
```

or start with a max-volume set (default is 100)

```zsh
exec swayosd --max-volume 120
```

```zsh
# Sink volume raise optionally with --device
bindsym XF86AudioRaiseVolume exec swayosd --output-volume raise
# Sink volume lower optionally with --device
bindsym XF86AudioLowerVolume exec  swayosd --output-volume lower --device alsa_output.pci-0000_11_00.4.analog-stereo.monitor
# Sink volume toggle mute
bindsym XF86AudioMute exec swayosd --output-volume mute-toggle
# Source volume toggle mute
bindsym XF86AudioMicMute exec swayosd --input-volume mute-toggle

# Capslock
bindsym --release Caps_Lock exec swayosd --caps-lock

# Capslock but specific LED name (/sys/class/leds/)
bindsym --release Caps_Lock exec swayosd --caps-lock-led input19::capslock

# Brightness raise
bindsym XF86MonBrightnessUp exec swayosd --brightness raise
# Brightness lower
bindsym XF86MonBrightnessDown exec swayosd --brightness lower
```

### Notes on using `--device`:

- It is for audio devices only.
- If it is omitted the default audio device is used.
- It only changes the target device for the currrent/next action that changes the volume.
- `--max-volume` is a global limit for all devices so `--device` has no effect on it.
- You can list your input audio devices using `pactl list short sources`, for outputs replace `sources` with `sinks`.

## Brightness Control

Some devices may not have permission to write `/sys/class/backlight/*/brightness`.

Workaround will be adding a rule inside `udev`:

1. Add `udev` rules:

`/etc/udev/rules.d/99-swayosd.rules`

```udevrules
ACTION=="add", SUBSYSTEM=="backlight", RUN+="/bin/chgrp video /sys/class/backlight/%k/brightness"
ACTION=="add", SUBSYSTEM=="backlight", RUN+="/bin/chmod g+w /sys/class/backlight/%k/brightness"
```

2. Add user to `video` group by running `sudo usermod -a -G video $USER`
3. Reboot system for udev rules to take effect

## Images

![image](https://user-images.githubusercontent.com/35975961/200685357-fb9697ae-a32d-4c60-a2ae-7791e70097b9.png)

![image](https://user-images.githubusercontent.com/35975961/200685469-96c3398f-0169-4d13-8df0-90951e30ff33.png)
