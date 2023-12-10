project('swayosd', 'rust',
          version: '0.1.0',
    meson_version: '>= 0.62.0',
  default_options: [ 'warning_level=2', 'werror=false', ],
)

config_path = join_paths(get_option('sysconfdir'), 'xdg', 'swayosd')

# glib-compile-resources Dependency
assert(find_program('glib-compile-resources').found())

# SCSS Dependency
sassc = find_program('sassc')
assert(sassc.found())

subdir('data')

subdir('src')
