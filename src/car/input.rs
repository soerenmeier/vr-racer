//! Driver input from the keyboard and a racing wheel. Both work at the same
//! time, so scripts can drive with keys while a wheel is plugged in.

use bevy::input::gamepad::{AxisSettings, GamepadSettings};
use bevy::prelude::*;

use super::CarInput;

/// How fast keyboard steering moves towards full lock, per second.
const KEYBOARD_STEER_RATE: f32 = 2.5;

/// Thrustmaster's USB vendor id. Other controllers map their axes
/// differently, so only these are read as a wheel.
pub const THRUSTMASTER: u16 = 0x044f;
/// Axes of a Thrustmaster T300RS, as gilrs maps them. Pedals go from their
/// released value to the opposite end.
const STEER_AXIS: GamepadAxis = GamepadAxis::LeftStickX;
const THROTTLE_AXIS: GamepadAxis = GamepadAxis::RightStickX;
const THROTTLE_RELEASED: f32 = 1.0;
const BRAKE_AXIS: GamepadAxis = GamepadAxis::LeftStickY;
const BRAKE_RELEASED: f32 = -1.0;

/// Bevy ignores small stick movements by default (5% around the centre and
/// changes under 1%), which on a wheel is many degrees of steering.
pub(super) fn configure_wheel(
	add: On<Add<Gamepad>>,
	mut settings: Query<&mut GamepadSettings>,
) {
	if let Ok(mut settings) = settings.get_mut(add.entity) {
		settings.default_axis_settings =
			AxisSettings::new(-1.0, 0.0, 0.0, 1.0, 0.0)
				.expect("valid axis settings");
	}
}

pub(super) fn read_controls(
	keys: Res<ButtonInput<KeyCode>>,
	time: Res<Time>,
	gamepads: Query<&Gamepad>,
	mut inputs: Query<&mut CarInput>,
	mut keyboard_steer: Local<f32>,
) {
	let pressed = |key| if keys.pressed(key) { 1.0 } else { 0.0 };
	let target_steer = pressed(KeyCode::KeyD) - pressed(KeyCode::KeyA);
	let step = KEYBOARD_STEER_RATE * time.delta_secs();
	*keyboard_steer += (target_steer - *keyboard_steer).clamp(-step, step);

	let mut throttle = pressed(KeyCode::KeyW);
	let mut brake = pressed(KeyCode::KeyS);
	let mut steer = *keyboard_steer;

	for wheel in gamepads
		.iter()
		.filter(|gamepad| gamepad.vendor_id() == Some(THRUSTMASTER))
	{
		// Axes that haven't reported yet count as released.
		let axis = |axis, released| wheel.get(axis).unwrap_or(released);

		throttle = throttle.max(pedal(
			axis(THROTTLE_AXIS, THROTTLE_RELEASED),
			THROTTLE_RELEASED,
		));
		brake =
			brake.max(pedal(axis(BRAKE_AXIS, BRAKE_RELEASED), BRAKE_RELEASED));
		steer += axis(STEER_AXIS, 0.0);
	}

	for mut input in &mut inputs {
		input.throttle = throttle;
		input.brake = brake;
		input.steer = steer.clamp(-1.0, 1.0);
	}
}

/// How far a pedal is pressed, from 0 to 1, given its axis value and the
/// value it has when released (1 or -1).
fn pedal(value: f32, released: f32) -> f32 {
	((released - value) * released / 2.0).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn pedals_go_from_released_to_pressed() {
		assert_eq!(pedal(THROTTLE_RELEASED, THROTTLE_RELEASED), 0.0);
		assert_eq!(pedal(-THROTTLE_RELEASED, THROTTLE_RELEASED), 1.0);
		assert_eq!(pedal(BRAKE_RELEASED, BRAKE_RELEASED), 0.0);
		assert_eq!(pedal(0.0, BRAKE_RELEASED), 0.5);
		assert_eq!(pedal(-BRAKE_RELEASED, BRAKE_RELEASED), 1.0);
	}
}
