pkgdatadir = join_paths(get_option('prefix'), get_option('datadir'), meson.project_name())

cargo_bin  = find_program('cargo')
cargo_opt  = [ '--manifest-path', meson.project_source_root() / 'Cargo.toml' ]
cargo_opt += [ '--target-dir', meson.project_build_root()  / 'src' ]
cargo_env  = [ 'CARGO_HOME=' + meson.project_build_root()  / 'cargo-home' ]

if get_option('buildtype') == 'release'
  cargo_opt += [ '--release' ]
  rust_target = 'release'
else
  rust_target = 'debug'
endif

foreach prog : ['swayosd-server', 'swayosd-client', 'swayosd-libinput-backend']
  custom_target(
    'cargo-build: @0@'.format(prog),
    build_by_default: true,
    build_always_stale: true,
    output: prog,
    console: true,
    install: true,
    install_dir: join_paths(get_option('prefix'), get_option('bindir')),
    command: [
      'env', cargo_env,
      cargo_bin, 'build', cargo_opt, '&&',
      'cp', '-f', 'src' / rust_target / prog, '@OUTPUT@',
    ]
  )
endforeach
