//! VR through OpenXR, for example a Quest over Meta Quest Link. Without a
//! headset the game runs on the monitor as before.

use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;
use bevy::render::pipelined_rendering::PipelinedRenderingPlugin;
use bevy_mod_openxr::add_xr_plugins;
use bevy_mod_openxr::reference_space::OxrReferenceSpacePlugin;
use bevy_mod_xr::session::XrTrackingRoot;
use openxr::ReferenceSpaceType;

use crate::car::DriverHead;
use crate::script;

/// Bevy's default plugins, with VR unless this is a scripted test run.
pub fn default_plugins() -> PluginGroupBuilder {
	if script::is_scripted() {
		return DefaultPlugins.build();
	}

	// Pipelined rendering adds a frame of latency, which you feel in VR.
	add_xr_plugins(DefaultPlugins.build().disable::<PipelinedRenderingPlugin>())
		// Seated: the space starts where the headset is when the game
		// starts, and the headset's own recentre moves it back there.
		.set(OxrReferenceSpacePlugin {
			default_primary_ref_space: ReferenceSpaceType::LOCAL,
		})
}

pub struct VrPlugin;

impl Plugin for VrPlugin {
	fn build(&self, app: &mut App) {
		app.add_observer(follow_driver_head);
	}
}

/// Puts the tracking space at the driver's eyes, so the headset moves with
/// the car.
fn follow_driver_head(
	add: On<Add<DriverHead>>,
	roots: Query<Entity, With<XrTrackingRoot>>,
	mut commands: Commands,
) {
	for root in &roots {
		commands.entity(root).insert(ChildOf(add.entity));
	}
}
