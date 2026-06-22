cargo_opt = ['--manifest-path', meson.project_source_root() / 'Cargo.toml']

if get_option('buildtype') == 'release'
  cargo_opt += ['--release']
  rust_target = 'release'
else
  rust_target = 'debug'
endif

binaries = ['swayosd-server', 'swayosd-client', 'swayosd-libinput-backend']
binaries_path = []
foreach prog : binaries
  binaries_path += meson.project_source_root() / 'target' / rust_target / prog
endforeach

cargo_bin = find_program('cargo')
assert(cargo_bin.found())

custom_target(
  'Cargo Build',
  build_by_default: true,
  build_always_stale: true,
  output: binaries,
  console: true,
  install: true,
  install_dir: join_paths(get_option('prefix'), get_option('bindir')),
  command: [
    cargo_bin,
    'build',
    cargo_opt,
    '&&',
    'cp',
    '-f', binaries_path,
    '@OUTDIR@',
  ],
)
