mod camera;
mod car;
mod highway;
mod hud;
mod race;
mod script;
mod traffic;

use avian3d::prelude::*;
use bevy::prelude::*;

fn main() {
	App::new()
		.add_plugins((
			DefaultPlugins,
			PhysicsPlugins::default(),
			highway::HighwayPlugin,
			traffic::TrafficPlugin,
			race::RacePlugin,
			hud::HudPlugin,
			car::CarPlugin,
			camera::CameraPlugin,
			script::ScriptPlugin,
		))
		.run();
}
