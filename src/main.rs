mod camera;
mod car;
mod world;

use bevy::prelude::*;

fn main() {
	App::new()
		.add_plugins((
			DefaultPlugins,
			world::WorldPlugin,
			car::CarPlugin,
			camera::CameraPlugin,
		))
		.run();
}
