//! Raycast vehicle: the chassis is one rigid body, and each wheel casts a
//! ray to the ground and pushes on the chassis with suspension and tyre
//! forces at its contact point.

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_inspector_egui::prelude::*;
use serde::{Deserialize, Serialize};

use super::{Car, CarInput};

/// Porsche 911 Carrera GTS, roughly.
const MASS: f32 = 1450.0;
/// Low and towards the rear, as the engine sits behind the rear axle.
const CENTER_OF_MASS: Vec3 = Vec3::new(0.0, 0.45, 0.27);
/// A solid box of the car's size and mass.
const ANGULAR_INERTIA: Vec3 = Vec3::new(2460.0, 2730.0, 510.0);
const BODY_SIZE: Vec3 = Vec3::new(1.8, 0.9, 4.4);
const BODY_CENTER: Vec3 = Vec3::new(0.0, 0.8, 0.0);

const WHEEL_RADIUS: f32 = 0.354;
const SUSPENSION_LENGTH: f32 = 0.3;
/// Height of the suspension mounts above the bottom of the car.
const MOUNT_HEIGHT: f32 = 0.62;

/// Below this speed, slip angles are measured as if going this fast, so
/// the tyres don't jitter when the car is almost stopped.
const LOW_SPEED: f32 = 3.0;

/// Brakes and rolling resistance fade out below this speed, so they stop
/// the car instead of pushing it back and forth.
const STOP_SPEED: f32 = 0.5;

/// How the car drives. Tuned in the tuning panel (F1) and saved to
/// tuning.ron.
#[derive(
	Resource, Reflect, InspectorOptions, Serialize, Deserialize, Clone,
)]
#[reflect(Resource, InspectorOptions)]
#[serde(default)]
pub struct CarTuning {
	/// Full-throttle force at low speed, in N.
	#[inspector(min = 0.0, max = 20_000.0)]
	pub drive_force: f32,
	/// Limits the drive force at speed, in W.
	#[inspector(min = 0.0, max = 1_000_000.0)]
	pub engine_power: f32,
	/// All four brakes together, in N.
	#[inspector(min = 0.0, max = 40_000.0)]
	pub brake_force: f32,
	/// Friction coefficient: the most force a tyre gives per newton of load.
	#[inspector(min = 0.1, max = 2.0)]
	pub tyre_grip: f32,
	/// Slip angle at which a tyre gives its full sideways grip, in degrees.
	#[inspector(min = 1.0, max = 20.0)]
	pub peak_slip_angle: f32,
	/// Front wheel angle at full lock, in degrees.
	#[inspector(min = 5.0, max = 45.0)]
	pub max_steer_angle: f32,
	/// How far behind the steering axis the front tyres' sideways force
	/// acts, in m. It turns into the torque that pulls the steering
	/// straight. The pneumatic part fades as a tyre nears its grip limit, so
	/// the steering goes light when the front starts to slide.
	#[inspector(min = 0.0, max = 0.1)]
	pub mechanical_trail: f32,
	#[inspector(min = 0.0, max = 0.1)]
	pub pneumatic_trail: f32,
	/// Per wheel, in N/m.
	#[inspector(min = 10_000.0, max = 150_000.0)]
	pub spring_stiffness: f32,
	/// Per wheel, in N s/m.
	#[inspector(min = 0.0, max = 15_000.0)]
	pub spring_damping: f32,
	/// Share of a tyre's load that slows it down while rolling.
	#[inspector(min = 0.0, max = 0.1)]
	pub rolling_resistance: f32,
	/// Air drag in N, per (m/s) squared.
	#[inspector(min = 0.0, max = 2.0)]
	pub air_drag: f32,
}

