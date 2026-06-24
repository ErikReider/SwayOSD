set -l c complete -c swayosd-server

$c --no-files

$c      -l config     -d "Use a custom config file instead of looking for one" --require-parameter --force-files
$c -s s -l style      -d "Use a custom Stylesheet file instead of looking for one" --require-parameter --force-files
$c      -l top-margin -d "OSD margin from top edge (0.5 would be screen center). Default is 0.85"
$c -s h -l help       -d "Print help"
$c -s V -l version    -d "Print version"
