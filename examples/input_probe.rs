//! Logs raw game controller events, to see how a wheel's axes and buttons
//! show up. Run with `cargo run --example input_probe`.

use std::collections::BTreeMap;

use bevy::input::gamepad::RawGamepadEvent;
use bevy::prelude::*;

const RUN_FOR: f32 = 75.0;

fn main() {
	App::new()
		.add_plugins(DefaultPlugins)
		.add_systems(Startup, |mut commands: Commands| {
			commands.spawn(Camera2d);
		})
		.add_systems(Update, log_events)
		.run();
}

fn log_events(
	time: Res<Time>,
	mut events: MessageReader<RawGamepadEvent>,
	mut ranges: Local<BTreeMap<String, (f32, f32)>>,
	mut exit: MessageWriter<AppExit>,
) {
	let t = time.elapsed_secs();

	for event in events.read() {
		match event {
			RawGamepadEvent::Connection(e) => {
				println!("{t:6.2} CONNECTION {:?}", e.connection);
			}
			RawGamepadEvent::Button(e) => {
				println!("{t:6.2} BUTTON {:?} {:.3}", e.button, e.value);
			}
			RawGamepadEvent::Axis(e) => {
				let key = format!("{:?}", e.axis);
				let range = ranges.entry(key).or_insert((e.value, e.value));
				range.0 = range.0.min(e.value);
				range.1 = range.1.max(e.value);
				println!("{t:6.2} AXIS {:?} {:.3}", e.axis, e.value);
			}
		}
	}

	if t > RUN_FOR {
		for (axis, (min, max)) in ranges.iter() {
			println!("RANGE {axis}: {min:.3} .. {max:.3}");
		}
		exit.write(AppExit::Success);
	}
}
