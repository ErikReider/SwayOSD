pkgdatadir = join_paths(get_option('prefix'), get_option('datadir'), meson.project_name())

cargo_bin  = find_program('cargo')
assert(cargo_bin.found())
cargo_opt  = [ '--manifest-path', meson.project_source_root() / 'Cargo.toml' ]
cargo_opt += [ '--target-dir', meson.project_build_root()  / 'src' ]
cargo_env  = [ 'CARGO_HOME=' + meson.project_build_root()  / 'cargo-home' ]

if get_option('buildtype') == 'release'
  cargo_opt += [ '--release' ]
  rust_target = 'release'
else
  rust_target = 'debug'
endif

binaries = [
  'swayosd-server',
  'swayosd-client',
  'swayosd-libinput-backend'
]
binaries_path = []
foreach prog : binaries
  binaries_path += '@OUTDIR@/@0@/@1@'.format(rust_target, prog)
endforeach

custom_target(
  'Cargo Build',
  build_by_default: true,
  build_always_stale: true,
  output: binaries,
  console: true,
  install: true,
  install_dir: join_paths(get_option('prefix'), get_option('bindir')),
  command: [
    'env', cargo_env,
    cargo_bin, 'build', cargo_opt, '&&',
    'cp', '-f', binaries_path, '@OUTDIR@'
  ]
)
