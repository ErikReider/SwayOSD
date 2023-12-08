use std::{env, process::Command};

fn main() {
	let output = Command::new("glib-compile-resources")
		.args(&["./data/swayosd.gresource.xml", "--sourcedir=./data"])
		.arg(&format!(
			"--target={}/swayosd.gresource",
			env::var("OUT_DIR").unwrap()
		))
		.status()
		.expect("failed to execute process");
	assert!(output.success());
}
