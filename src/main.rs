mod camera;
mod car;
#[cfg(windows)]
mod force_feedback;
mod highway;
mod hud;
mod race;
mod script;
mod traffic;
mod tuning;

use avian3d::prelude::*;
use bevy::prelude::*;

fn main() {
	let mut app = App::new();
	app.add_plugins((
		DefaultPlugins,
		PhysicsPlugins::default(),
		highway::HighwayPlugin,
		traffic::TrafficPlugin,
		race::RacePlugin,
		hud::HudPlugin,
		car::CarPlugin,
		camera::CameraPlugin,
		script::ScriptPlugin,
	));

	#[cfg(windows)]
	app.add_plugins(force_feedback::ForceFeedbackPlugin);

	// After the plugins whose tuning it loads, so the saved values win.
	app.add_plugins(tuning::TuningPlugin);

	app.run();
}
