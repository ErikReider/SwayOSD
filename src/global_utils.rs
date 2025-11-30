pub fn segmented_progress_parser(ref_value: &str) -> Result<(u32, u32), String> {
	let (value, n_segments) = match ref_value.split_once(":") {
		Some(v) => v,
		None => {
			return Err(format!(
				"Value {} not valid for segmented_progress",
				ref_value
			));
		}
	};
	match (value.parse::<u32>(), n_segments.parse::<u32>()) {
		(Ok(value), Ok(max)) => Ok((value, max)),
		_ => Err(format!(
			"Value {} not valid for segmented_progress. Must contain positive integers",
			ref_value
		)),
	}
}

pub fn div_round_u32(a: u32, b: u32) -> u32 {
	(a + b / 2) / b
}
