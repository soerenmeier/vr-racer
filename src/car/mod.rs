mod input;
mod physics;

pub use input::THRUSTMASTER;
pub use physics::SteeringTorque;

use std::f32::consts::PI;

use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;

const MODEL: &str = "models/porsche_911.glb";
/// The model's own root nodes shrink it to 1% and turn it backwards, so
/// undo that. Inside the model, positions are then in car space (metres,
/// facing -Z).
const MODEL_SCALE: f32 = 100.0;

/// Nodes in the model whose name contains this are the steering wheel.
const STEERING_WHEEL_NODE: &str = "steering_wheel";
/// Measured from the model's vertices, in car space.
const STEERING_WHEEL_CENTER: Vec3 = Vec3::new(-0.335, 0.853, -0.241);
const STEERING_WHEEL_AXIS: Vec3 = Vec3::new(0.0, 0.267, 0.964);
/// How far the steering wheel turns at full lock (900° lock to lock).
const STEERING_WHEEL_LOCK: f32 = 450.0_f32.to_radians();

const SPAWN_POSITION: Vec3 = Vec3::new(0.0, 0.1, 0.0);
const DRIVER_HEAD: Vec3 = Vec3::new(-0.335, 1.1, 0.4);

pub struct CarPlugin;

impl Plugin for CarPlugin {
	fn build(&self, app: &mut App) {
		app.add_plugins(physics::CarPhysicsPlugin)
			.add_systems(Startup, spawn_car)
			.add_observer(find_steering_wheel)
			.add_observer(input::configure_wheel)
			.add_systems(
				Update,
				(input::read_controls, turn_steering_wheel).chain(),
			);
	}
}

#[derive(Component)]
pub struct Car;

/// What the driver is doing. Throttle and brake go from 0 to 1, steer
/// from -1 (full left) to 1 (full right).
#[derive(Component, Default)]
pub struct CarInput {
	pub throttle: f32,
	pub brake: f32,
	pub steer: f32,
}

/// A steering wheel node of the model, with where it sits when centred.
#[derive(Component)]
struct SteeringWheel {
	rest: Vec3,
}

/// Where the driver's eyes are. The camera attaches here.
#[derive(Component)]
pub struct DriverHead;

fn spawn_car(mut commands: Commands, asset_server: Res<AssetServer>) {
	let model = asset_server.load(GltfAssetLabel::Scene(0).from_asset(MODEL));

	commands.spawn((
		Car,
		CarInput::default(),
		physics::chassis(),
		Transform::from_translation(SPAWN_POSITION),
		Visibility::default(),
		children![
			(
				WorldAssetRoot(model),
				Transform::from_scale(Vec3::splat(MODEL_SCALE))
					.with_rotation(Quat::from_rotation_y(PI)),
			),
			(
				DriverHead,
				Transform::from_translation(DRIVER_HEAD),
				Visibility::default(),
			),
		],
	));
}

/// Marks the model's steering wheel nodes once the model has spawned. Their
/// mesh children have the same name, so only take the outermost node.
fn find_steering_wheel(
	ready: On<WorldInstanceReady>,
	children: Query<&Children>,
	nodes: Query<(&Name, &Transform, &ChildOf)>,
	mut commands: Commands,
) {
	let is_wheel = |entity| {
		nodes
			.get(entity)
			.is_ok_and(|(name, ..)| name.as_str().contains(STEERING_WHEEL_NODE))
	};

	for entity in children.iter_descendants(ready.entity) {
		let Ok((_, transform, child_of)) = nodes.get(entity) else {
			continue;
		};

		if is_wheel(entity) && !is_wheel(child_of.parent()) {
			commands.entity(entity).insert(SteeringWheel {
				rest: transform.translation,
			});
		}
	}
}

/// Spins the wheel nodes around the wheel's own axis. Clockwise (as the
/// driver sees it) for positive steer.
fn turn_steering_wheel(
	input: Single<&CarInput>,
	mut wheels: Query<(&mut Transform, &SteeringWheel)>,
) {
	let angle = -input.steer * STEERING_WHEEL_LOCK;
	let rotation =
		Quat::from_axis_angle(STEERING_WHEEL_AXIS.normalize(), angle);

	for (mut transform, wheel) in &mut wheels {
		transform.rotation = rotation;
		transform.translation = STEERING_WHEEL_CENTER
			+ rotation * (wheel.rest - STEERING_WHEEL_CENTER);
	}
}
