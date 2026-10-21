//! Force feedback on a racing wheel: plays the steering torque from the
//! tyre model, plus a damper so the wheel has some weight. Windows only,
//! through Windows.Gaming.Input, on its own thread so the game never waits
//! on the wheel.

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use bevy::prelude::*;
use windows::Foundation::TimeSpan;
use windows::Gaming::Input::ForceFeedback::{
	ConditionForceEffect, ConditionForceEffectKind, ConstantForceEffect,
	ForceFeedbackLoadEffectResult, ForceFeedbackMotor,
};
use windows::Gaming::Input::RawGameController;
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
use windows_numerics::Vector3;

use crate::car::{Car, SteeringTorque, THRUSTMASTER};

/// Steering torque in Nm that gives the wheel's full force.
const FULL_FORCE_TORQUE: f32 = 250.0;
/// Overall strength, from 0 to 1.
const GAIN: f32 = 0.7;
/// Resists fast wheel movements so the wheel has some weight, from 0 to 1.
const DAMPER: f32 = 0.15;
/// On the T300, a positive force turns the wheel left.
const FORCE_TO_TURN_RIGHT: f32 = -1.0;

const UPDATE_INTERVAL: Duration = Duration::from_millis(5);
/// Each update only plays for this long, so the wheel lets go by itself if
/// updates stop.
const FORCE_DURATION: Duration = Duration::from_millis(500);
/// If the game stops sending forces (frozen or closing), let go of the
/// wheel.
const STALE_AFTER: Duration = Duration::from_millis(250);
const RETRY_AFTER: Duration = Duration::from_secs(1);

pub struct ForceFeedbackPlugin;

impl Plugin for ForceFeedbackPlugin {
	fn build(&self, app: &mut App) {
		// Scripted test runs shouldn't turn the wheel of whoever sits at it.
		if std::env::args().any(|arg| arg == "--script") {
			return;
		}

		let latest = Latest::default();
		let thread_latest = latest.clone();
		thread::spawn(move || run(&thread_latest));

		app.insert_resource(ForceFeedback(latest))
			.add_systems(Update, send_force);
	}
}

/// The latest force for the wheel (-1 to 1, positive turns it right) and
/// when it was sent.
type Latest = Arc<Mutex<Option<(f32, Instant)>>>;

#[derive(Resource)]
struct ForceFeedback(Latest);

fn send_force(
	force_feedback: Res<ForceFeedback>,
	torque: Single<&SteeringTorque, With<Car>>,
) {
	let force = (torque.0 / FULL_FORCE_TORQUE * GAIN).clamp(-1.0, 1.0);
	*force_feedback.0.lock().unwrap() = Some((force, Instant::now()));
}

fn run(latest: &Latest) {
	if let Err(error) = unsafe { RoInitialize(RO_INIT_MULTITHREADED) } {
		warn!("Force feedback is off: {error}");
		return;
	}

	loop {
		match find_wheel() {
			Ok(Some(motor)) => {
				if let Err(error) = drive(&motor, latest) {
					warn!("Force feedback stopped: {error}");
				}
			}
			Ok(None) => {}
			Err(error) => warn!("Can't look for a wheel: {error}"),
		}
		thread::sleep(RETRY_AFTER);
	}
}

fn find_wheel() -> windows::core::Result<Option<ForceFeedbackMotor>> {
	for controller in &RawGameController::RawGameControllers()? {
		if controller.HardwareVendorId()? != THRUSTMASTER {
			continue;
		}

		if let Some(motor) =
			controller.ForceFeedbackMotors()?.into_iter().next()
		{
			info!("Force feedback on {}", controller.DisplayName()?);
			return Ok(Some(motor));
		}
	}

	Ok(None)
}

/// Plays forces on the wheel until something fails, for example when it is
/// unplugged.
fn drive(
	motor: &ForceFeedbackMotor,
	latest: &Latest,
) -> windows::core::Result<()> {
	let along_wheel = Vector3 {
		X: 1.0,
		Y: 0.0,
		Z: 0.0,
	};

	let damper =
		ConditionForceEffect::CreateInstance(ConditionForceEffectKind::Damper)?;
	damper.SetParameters(along_wheel, DAMPER, DAMPER, 1.0, 1.0, 0.0, 0.0)?;
	if motor.LoadEffectAsync(&damper)?.join()?
		== ForceFeedbackLoadEffectResult::Succeeded
	{
		damper.Start()?;
	}

	let constant = ConstantForceEffect::new()?;
	if motor.LoadEffectAsync(&constant)?.join()?
		!= ForceFeedbackLoadEffectResult::Succeeded
	{
		warn!("The wheel can't play a constant force");
		return Ok(());
	}

	let duration = TimeSpan {
		Duration: FORCE_DURATION.as_nanos() as i64 / 100,
	};
	loop {
		let force = match *latest.lock().unwrap() {
			Some((force, sent)) if sent.elapsed() < STALE_AFTER => force,
			_ => 0.0,
		};

		constant.SetParameters(
			along_wheel * (force * FORCE_TO_TURN_RIGHT),
			duration,
		)?;
		constant.Start()?;
		thread::sleep(UPDATE_INTERVAL);
	}
}
