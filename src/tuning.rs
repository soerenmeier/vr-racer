//! A panel (F1) to tune the car and force feedback while driving, saved to
//! tuning.ron and loaded at startup.

use std::fs;

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_egui::{
	EguiContext, EguiGlobalSettings, EguiPlugin, EguiPrimaryContextPass,
	PrimaryEguiContext, egui,
};
use bevy_inspector_egui::DefaultInspectorConfigPlugin;
use bevy_inspector_egui::bevy_inspector::ui_for_resource;
use serde::{Deserialize, Serialize};

use crate::camera::DriverCamera;
use crate::car::{Car, CarInput, CarTuning, SteeringTorque};
#[cfg(windows)]
use crate::force_feedback::{ForceFeedbackTuning, wheel_force};

/// In the folder the game runs from, so `cargo run` uses the repo's copy.
const TUNING_FILE: &str = "tuning.ron";
const TOGGLE_KEY: KeyCode = KeyCode::F1;

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct TuningFile {
	car: CarTuning,
	#[cfg(windows)]
	force_feedback: ForceFeedbackTuning,
}

pub struct TuningPlugin;

impl Plugin for TuningPlugin {
	fn build(&self, app: &mut App) {
		let tuning = load();

		app.add_plugins((EguiPlugin::default(), DefaultInspectorConfigPlugin))
			// The UI cameras that draw the cockpit displays come first, and
			// in VR there are eye cameras too, so pick the monitor's camera
			// for the panel by hand.
			.insert_resource(EguiGlobalSettings {
				auto_create_primary_context: false,
				..default()
			})
			.insert_resource(tuning.car)
			.add_observer(attach_to_driver_camera)
			.add_systems(EguiPrimaryContextPass, tuning_panel);

		#[cfg(windows)]
		app.insert_resource(tuning.force_feedback);
	}
}

fn load() -> TuningFile {
	let Ok(text) = fs::read_to_string(TUNING_FILE) else {
		return TuningFile::default();
	};

	ron::from_str(&text).unwrap_or_else(|error| {
		warn!("Can't read {TUNING_FILE}, using the defaults: {error}");
		TuningFile::default()
	})
}

fn save(world: &World) -> Result<(), String> {
	let tuning = TuningFile {
		car: world.resource::<CarTuning>().clone(),
		#[cfg(windows)]
		force_feedback: world.resource::<ForceFeedbackTuning>().clone(),
	};

	let text = ron::ser::to_string_pretty(&tuning, default())
		.map_err(|error| error.to_string())?;
	fs::write(TUNING_FILE, text).map_err(|error| error.to_string())
}

fn attach_to_driver_camera(add: On<Add<DriverCamera>>, mut commands: Commands) {
	commands.entity(add.entity).insert(PrimaryEguiContext);
}

fn tuning_panel(
	world: &mut World,
	mut open: Local<bool>,
	mut status: Local<String>,
) {
	if world
		.resource::<ButtonInput<KeyCode>>()
		.just_pressed(TOGGLE_KEY)
	{
		*open = !*open;
	}
	if !*open {
		return;
	}

	let Ok(context) = world
		.query_filtered::<&mut EguiContext, With<PrimaryEguiContext>>()
		.single(world)
	else {
		return;
	};
	let mut context = context.clone();
	let readout = readout(world);

	egui::Window::new("Tuning").show(context.get_mut(), |ui| {
		ui.label(readout);
		ui.separator();

		egui::ScrollArea::vertical().show(ui, |ui| {
			egui::CollapsingHeader::new("Car")
				.default_open(true)
				.show(ui, |ui| ui_for_resource::<CarTuning>(world, ui));

			#[cfg(windows)]
			egui::CollapsingHeader::new("Force feedback")
				.default_open(true)
				.show(ui, |ui| {
					ui_for_resource::<ForceFeedbackTuning>(world, ui)
				});
		});
		ui.separator();

		ui.horizontal(|ui| {
			if ui.button("Save").clicked() {
				*status = match save(world) {
					Ok(()) => format!("Saved to {TUNING_FILE}"),
					Err(error) => format!("Can't save: {error}"),
				};
			}

			if ui.button("Reset to defaults").clicked() {
				world.insert_resource(CarTuning::default());
				#[cfg(windows)]
				world.insert_resource(ForceFeedbackTuning::default());
				*status = "Reset, not saved yet".to_string();
			}

			ui.label(status.as_str());
		});
	});
}

/// Live values that help while tuning.
fn readout(world: &mut World) -> String {
	let Ok((velocity, input, torque)) = world
		.query_filtered::<(&LinearVelocity, &CarInput, &SteeringTorque), With<Car>>(
		)
		.single(world)
	else {
		return String::new();
	};

	let mut text = format!(
		"Speed: {:.0} km/h\nThrottle: {:.2}  Brake: {:.2}  Steer: {:+.2}\n\
		 Steering torque: {:+.0} Nm",
		velocity.length() * 3.6,
		input.throttle,
		input.brake,
		input.steer,
		torque.0,
	);

	#[cfg(windows)]
	{
		let tuning = world.resource::<ForceFeedbackTuning>();
		text.push_str(&format!(
			"\nWheel force: {:+.2}",
			wheel_force(tuning, torque.0)
		));
	}

	text
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn saved_tuning_loads_back() {
		let mut tuning = TuningFile::default();
		tuning.car.tyre_grip = 1.3;

		let text = ron::ser::to_string_pretty(&tuning, default()).unwrap();
		let loaded: TuningFile = ron::from_str(&text).unwrap();

		assert_eq!(loaded.car.tyre_grip, 1.3);
	}

	#[test]
	fn missing_values_use_the_defaults() {
		let loaded: TuningFile =
			ron::from_str("(car: (tyre_grip: 1.3))").unwrap();

		assert_eq!(loaded.car.tyre_grip, 1.3);
		assert_eq!(loaded.car.brake_force, CarTuning::default().brake_force);
	}
}
