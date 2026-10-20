//! Raycast vehicle: the chassis is one rigid body, and each wheel casts a
//! ray to the ground and pushes on the chassis with suspension and tyre
//! forces at its contact point.

use avian3d::prelude::*;
use bevy::prelude::*;

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
const SPRING_STIFFNESS: f32 = 46_000.0;
const SPRING_DAMPING: f32 = 3300.0;
/// Height of the suspension mounts above the bottom of the car.
const MOUNT_HEIGHT: f32 = 0.62;

const MAX_STEER_ANGLE: f32 = 35.0_f32.to_radians();
/// Friction coefficient: the most force a tyre gives per newton of load.
const TYRE_GRIP: f32 = 1.0;
/// Slip angle at which a tyre gives its full sideways grip.
const PEAK_SLIP_ANGLE: f32 = 8.0_f32.to_radians();
/// Below this speed, slip angles are measured as if going this fast, so
/// the tyres don't jitter when the car is almost stopped.
const LOW_SPEED: f32 = 3.0;

const DRIVE_FORCE: f32 = 8000.0;
const ENGINE_POWER: f32 = 330_000.0;
const REVERSE_FORCE: f32 = 3000.0;
/// Below this forward speed, the brake pedal reverses instead.
const REVERSE_BELOW_SPEED: f32 = 1.0;
const BRAKE_FORCE: f32 = 16_000.0;
const ROLLING_RESISTANCE: f32 = 0.015;
/// Brakes and rolling resistance fade out below this speed, so they stop
/// the car instead of pushing it back and forth.
const STOP_SPEED: f32 = 0.5;
const AIR_DRAG: f32 = 0.4;

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
		app.add_systems(FixedUpdate, apply_wheel_forces);
	}
}

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
	spatial: SpatialQuery,
	mut cars: Query<(Entity, Forces, &CarInput), With<Car>>,
) {
	for (entity, mut forces, input) in &mut cars {
		let rotation = forces.rotation();
		let up = rotation * Vec3::Y;
		let filter = SpatialQueryFilter::from_excluded_entities([entity]);

		let forward_speed =
			forces.linear_velocity().dot(rotation * Vec3::NEG_Z);
		let (drive, brake) = pedals(input, forward_speed);

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
			let load = suspension_force(compression, velocity.dot(hit.normal));

			// Positive steer turns right, which is a negative turn around Y.
			let steer = if wheel.steered {
				-input.steer * MAX_STEER_ANGLE
			} else {
				0.0
			};
			let wheel_rotation = rotation * Quat::from_rotation_y(steer);
			let forward = wheel_rotation * Vec3::NEG_Z;
			let side = wheel_rotation * Vec3::X;
			let forward_velocity = velocity.dot(forward);

			let mut longitudinal = opposing(
				forward_velocity,
				brake * BRAKE_FORCE / WHEELS.len() as f32
					+ ROLLING_RESISTANCE * load,
			);
			if wheel.driven {
				longitudinal += drive / DRIVEN_WHEELS;
			}
			let lateral =
				lateral_force(velocity.dot(side), forward_velocity, load);
			let (longitudinal, lateral) =
				limit_to_grip(longitudinal, lateral, load);

			forces.apply_force_at_point(
				// The ground pushes back along its normal, not the car's up,
				// so a tilted car isn't pushed sideways.
				hit.normal * load + forward * longitudinal + side * lateral,
				contact,
			);
		}

		let velocity = forces.linear_velocity();
		forces.apply_force(-velocity * velocity.length() * AIR_DRAG);
	}
}

/// Turns the pedals into a total drive force and a brake amount (0 to 1).
/// The brake pedal reverses once the car has almost stopped.
fn pedals(input: &CarInput, forward_speed: f32) -> (f32, f32) {
	if input.brake > 0.0 && forward_speed < REVERSE_BELOW_SPEED {
		return (-input.brake * REVERSE_FORCE, 0.0);
	}

	// The engine's power limits the force at speed.
	let drive = DRIVE_FORCE.min(ENGINE_POWER / forward_speed.max(1.0));

	(input.throttle * drive, input.brake)
}

/// Spring and damper force pushing the chassis up. A spring can only push.
fn suspension_force(compression: f32, up_velocity: f32) -> f32 {
	(compression * SPRING_STIFFNESS - up_velocity * SPRING_DAMPING).max(0.0)
}

/// A friction force of up to `max` against `velocity`, fading out near
/// zero.
fn opposing(velocity: f32, max: f32) -> f32 {
	-max * (velocity / STOP_SPEED).clamp(-1.0, 1.0)
}

/// Sideways tyre force: grows with the slip angle up to the tyre's grip.
fn lateral_force(side_velocity: f32, forward_velocity: f32, load: f32) -> f32 {
	let slip_angle = side_velocity.atan2(forward_velocity.abs().max(LOW_SPEED));

	-load * TYRE_GRIP * (slip_angle / PEAK_SLIP_ANGLE).clamp(-1.0, 1.0)
}

/// A tyre can't give more total force than its grip, so scale both
/// directions down together when they ask for more.
fn limit_to_grip(longitudinal: f32, lateral: f32, load: f32) -> (f32, f32) {
	let max = load * TYRE_GRIP;
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
	fn brake_reverses_only_when_stopped() {
		assert_eq!(pedals(&input(0.0, 1.0), 10.0), (0.0, 1.0));
		assert_eq!(pedals(&input(0.0, 1.0), 0.0), (-REVERSE_FORCE, 0.0));
	}

	#[test]
	fn spring_only_pushes() {
		assert!(suspension_force(0.1, 0.0) > 0.0);
		assert_eq!(suspension_force(0.0, 1.0), 0.0);
	}

	#[test]
	fn tyres_resist_sliding_up_to_their_grip() {
		assert!(lateral_force(1.0, 20.0, 4000.0) < 0.0);
		assert_eq!(lateral_force(-50.0, 20.0, 4000.0), 4000.0 * TYRE_GRIP);

		let (x, y) = limit_to_grip(3000.0, 4000.0, 2500.0);
		assert!((x.hypot(y) - 2500.0 * TYRE_GRIP).abs() < 0.01);
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
