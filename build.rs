use std::process::Command;

fn main() {
	let output = Command::new("sh")
		.args(&[
			"-c",
			"cd data && glib-compile-resources swayosd.gresource.xml",
		])
		.status()
		.expect("failed to execute process");
	assert!(output.success());
}
