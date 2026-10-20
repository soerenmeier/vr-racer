use std::f32::consts::PI;

use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;

const MAX_SPEED: f32 = 30.0;
const MAX_REVERSE_SPEED: f32 = 8.0;
const ACCELERATION: f32 = 8.0;
const REVERSE_ACCELERATION: f32 = 4.0;
const BRAKE_DECELERATION: f32 = 15.0;
const ROLLING_DRAG: f32 = 2.0;

const WHEELBASE: f32 = 2.44;
const MAX_STEER_ANGLE: f32 = 35.0_f32.to_radians();
/// Stands in for tyre grip: the car can't turn faster than this.
const MAX_YAW_RATE: f32 = 1.2;
/// How fast keyboard steering moves towards full lock, per second.
const KEYBOARD_STEER_RATE: f32 = 2.5;

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

const DRIVER_HEAD: Vec3 = Vec3::new(-0.335, 1.1, 0.4);

pub struct CarPlugin;

impl Plugin for CarPlugin {
	fn build(&self, app: &mut App) {
		app.add_systems(Startup, spawn_car)
			.add_observer(find_steering_wheel)
			.add_systems(
				Update,
				(read_keyboard, update_speed, move_car, turn_steering_wheel)
					.chain(),
			);
	}
}

#[derive(Component, Default)]
struct Car {
	/// Forward speed in m/s, negative when reversing.
	speed: f32,
}

/// What the driver is doing. Throttle and brake go from 0 to 1, steer
/// from -1 (full left) to 1 (full right).
#[derive(Component, Default)]
struct CarInput {
	throttle: f32,
	brake: f32,
	steer: f32,
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
		Car::default(),
		CarInput::default(),
		Transform::default(),
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

fn read_keyboard(
	keys: Res<ButtonInput<KeyCode>>,
	time: Res<Time>,
	mut inputs: Query<&mut CarInput>,
) {
	let pressed = |key| if keys.pressed(key) { 1.0 } else { 0.0 };
	let target_steer = pressed(KeyCode::KeyD) - pressed(KeyCode::KeyA);
	let step = KEYBOARD_STEER_RATE * time.delta_secs();

	for mut input in &mut inputs {
		input.throttle = pressed(KeyCode::KeyW);
		input.brake = pressed(KeyCode::KeyS);
		input.steer += (target_steer - input.steer).clamp(-step, step);
	}
}

fn update_speed(time: Res<Time>, mut cars: Query<(&mut Car, &CarInput)>) {
	for (mut car, input) in &mut cars {
		car.speed = next_speed(car.speed, input, time.delta_secs());
	}
}

fn move_car(
	time: Res<Time>,
	mut cars: Query<(&mut Transform, &Car, &CarInput)>,
) {
	let dt = time.delta_secs();

	for (mut transform, car, input) in &mut cars {
		// Bicycle model: the turn rate grows with speed and steer angle.
		let steer_angle = input.steer * MAX_STEER_ANGLE;
		let yaw_rate = (car.speed * steer_angle.tan() / WHEELBASE)
			.clamp(-MAX_YAW_RATE, MAX_YAW_RATE);

		// Positive rotation around Y turns left, positive steer is right.
		transform.rotate_y(-yaw_rate * dt);

		let forward = transform.forward();
		transform.translation += forward * car.speed * dt;
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

fn next_speed(speed: f32, input: &CarInput, dt: f32) -> f32 {
	// The brake pedal slows the car down, and reverses once it has stopped.
	let brake = if speed > 0.0 {
		BRAKE_DECELERATION
	} else {
		REVERSE_ACCELERATION
	};
	let speed =
		speed + (input.throttle * ACCELERATION - input.brake * brake) * dt;

	// Drag always slows the car down, but never makes it go backwards.
	let drag = ROLLING_DRAG * dt;
	let speed = if speed.abs() <= drag {
		0.0
	} else {
		speed - drag * speed.signum()
	};

	speed.clamp(-MAX_REVERSE_SPEED, MAX_SPEED)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn input(throttle: f32, brake: f32) -> CarInput {
		CarInput {
			throttle,
			brake,
			steer: 0.0,
		}
	}

	#[test]
	fn throttle_accelerates() {
		assert!(next_speed(0.0, &input(1.0, 0.0), 0.1) > 0.0);
	}

	#[test]
	fn coasting_stops_without_reversing() {
		let mut speed = 1.0;
		for _ in 0..100 {
			speed = next_speed(speed, &input(0.0, 0.0), 0.1);
		}

		assert_eq!(speed, 0.0);
	}

	#[test]
	fn brake_reverses_once_stopped() {
		let mut speed = 5.0;
		for _ in 0..100 {
			speed = next_speed(speed, &input(0.0, 1.0), 0.1);
		}

		assert_eq!(speed, -MAX_REVERSE_SPEED);
	}

	#[test]
	fn speed_is_capped() {
		let mut speed = 0.0;
		for _ in 0..1000 {
			speed = next_speed(speed, &input(1.0, 0.0), 0.1);
		}

		assert_eq!(speed, MAX_SPEED);
	}
}
