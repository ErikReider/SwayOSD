use gtk::glib::clone;
use pulse::{
	callbacks::ListResult,
	context::{introspect, Context},
	mainloop::standard::{IterateResult, Mainloop},
	operation::{Operation, State},
	proplist::Proplist,
	volume::ChannelVolumes,
};

use std::cell::RefCell;
use std::ops::Deref;
use std::rc::Rc;

/// Whether we're operating on an output (sink) or input (source) device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
	Sink,
	Source,
}

/// Error types for PulseAudio operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum PulseError {
	#[error("PulseAudio connect error: {0}")]
	Connect(String),
	#[error("PulseAudio operation error: {0}")]
	Operation(String),
	#[error("PulseAudio get info error: {0}")]
	GetInfo(String),
}

impl From<pulse::error::PAErr> for PulseError {
	fn from(error: pulse::error::PAErr) -> Self {
		Self::Operation(
			error
				.to_string()
				.unwrap_or_else(|| "Unknown PA error".to_string()),
		)
	}
}

/// Minimal device info needed by SwayOSD.
#[derive(Debug, Clone)]
pub struct DeviceInfo {
	pub kind: DeviceKind,
	pub index: u32,
	pub volume: ChannelVolumes,
	pub mute: bool,
}

impl From<&introspect::SinkInfo<'_>> for DeviceInfo {
	fn from(info: &introspect::SinkInfo) -> Self {
		DeviceInfo {
			kind: DeviceKind::Sink,
			index: info.index,
			volume: info.volume,
			mute: info.mute,
		}
	}
}

impl From<&introspect::SourceInfo<'_>> for DeviceInfo {
	fn from(info: &introspect::SourceInfo) -> Self {
		DeviceInfo {
			kind: DeviceKind::Source,
			index: info.index,
			volume: info.volume,
			mute: info.mute,
		}
	}
}

// ---------------------------------------------------------------------------
// Callback helper: collects the first item from a PulseAudio list callback.
//
// libpulse-binding's SinkInfo/SourceInfo carry a lifetime parameter that
// makes a generic function impractical (HRTB bounds don't unify). A macro
// avoids the issue by expanding at each call site.
// ---------------------------------------------------------------------------

/// Fires a PulseAudio introspect query that yields `ListResult<&$info_ty>`,
/// collects the first item as a `DeviceInfo`, and returns it (or an error).
macro_rules! query_device {
	($self:expr, $kind:expr, $introspect_method:ident ( $($arg:expr),* ), $info_ty:ty) => {{
		let result: Rc<RefCell<Option<DeviceInfo>>> = Rc::new(RefCell::new(None));
		let op = $self.introspect.$introspect_method(
			$($arg,)*
			clone!(
				#[strong]
				result,
				move |list: ListResult<&$info_ty>| {
					if let ListResult::Item(item) = list {
						result.replace(Some(DeviceInfo::from(item)));
					}
				}
			),
		);
		$self.wait_for_operation(op)?;
		result
			.take()
			.ok_or_else(|| PulseError::GetInfo(format!("{:?} not found", $kind)))
	}};
}

// ---------------------------------------------------------------------------
// VolumeController: PulseAudio connection wrapper for both sinks and sources
// ---------------------------------------------------------------------------

pub struct VolumeController {
	mainloop: Rc<RefCell<Mainloop>>,
	context: Rc<RefCell<Context>>,
	introspect: introspect::Introspector,
}

impl VolumeController {
	pub fn create() -> Result<Self, PulseError> {
		let mut proplist = Proplist::new()
			.ok_or_else(|| PulseError::Connect("Failed to create proplist".into()))?;
		proplist
			.set_str(pulse::proplist::properties::APPLICATION_NAME, "SwayOSD")
			.map_err(|_| PulseError::Connect("Failed to set application name".into()))?;

		let mainloop = Mainloop::new()
			.ok_or_else(|| PulseError::Connect("Failed to create mainloop".into()))?;
		let mainloop = Rc::new(RefCell::new(mainloop));

		let context = Context::new_with_proplist(mainloop.borrow().deref(), "SwayOSD", &proplist)
			.ok_or_else(|| PulseError::Connect("Failed to create context".into()))?;
		let context = Rc::new(RefCell::new(context));

		// return Err(PulseError::Connect("Failed to connect context".into()));

		context
			.borrow_mut()
			.connect(None, pulse::context::FlagSet::NOFLAGS, None)
			.map_err(|_| PulseError::Connect("Failed to connect context".into()))?;

		loop {
			match mainloop.borrow_mut().iterate(true) {
				IterateResult::Err(e) => return Err(e.into()),
				IterateResult::Quit(_) => {
					return Err(PulseError::Connect("Mainloop quit unexpectedly".into()));
				}
				IterateResult::Success(_) => {}
			}
			match context.borrow().get_state() {
				pulse::context::State::Ready => break,
				pulse::context::State::Failed | pulse::context::State::Terminated => {
					return Err(PulseError::Connect(
						"Context state failed/terminated".into(),
					));
				}
				_ => {}
			}
		}

		let introspect = context.borrow_mut().introspect();
		Ok(Self {
			mainloop,
			context,
			introspect,
		})
	}

