use bevy::prelude::*;

use crate::car::DriverHead;

const FIELD_OF_VIEW: f32 = 70.0_f32.to_radians();

/// The camera on the monitor. In VR the headset has its own cameras.
#[derive(Component)]
pub struct DriverCamera;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
	fn build(&self, app: &mut App) {
		app.add_observer(attach_camera);
	}
}

/// Puts the camera at the driver's eyes, so it moves with the car.
fn attach_camera(add: On<Add<DriverHead>>, mut commands: Commands) {
	commands.spawn((
		DriverCamera,
		Camera3d::default(),
		Projection::Perspective(PerspectiveProjection {
			fov: FIELD_OF_VIEW,
			..default()
		}),
		Transform::default(),
		ChildOf(add.entity),
	));
}
