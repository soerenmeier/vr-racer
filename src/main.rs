mod camera;
mod car;
mod script;
mod world;

use avian3d::prelude::*;
use bevy::prelude::*;

fn main() {
	App::new()
		.add_plugins((
			DefaultPlugins,
			PhysicsPlugins::default(),
			world::WorldPlugin,
			car::CarPlugin,
			camera::CameraPlugin,
			script::ScriptPlugin,
		))
		.run();
}