impl Default for CarTuning {
	fn default() -> Self {
		Self {
			drive_force: 8000.0,
			engine_power: 330_000.0,
			brake_force: 16_000.0,
			tyre_grip: 1.0,
			peak_slip_angle: 8.0,
			max_steer_angle: 35.0,
			mechanical_trail: 0.02,
			pneumatic_trail: 0.04,
			spring_stiffness: 46_000.0,
			spring_damping: 3300.0,
			rolling_resistance: 0.015,
			air_drag: 0.4,
		}
	}
}

struct Wheel {
	mount: Vec3,
	steered: bool,
	driven: bool,
}

/// Positions measured from the model's tyres.
const WHEELS: [Wheel; 4] = [
	Wheel {
		mount: Vec3::new(-0.775, MOUNT_HEIGHT, -1.218),
		steered: true,
		driven: false,
	},
	Wheel {
		mount: Vec3::new(0.775, MOUNT_HEIGHT, -1.218),
		steered: true,
		driven: false,
	},
	Wheel {
		mount: Vec3::new(-0.787, MOUNT_HEIGHT, 1.223),
		steered: false,
		driven: true,
	},
	Wheel {
		mount: Vec3::new(0.787, MOUNT_HEIGHT, 1.223),
		steered: false,
		driven: true,
	},
];
const DRIVEN_WHEELS: f32 = 2.0;

pub(super) struct CarPhysicsPlugin;

impl Plugin for CarPhysicsPlugin {
	fn build(&self, app: &mut App) {
		app.init_resource::<CarTuning>()
			.register_type::<CarTuning>()
			.add_systems(FixedUpdate, apply_wheel_forces);
	}
}

/// The torque the front tyres put on the steering, in Nm around the
/// steering axes. Positive turns the steering right. Force feedback plays
/// it on the wheel.
#[derive(Component, Default)]
pub struct SteeringTorque(pub f32);

pub(super) fn chassis() -> impl Bundle {
	(
		RigidBody::Dynamic,
		Mass(MASS),
		CenterOfMass(CENTER_OF_MASS),
		AngularInertia::new(ANGULAR_INERTIA),
		NoAutoMass,
		NoAutoCenterOfMass,
		NoAutoAngularInertia,
		TransformInterpolation,
		SteeringTorque::default(),
		// So the race can count hits.
		CollisionEventsEnabled,
		// On the chassis itself rather than a child, so the wheel rays can
		// skip it by excluding the chassis entity.
		Collider::compound(vec![(
			BODY_CENTER,
			Quat::IDENTITY,
			Collider::cuboid(BODY_SIZE.x, BODY_SIZE.y, BODY_SIZE.z),
		)]),
	)
}

fn apply_wheel_forces(
	tuning: Res<CarTuning>,
	spatial: SpatialQuery,
	mut cars: Query<
		(Entity, Forces, &CarInput, &mut SteeringTorque),
		With<Car>,
	>,
) {
	for (entity, mut forces, input, mut steering_torque) in &mut cars {
		let rotation = forces.rotation();
		let up = rotation * Vec3::Y;
		let filter = SpatialQueryFilter::from_excluded_entities([entity]);

		let forward_speed =
			forces.linear_velocity().dot(rotation * Vec3::NEG_Z);
		let drive = input.throttle * drive_force(&tuning, forward_speed);
		steering_torque.0 = 0.0;

		for wheel in &WHEELS {
			let mount = forces.position() + rotation * wheel.mount;
			let Some(hit) = spatial.cast_ray(
				mount,
				rotation * Dir3::NEG_Y,
				SUSPENSION_LENGTH + WHEEL_RADIUS,
				true,
				&filter,
			) else {
				continue;
			};

			let contact = mount - up * hit.distance;
			let velocity = forces.velocity_at_point(contact);
			let compression = SUSPENSION_LENGTH + WHEEL_RADIUS - hit.distance;
			let load = suspension_force(
				&tuning,
				compression,
				velocity.dot(hit.normal),
			);

			// Positive steer turns right, which is a negative turn around Y.
			let steer = if wheel.steered {
				-input.steer * tuning.max_steer_angle.to_radians()
			} else {
				0.0
			};
			let wheel_rotation = rotation * Quat::from_rotation_y(steer);
			let forward = wheel_rotation * Vec3::NEG_Z;
			let side = wheel_rotation * Vec3::X;
			let forward_velocity = velocity.dot(forward);

			let mut longitudinal = opposing(
				forward_velocity,
				input.brake * tuning.brake_force / WHEELS.len() as f32
					+ tuning.rolling_resistance * load,
			);
			if wheel.driven {
				longitudinal += drive / DRIVEN_WHEELS;
			}
			let slip_angle = slip_angle(velocity.dot(side), forward_velocity);
			let lateral = lateral_force(&tuning, slip_angle, load);
			let (longitudinal, lateral) =
				limit_to_grip(&tuning, longitudinal, lateral, load);

			forces.apply_force_at_point(
				// The ground pushes back along its normal, not the car's up,
				// so a tilted car isn't pushed sideways.
				hit.normal * load + forward * longitudinal + side * lateral,
				contact,
			);

			if wheel.steered {
				steering_torque.0 +=
					aligning_torque(&tuning, lateral, slip_angle);
			}
		}

		let velocity = forces.linear_velocity();
		forces.apply_force(-velocity * velocity.length() * tuning.air_drag);
	}
}

