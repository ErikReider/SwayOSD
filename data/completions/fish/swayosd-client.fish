set -l c complete -c swayosd-client

$c --no-files

$c -l config  -d "Use a custom config file instead of looking for one" --require-parameter --force-files
$c -l monitor -d "Which monitor to display osd on (e.g., HDMI-A-1, DP-1)" --require-parameter --no-files --arguments "({ { command -qv swaymsg && swaymsg -t get_outputs } || { command -qv hyprctl && hyprctl monitors -j } } | jq --raw-output .[].name)"

$c -l caps-lock       -d "Shows capslock osd. Note: Doesn't toggle CapsLock, just displays the status"
$c -l caps-lock-led   -d "Shows capslock osd. Uses LED class NAME. Note: Doesn't toggle CapsLock, just displays the status (/sys/class/leds/NAME)"     --require-parameter --no-files --arguments "(find /sys/class/leds -mindepth 1 -maxdepth 1 -printf '%f\n')"
$c -l num-lock        -d "Shows numlock osd. Note: Doesn't toggle NumLock, just displays the status"
$c -l num-lock-led    -d "Shows numlock osd. Uses LED class NAME. Note: Doesn't toggle NumLock, just displays the status (/sys/class/leds/NAME)"       --require-parameter --no-files --arguments "(find /sys/class/leds -mindepth 1 -maxdepth 1 -printf '%f\n')"
$c -l scroll-lock     -d "Shows scrolllock osd. Note: Doesn't toggle ScrollLock, just displays the status"
$c -l scroll-lock-led -d "Shows scrolllock osd. Uses LED class NAME. Note: Doesn't toggle ScrollLock, just displays the status (/sys/class/leds/NAME)" --require-parameter --no-files --arguments "(find /sys/class/leds -mindepth 1 -maxdepth 1 -printf '%f\n')"

$c -l output-volume -d "Shows volume osd and raises, loweres or mutes default sink volume"   --require-parameter --no-files --arguments "raise lower mute-toggle"
$c -l input-volume  -d "Shows volume osd and raises, loweres or mutes default source volume" --require-parameter --no-files --arguments "raise lower mute-toggle"
$c -l max-volume    -d "Sets the maximum volume"

$c -l device         -d "For which device to increase/decrease audio/brightness. Can be wildcard for brightness." --require-parameter --no-files --arguments "({ pactl list short sinks && pactl list short sources } | cut -f2; brightnessctl --list --class backlight --machine-readable | cut -d ',' -f1)"
$c -l brightness     -d "Shows brightness osd and raises or loweres all available sources of brightness device"   --require-parameter --no-files --arguments "raise lower"
$c -l min-brightness -d "Sets the minimum Brightness"

$c -l playerctl -d "Shows Playerctl osd and runs the playerctl command" --require-parameter --no-files --arguments "play-pause play pause stop next prev shuffle"

$c -l player -d "For which player to run the playerctl commands" --require-parameter --no-files --arguments "auto all shift unshift"
$c -l player -d "For which player to run the playerctl commands" --require-parameter --no-files --arguments "(playerctl -l)"

$c -l custom-message -d "Message to display"

$c -l custom-icon -d "Icon to display when using custom-message/custom-progress. Icon name is from Freedesktop specification"
$c -l custom-icon --exclusive --arguments "(__fish_complete_freedesktop_icons)"

$c -l custom-progress           -d "Progress to display (0.0 <-> 1.0)"
$c -l custom-segmented-progress -d "Segmented progress to display (value:num-segments). Ex: 2:4"
$c -l custom-progress-text      -d "Text to display when using custom-progress or custom-segmented-progress"

$c -s h -l help    -d "Print help"
$c -s V -l version -d "Print version"
