use bevy::prelude::*;

const CAMERA_START: Vec3 = Vec3::new(0.0, 4.0, 10.0);

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
	fn build(&self, app: &mut App) {
		app.add_systems(Startup, spawn_camera);
	}
}

fn spawn_camera(mut commands: Commands) {
	commands.spawn((
		Camera3d::default(),
		Transform::from_translation(CAMERA_START)
			.looking_at(Vec3::ZERO, Vec3::Y),
	));
}