	fn wait_for_operation<G: ?Sized>(&self, op: Operation<G>) -> Result<(), PulseError> {
		loop {
			match self.mainloop.borrow_mut().iterate(true) {
				IterateResult::Err(e) => return Err(e.into()),
				IterateResult::Quit(_) => {
					return Err(PulseError::Operation("Mainloop quit unexpectedly".into()));
				}
				IterateResult::Success(_) => {}
			}
			match op.get_state() {
				State::Done => return Ok(()),
				State::Running => {}
				State::Cancelled => {
					return Err(PulseError::Operation("Operation cancelled".into()));
				}
			}
		}
	}

	pub fn get_default_device(&self, kind: DeviceKind) -> Result<DeviceInfo, PulseError> {
		let name = self.get_default_device_name(kind)?;
		self.get_device_by_name(kind, &name)
	}

	pub fn get_device_by_name(
		&self,
		kind: DeviceKind,
		name: &str,
	) -> Result<DeviceInfo, PulseError> {
		match kind {
			DeviceKind::Sink => {
				query_device!(
					self,
					kind,
					get_sink_info_by_name(name),
					introspect::SinkInfo
				)
			}
			DeviceKind::Source => {
				query_device!(
					self,
					kind,
					get_source_info_by_name(name),
					introspect::SourceInfo
				)
			}
		}
	}

	pub fn get_device_by_index(
		&self,
		kind: DeviceKind,
		index: u32,
	) -> Result<DeviceInfo, PulseError> {
		match kind {
			DeviceKind::Sink => {
				query_device!(
					self,
					kind,
					get_sink_info_by_index(index),
					introspect::SinkInfo
				)
			}
			DeviceKind::Source => {
				query_device!(
					self,
					kind,
					get_source_info_by_index(index),
					introspect::SourceInfo
				)
			}
		}
	}

	pub fn set_volume_by_index(&mut self, kind: DeviceKind, index: u32, volume: &ChannelVolumes) {
		let op = match kind {
			DeviceKind::Sink => self
				.introspect
				.set_sink_volume_by_index(index, volume, None),
			DeviceKind::Source => self
				.introspect
				.set_source_volume_by_index(index, volume, None),
		};
		if let Err(e) = self.wait_for_operation(op) {
			eprintln!("Failed to set {:?} volume: {}", kind, e);
		}
	}

	pub fn set_mute_by_index(&mut self, kind: DeviceKind, index: u32, mute: bool) {
		let op = match kind {
			DeviceKind::Sink => self.introspect.set_sink_mute_by_index(index, mute, None),
			DeviceKind::Source => self.introspect.set_source_mute_by_index(index, mute, None),
		};
		if let Err(e) = self.wait_for_operation(op) {
			eprintln!("Failed to set {:?} mute: {}", kind, e);
		}
	}

	fn get_default_device_name(&self, kind: DeviceKind) -> Result<String, PulseError> {
		let name: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

		let op = self.introspect.get_server_info(clone!(
			#[strong]
			name,
			move |info| {
				let value = match kind {
					DeviceKind::Sink => &info.default_sink_name,
					DeviceKind::Source => &info.default_source_name,
				};
				if let Some(cow) = value.as_ref() {
					name.replace(Some(cow.to_string()));
				}
			}
		));
		self.wait_for_operation(op)?;

		name.take()
			.ok_or_else(|| PulseError::GetInfo("No default device name".into()))
	}
}

impl Drop for VolumeController {
	fn drop(&mut self) {
		self.context.borrow_mut().disconnect();
		self.mainloop.borrow_mut().quit(pulse::def::Retval(0));
	}
}
