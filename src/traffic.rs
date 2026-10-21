//! Cars driving along the highway at a steady speed per lane, slower on
//! the right, for the player to overtake.

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;

use crate::highway::{LANES, ROAD_LENGTH, lane_x};
use crate::race::RaceState;

struct TrafficModel {
	path: &'static str,
	/// Width, height and length in metres, for the collider.
	size: Vec3,
	wheel_radius: f32,
}

/// Prepared in Blender: scaled to metres, centred, on the ground, facing -Z,
/// with each wheel a separate node centred on its axle.
const MODELS: [TrafficModel; 3] = [
	TrafficModel {
		path: "models/traffic/ford_bronco.glb",
		size: Vec3::new(2.19, 1.98, 4.81),
		wheel_radius: BRONCO_WHEEL_RADIUS,
	},
	TrafficModel {
		path: "models/traffic/fiat_500_abarth.glb",
		size: Vec3::new(1.9, 1.52, 3.66),
		wheel_radius: FIAT_WHEEL_RADIUS,
	},
	TrafficModel {
		path: "models/traffic/toyota_hilux.glb",
		size: Vec3::new(2.23, 1.98, 5.33),
		wheel_radius: HILUX_WHEEL_RADIUS,
	},
];

/// Measured in Blender from each model's tyres.
const BRONCO_WHEEL_RADIUS: f32 = 0.456;
const FIAT_WHEEL_RADIUS: f32 = 0.246;
const HILUX_WHEEL_RADIUS: f32 = 0.403;

/// The names Blender gave the wheel nodes.
const WHEEL_NODES: [&str; 4] = [
	"wheel_front_left",
	"wheel_front_right",
	"wheel_rear_left",
	"wheel_rear_right",
];

/// Speed of each lane in km/h, from the right lane to the left.
const LANE_SPEEDS: [f32; LANES] = [80.0, 100.0, 120.0];
const CARS_PER_LANE: usize = 320;
/// Distance from the start to the first car.
const FIRST_CAR: f32 = 10.0;
/// Traffic sets off together with the player, from standing still.
const ACCELERATION: f32 = 3.0;

pub struct TrafficPlugin;

impl Plugin for TrafficPlugin {
	fn build(&self, app: &mut App) {
		app.add_systems(Startup, spawn_traffic)
			.add_observer(find_wheels)
			.add_systems(
				FixedUpdate,
				(
					accelerate.run_if(in_state(RaceState::Racing)),
					remove_at_road_end,
				),
			)
			.add_systems(Update, spin_wheels);
	}
}

#[derive(Component)]
pub struct TrafficCar {
	/// The speed of its lane in m/s.
	speed: f32,
	wheel_radius: f32,
}

/// A wheel node of a traffic car's model.
#[derive(Component)]
struct TrafficWheel {
	car: Entity,
}

fn spawn_traffic(mut commands: Commands, asset_server: Res<AssetServer>) {
	let scenes = MODELS.map(|model| {
		asset_server.load(GltfAssetLabel::Scene(0).from_asset(model.path))
	});
	let spacing = ROAD_LENGTH / CARS_PER_LANE as f32;

	for (lane, speed) in LANE_SPEEDS.into_iter().enumerate() {
		for i in 0..CARS_PER_LANE {
			let model = (i + lane) % MODELS.len();
			let size = MODELS[model].size;

			// Stagger the lanes so cars don't drive side by side.
			let z = -(FIRST_CAR + (i as f32 + lane as f32 / 3.0) * spacing);

			commands.spawn((
				TrafficCar {
					speed: speed / 3.6,
					wheel_radius: MODELS[model].wheel_radius,
				},
				RigidBody::Kinematic,
				Collider::compound(vec![(
					Vec3::Y * size.y / 2.0,
					Quat::IDENTITY,
					Collider::cuboid(size.x, size.y, size.z),
				)]),
				TransformInterpolation,
				Transform::from_xyz(lane_x(lane), 0.0, z),
				WorldAssetRoot(scenes[model].clone()),
			));
		}
	}
}

/// Marks the wheel nodes of a traffic car once its model has spawned.
fn find_wheels(
	ready: On<WorldInstanceReady>,
	cars: Query<(), With<TrafficCar>>,
	children: Query<&Children>,
	names: Query<&Name>,
	mut commands: Commands,
) {
	if !cars.contains(ready.entity) {
		return;
	}

	for entity in children.iter_descendants(ready.entity) {
		if names
			.get(entity)
			.is_ok_and(|name| WHEEL_NODES.contains(&name.as_str()))
		{
			commands
				.entity(entity)
				.insert(TrafficWheel { car: ready.entity });
		}
	}
}

/// Rolls the wheels at the car's speed. Forward is -Z, so a rolling wheel
/// turns backwards around X.
fn spin_wheels(
	time: Res<Time>,
	cars: Query<(&LinearVelocity, &TrafficCar)>,
	mut wheels: Query<(&mut Transform, &TrafficWheel)>,
) {
	for (mut transform, wheel) in &mut wheels {
		let Ok((velocity, car)) = cars.get(wheel.car) else {
			continue;
		};

		let speed = -velocity.z;
		transform.rotate_local_x(-speed / car.wheel_radius * time.delta_secs());
	}
}

/// All cars speed up at the same rate, so the gaps in a lane stay the same.
fn accelerate(
	time: Res<Time>,
	mut cars: Query<(&mut LinearVelocity, &TrafficCar)>,
) {
	for (mut velocity, car) in &mut cars {
		let speed =
			(-velocity.z + ACCELERATION * time.delta_secs()).min(car.speed);
		velocity.z = -speed;
	}
}

/// Cars that reach the end of the highway leave. They don't start over at
/// the beginning, as they could land on top of the player.
fn remove_at_road_end(
	cars: Query<(Entity, &Position), With<TrafficCar>>,
	mut commands: Commands,
) {
	for (entity, position) in &cars {
		if position.z < -ROAD_LENGTH {
			commands.entity(entity).despawn();
		}
	}
}
