//! Checks force feedback on a wheel through Windows.Gaming.Input: lists the
//! controllers and their motors, then pushes the wheel briefly each way and
//! reports how far it turned. Keep your hands off the wheel.
//! Run with `cargo run --example ffb_probe`.

use std::thread::sleep;
use std::time::Duration;

use windows::Foundation::TimeSpan;
use windows::Gaming::Input::ForceFeedback::{
	ConstantForceEffect, ForceFeedbackLoadEffectResult,
};
use windows::Gaming::Input::{GameControllerSwitchPosition, RawGameController};
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
use windows_numerics::Vector3;

const FORCE: f32 = 0.3;
const PUSH_FOR: Duration = Duration::from_millis(500);

fn main() -> windows::core::Result<()> {
	unsafe { RoInitialize(RO_INIT_MULTITHREADED)? };

	// Controllers show up a moment after the first call.
	let mut controllers = RawGameController::RawGameControllers()?;
	for _ in 0..30 {
		if controllers.Size()? > 0 {
			break;
		}
		sleep(Duration::from_millis(100));
		controllers = RawGameController::RawGameControllers()?;
	}

	for controller in &controllers {
		let motors = controller.ForceFeedbackMotors()?;
		println!(
			"{} (vendor {:04x}, product {:04x}): {} axes, {} buttons, {} force feedback motors",
			controller.DisplayName()?,
			controller.HardwareVendorId()?,
			controller.HardwareProductId()?,
			controller.AxisCount()?,
			controller.ButtonCount()?,
			motors.Size()?,
		);

		for motor in &motors {
			println!(
				"  motor: axes {:?}, enabled {}, paused {}, gain {}",
				motor.SupportedAxes()?,
				motor.IsEnabled()?,
				motor.AreEffectsPaused()?,
				motor.MasterGain()?,
			);

			let effect = ConstantForceEffect::new()?;
			let result = motor.LoadEffectAsync(&effect)?.join()?;
			println!("  load constant force: {result:?}");
			if result != ForceFeedbackLoadEffectResult::Succeeded {
				continue;
			}

			for force in [FORCE, -FORCE, FORCE] {
				let before = steering(&controller)?;
				effect.SetParameters(
					Vector3 {
						X: force,
						Y: 0.0,
						Z: 0.0,
					},
					TimeSpan {
						Duration: PUSH_FOR.as_nanos() as i64 / 100,
					},
				)?;
				effect.Start()?;
				let mut path = Vec::new();
				for _ in 0..5 {
					sleep(PUSH_FOR / 5);
					path.push(format!("{:.3}", steering(&controller)?));
				}
				effect.Stop()?;
				println!(
					"  force {force:+}: steering {before:.3} -> {}",
					path.join(" ")
				);
				sleep(Duration::from_millis(700));
			}

			motor.TryUnloadEffectAsync(&effect)?.join()?;
		}
	}

	Ok(())
}

/// The first axis, from 0 (left) to 1 (right).
fn steering(controller: &RawGameController) -> windows::core::Result<f64> {
	let mut buttons = vec![false; controller.ButtonCount()? as usize];
	let mut switches = vec![
		GameControllerSwitchPosition::Center;
		controller.SwitchCount()? as usize
	];
	let mut axes = vec![0.0; controller.AxisCount()? as usize];
	controller.GetCurrentReading(&mut buttons, &mut switches, &mut axes)?;
	Ok(axes[0])
}
