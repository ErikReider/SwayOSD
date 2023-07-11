datadir = get_option('datadir')

# LICENSE
install_data(
  '../LICENSE',
  install_dir: get_option('datadir') / 'licenses' / meson.project_name()
)
# Dbus conf
install_data(
  join_paths('dbus', 'org.erikreider.swayosd.conf'),
  install_dir: join_paths(datadir, 'dbus-1', 'system.d')
)
# Polkit rule
install_data(
  join_paths('polkit', 'rules', 'org.erikreider.swayosd.rules'),
  install_dir: join_paths(datadir, 'polkit-1', 'rules.d')
)
# Polkit policy
conf_data = configuration_data()
conf_data.set('bindir', join_paths(get_option('prefix'), get_option('bindir')))
configure_file(
  input: join_paths('polkit', 'actions', 'org.erikreider.swayosd.policy.in'),
  output: 'org.erikreider.swayosd.policy',
  configuration: conf_data,
  install: true,
  install_dir: join_paths(datadir, 'polkit-1', 'actions')
)
# Dbus service
configure_file(
  configuration: conf_data,
  input: join_paths('services', 'dbus', 'org.erikreider.swayosd.service.in'),
  output: '@BASENAME@',
  install_dir: datadir + '/dbus-1/services'
)

# Systemd service unit
systemd = dependency('systemd', required: false)
if systemd.found()
  systemd_service_install_dir = systemd.get_variable(pkgconfig :'systemduserunitdir')
else
  systemd_service_install_dir = join_paths(libdir, 'systemd', 'user')
endif

configure_file(
  configuration: conf_data,
  input: join_paths('services', 'systemd', 'swayosd-libinput-backend.service.in'),
  output: '@BASENAME@',
  install_dir: systemd_service_install_dir
)