/// The engine's full-throttle drive force. Its power limits the force at
/// speed.
fn drive_force(tuning: &CarTuning, forward_speed: f32) -> f32 {
	tuning
		.drive_force
		.min(tuning.engine_power / forward_speed.max(1.0))
}

/// Spring and damper force pushing the chassis up. A spring can only push.
fn suspension_force(
	tuning: &CarTuning,
	compression: f32,
	up_velocity: f32,
) -> f32 {
	(compression * tuning.spring_stiffness
		- up_velocity * tuning.spring_damping)
		.max(0.0)
}

/// A friction force of up to `max` against `velocity`, fading out near
/// zero.
fn opposing(velocity: f32, max: f32) -> f32 {
	-max * (velocity / STOP_SPEED).clamp(-1.0, 1.0)
}

/// The angle between where a tyre points and where it is going.
fn slip_angle(side_velocity: f32, forward_velocity: f32) -> f32 {
	side_velocity.atan2(forward_velocity.abs().max(LOW_SPEED))
}

/// Sideways tyre force: grows with the slip angle up to the tyre's grip.
fn lateral_force(tuning: &CarTuning, slip_angle: f32, load: f32) -> f32 {
	let peak = tuning.peak_slip_angle.to_radians();

	-load * tuning.tyre_grip * (slip_angle / peak).clamp(-1.0, 1.0)
}

/// The torque a front tyre's sideways force puts on the steering. It
/// always turns the tyre back towards where the car is going.
fn aligning_torque(tuning: &CarTuning, lateral: f32, slip_angle: f32) -> f32 {
	let peak = tuning.peak_slip_angle.to_radians();
	let grip_used = (slip_angle.abs() / peak).min(1.0);
	let trail =
		tuning.mechanical_trail + tuning.pneumatic_trail * (1.0 - grip_used);

	-lateral * trail
}

/// A tyre can't give more total force than its grip, so scale both
/// directions down together when they ask for more.
fn limit_to_grip(
	tuning: &CarTuning,
	longitudinal: f32,
	lateral: f32,
	load: f32,
) -> (f32, f32) {
	let max = load * tuning.tyre_grip;
	let total = longitudinal.hypot(lateral);

	if total <= max {
		return (longitudinal, lateral);
	}

	let scale = max / total;
	(longitudinal * scale, lateral * scale)
}

#[cfg(test)]
mod tests {
	use std::time::Duration;

	use bevy::mesh::MeshPlugin;
	use bevy::time::TimeUpdateStrategy;
	use bevy::world_serialization::WorldSerializationPlugin;

