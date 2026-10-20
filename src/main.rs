mod camera;

use bevy::prelude::*;

fn main() {
	App::new()
		.add_plugins((DefaultPlugins, camera::CameraPlugin))
		.run();
}
