cargo_opt  = []

cargo_env = environment()
cargo_env.set('CARGO_MANIFEST_PATH', meson.project_source_root() / 'Cargo.toml')
cargo_env.set('CARGO_TARGET_DIR', meson.project_build_root() / 'src')
cargo_env.set('CARGO_HOME', meson.project_build_root() / 'cargo-home')
cargo_env.set('CARGO_PROFILE', 'debug')

meson.add_devenv(cargo_env)

if get_option('buildtype') == 'release'
  cargo_opt += [ '--release' ]
  rust_target = 'release'
else
  rust_target = 'debug'
endif

binaries = ['swayosd-server', 'swayosd-client', 'swayosd-libinput-backend']
binaries_path = []
foreach prog : binaries
  binaries_path += '@OUTDIR@/@0@/@1@'.format(rust_target, prog)
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
  env: cargo_env,
  command: [cargo_bin, 'build', cargo_opt, '&&', 'cp', '-f', binaries_path, '@OUTDIR@'],
)