	use super::*;

	fn input(throttle: f32, brake: f32) -> CarInput {
		CarInput {
			throttle,
			brake,
			steer: 0.0,
		}
	}

	#[test]
	fn engine_power_limits_drive_at_speed() {
		let tuning = CarTuning::default();

		assert_eq!(drive_force(&tuning, 0.0), tuning.drive_force);
		assert_eq!(drive_force(&tuning, 100.0), tuning.engine_power / 100.0);
	}

	#[test]
	fn spring_only_pushes() {
		let tuning = CarTuning::default();

		assert!(suspension_force(&tuning, 0.1, 0.0) > 0.0);
		assert_eq!(suspension_force(&tuning, 0.0, 1.0), 0.0);
	}

	#[test]
	fn tyres_resist_sliding_up_to_their_grip() {
		let tuning = CarTuning::default();
		let grip = 4000.0 * tuning.tyre_grip;

		assert!(lateral_force(&tuning, slip_angle(1.0, 20.0), 4000.0) < 0.0);
		assert_eq!(
			lateral_force(&tuning, slip_angle(-50.0, 20.0), 4000.0),
			grip
		);

		let (x, y) = limit_to_grip(&tuning, 3000.0, 4000.0, 2500.0);
		assert!((x.hypot(y) - 2500.0 * tuning.tyre_grip).abs() < 0.01);
	}

	#[test]
	fn steering_lightens_when_the_front_slides() {
		let tuning = CarTuning::default();
		let torque = |slip| {
			aligning_torque(&tuning, lateral_force(&tuning, slip, 4000.0), slip)
		};

		// Turning right: the tyre pushes right and the steering back left.
		assert!(torque(slip_angle(-1.0, 20.0)) < 0.0);

		let near_limit = tuning.peak_slip_angle.to_radians() * 0.6;
		let sliding = slip_angle(-15.0, 20.0);
		assert!(torque(sliding).abs() < torque(near_limit).abs());
	}

	/// Runs a car on flat ground for `seconds` with the given input, and
	/// returns where it ended up and how fast it is moving.
	fn drive(input: CarInput, seconds: f32) -> (Transform, Vec3) {
		let mut app = App::new();
		app.add_plugins((
			MinimalPlugins,
			TransformPlugin,
			// Avian's collider systems need these, even without meshes.
			AssetPlugin::default(),
			MeshPlugin,
			WorldSerializationPlugin,
			PhysicsPlugins::default(),
			CarPhysicsPlugin,
		))
		.insert_resource(TimeUpdateStrategy::ManualDuration(
			Duration::from_secs_f32(1.0 / 60.0),
		));

		app.world_mut()
			.spawn((RigidBody::Static, Collider::half_space(Vec3::Y)));
		let car = app
			.world_mut()
			.spawn((Car, input, chassis(), Transform::from_xyz(0.0, 0.1, 0.0)))
			.id();

		// `App::run` would do this; tests call `update` directly.
		app.finish();
		app.cleanup();
		for _ in 0..(seconds * 60.0) as u32 {
			app.update();
		}

		let world = app.world();
		(
			*world.get::<Transform>(car).unwrap(),
			world.get::<LinearVelocity>(car).unwrap().0,
		)
	}

	#[test]
	fn car_settles_on_its_wheels() {
		let (transform, velocity) = drive(input(0.0, 0.0), 3.0);

		assert!(transform.translation.y.abs() < 0.1, "{transform:?}");
		assert!(velocity.length() < 0.05, "{velocity}");
	}

	#[test]
	fn throttle_drives_forward_and_upright() {
		let (transform, velocity) = drive(input(1.0, 0.0), 3.0);

		assert!(transform.translation.z < -10.0, "{transform:?}");
		assert!(velocity.z < -5.0, "{velocity}");
		assert!(transform.up().y > 0.99, "{transform:?}");
	}
}
