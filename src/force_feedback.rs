//! Force feedback on a racing wheel: plays the steering torque from the
//! tyre model, plus a damper so the wheel has some weight. Windows only,
//! through Windows.Gaming.Input, on its own thread so the game never waits
//! on the wheel.

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_inspector_egui::prelude::*;
use serde::{Deserialize, Serialize};
use windows::Foundation::TimeSpan;
use windows::Gaming::Input::ForceFeedback::{
	ConditionForceEffect, ConditionForceEffectKind, ConstantForceEffect,
	ForceFeedbackLoadEffectResult, ForceFeedbackMotor,
};
use windows::Gaming::Input::RawGameController;
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
use windows_numerics::Vector3;

use crate::car::{Car, SteeringTorque, THRUSTMASTER};
use crate::script;

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

/// How strong the force feedback is. Tuned in the tuning panel (F1) and
/// saved to tuning.ron.
#[derive(
	Resource, Reflect, InspectorOptions, Serialize, Deserialize, Clone,
)]
#[reflect(Resource, InspectorOptions)]
#[serde(default)]
pub struct ForceFeedbackTuning {
	/// Overall strength, from 0 to 1.
	#[inspector(min = 0.0, max = 1.0)]
	pub gain: f32,
	/// Steering torque in Nm that gives the wheel's full force.
	#[inspector(min = 20.0, max = 1000.0)]
	pub full_force_torque: f32,
	/// Resists fast wheel movements so the wheel has some weight, from 0
	/// to 1.
	#[inspector(min = 0.0, max = 1.0)]
	pub damper: f32,
}

impl Default for ForceFeedbackTuning {
	fn default() -> Self {
		Self {
			gain: 0.7,
			full_force_torque: 250.0,
			damper: 0.15,
		}
	}
}

/// The force for the wheel from the steering torque, from -1 to 1. Positive
/// turns it right.
pub fn wheel_force(tuning: &ForceFeedbackTuning, torque: f32) -> f32 {
	(torque / tuning.full_force_torque * tuning.gain).clamp(-1.0, 1.0)
}

pub struct ForceFeedbackPlugin;

impl Plugin for ForceFeedbackPlugin {
	fn build(&self, app: &mut App) {
		app.init_resource::<ForceFeedbackTuning>()
			.register_type::<ForceFeedbackTuning>();

		// Scripted test runs shouldn't turn the wheel of whoever sits at it.
		if script::is_scripted() {
			return;
		}

		let latest = Latest::default();
		let thread_latest = latest.clone();
		thread::spawn(move || run(&thread_latest));

		app.insert_resource(ForceFeedback(latest))
			.add_systems(Update, send_force);
	}
}

/// What the game last sent to the wheel thread.
struct Sent {
	force: f32,
	damper: f32,
	at: Instant,
}

type Latest = Arc<Mutex<Option<Sent>>>;

#[derive(Resource)]
struct ForceFeedback(Latest);

fn send_force(
	force_feedback: Res<ForceFeedback>,
	tuning: Res<ForceFeedbackTuning>,
	torque: Single<&SteeringTorque, With<Car>>,
) {
	*force_feedback.0.lock().unwrap() = Some(Sent {
		force: wheel_force(&tuning, torque.0),
		damper: tuning.damper,
		at: Instant::now(),
	});
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
	let has_damper = motor.LoadEffectAsync(&damper)?.join()?
		== ForceFeedbackLoadEffectResult::Succeeded;

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
	let mut damper_strength = None;
	loop {
		let (force, strength) = match &*latest.lock().unwrap() {
			Some(sent) if sent.at.elapsed() < STALE_AFTER => {
				(sent.force, sent.damper)
			}
			_ => (0.0, 0.0),
		};

		if has_damper && damper_strength != Some(strength) {
			damper.SetParameters(
				along_wheel,
				strength,
				strength,
				1.0,
				1.0,
				0.0,
				0.0,
			)?;
			damper.Start()?;
			damper_strength = Some(strength);
		}

		constant.SetParameters(
			along_wheel * (force * FORCE_TO_TURN_RIGHT),
			duration,
		)?;
		constant.Start()?;
		thread::sleep(UPDATE_INTERVAL);
	}
}
