# SwayOSD

A OSD window for common actions like volume and capslock.

This is my first time coding in Rust so fixes and improvements are appreciated :)

## Features:

- Input and output volume change indicator
- Input and output mute change indicator
- Customizable maximum Volume
- Capslock change (Note: doesn't change the caps lock state)
- Brightness change indicator

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
# Sink volume raise
bindsym XF86AudioRaiseVolume exec swayosd --output-volume raise
# Sink volume lower
bindsym XF86AudioLowerVolume exec  swayosd --output-volume lower
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

## Install

Available on the AUR thanks to @jgmdev!

- [swayosd-git](https://aur.archlinux.org/packages/swayosd-git)

## Images

![image](https://user-images.githubusercontent.com/35975961/200685357-fb9697ae-a32d-4c60-a2ae-7791e70097b9.png)

![image](https://user-images.githubusercontent.com/35975961/200685469-96c3398f-0169-4d13-8df0-90951e30ff33.png)
