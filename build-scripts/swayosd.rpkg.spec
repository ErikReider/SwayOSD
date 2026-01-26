# vim: syntax=spec
%global alt_pkg_name swayosd

Name:       %{alt_pkg_name}
Version:    0.3.0
Release:    1%{?dist}
Summary:    A GTK based on screen display for keyboard shortcuts like caps-lock and volume
Provides:   %{alt_pkg_name} = %{version}-%{release}
License:    GPLv3
URL:        https://github.com/ErikReider/swayosd
VCS:        {{{ git_repo_vcs }}}
Source:     {{{ git_repo_pack }}}

# TODO: Use fedora RPM rust packages
BuildRequires:  meson >= 1.5.1
BuildRequires:  rust
BuildRequires:  cargo
BuildRequires:  pkgconfig(gtk4)
BuildRequires:  pkgconfig(gtk4-layer-shell-0)
BuildRequires:  pkgconfig(glib-2.0) >= 2.50
BuildRequires:  pkgconfig(gobject-introspection-1.0) >= 1.68
BuildRequires:  pkgconfig(gee-0.8) >= 0.20
BuildRequires:  pkgconfig(libpulse)
BuildRequires:  pkgconfig(libudev)
BuildRequires:  pkgconfig(libevdev)
BuildRequires:  pkgconfig(libinput)
BuildRequires:  pkgconfig(dbus-1)
BuildRequires:  systemd-devel
BuildRequires:  systemd
BuildRequires:  sassc

Requires:       dbus
%{?systemd_requires}

%description
A OSD window for common actions like volume and capslock.

%prep
{{{ git_repo_setup_macro }}}

%build
%meson
%meson_build

%install
%meson_install

%files
%doc README.md
%{_bindir}/swayosd-client
%{_bindir}/swayosd-server
%{_bindir}/swayosd-libinput-backend
%license LICENSE
%config(noreplace) %{_sysconfdir}/xdg/swayosd/backend.toml
%config(noreplace) %{_sysconfdir}/xdg/swayosd/config.toml
%config(noreplace) %{_sysconfdir}/xdg/swayosd/style.css
%{_unitdir}/swayosd-libinput-backend.service
%{_libdir}/udev/rules.d/99-swayosd.rules
%{_datadir}/dbus-1/system-services/org.erikreider.swayosd.service
%{_datadir}/dbus-1/system.d/org.erikreider.swayosd.conf
%{_datadir}/polkit-1/actions/org.erikreider.swayosd.policy
%{_datadir}/polkit-1/rules.d/org.erikreider.swayosd.rules

# Changelog will be empty until you make first annotated Git tag.
%changelog
{{{ git_repo_changelog }}}
