use std::f32::consts::FRAC_PI_2;

use bevy::prelude::*;

const MAX_SPEED: f32 = 30.0;
const MAX_REVERSE_SPEED: f32 = 8.0;
const ACCELERATION: f32 = 8.0;
const REVERSE_ACCELERATION: f32 = 4.0;
const BRAKE_DECELERATION: f32 = 15.0;
const ROLLING_DRAG: f32 = 2.0;

const WHEELBASE: f32 = 2.6;
const MAX_STEER_ANGLE: f32 = 35.0_f32.to_radians();
/// Stands in for tyre grip: the car can't turn faster than this.
const MAX_YAW_RATE: f32 = 1.2;
/// How fast keyboard steering moves towards full lock, per second.
const KEYBOARD_STEER_RATE: f32 = 2.5;

/// How far the steering wheel turns at full lock (900° lock to lock).
const STEERING_WHEEL_LOCK: f32 = 450.0_f32.to_radians();
const STEERING_WHEEL_TILT: f32 = 20.0_f32.to_radians();

const DRIVER_HEAD: Vec3 = Vec3::new(-0.4, 1.35, 0.5);

pub struct CarPlugin;

impl Plugin for CarPlugin {
	fn build(&self, app: &mut App) {
		app.add_systems(Startup, spawn_car).add_systems(
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

#[derive(Component)]
struct SteeringWheel;

/// Where the driver's eyes are. The camera attaches here.
#[derive(Component)]
pub struct DriverHead;

fn spawn_car(
	mut commands: Commands,
	mut meshes: ResMut<Assets<Mesh>>,
	mut materials: ResMut<Assets<StandardMaterial>>,
) {
	let paint = materials.add(Color::srgb(0.7, 0.1, 0.1));
	let interior = materials.add(Color::srgb(0.15, 0.15, 0.15));

	let mut part =
		|size: Vec3, pos: Vec3, material: &Handle<StandardMaterial>| {
			(
				Mesh3d(meshes.add(Cuboid::from_size(size))),
				MeshMaterial3d(material.clone()),
				Transform::from_translation(pos),
			)
		};
	let tub = part(vec3(1.8, 0.4, 4.2), vec3(0.0, 0.4, 0.0), &paint);
	let hood = part(vec3(1.8, 0.25, 1.3), vec3(0.0, 0.725, -1.45), &paint);
	let left_door = part(vec3(0.1, 0.4, 2.0), vec3(-0.85, 0.8, 0.2), &paint);
	let right_door = part(vec3(0.1, 0.4, 2.0), vec3(0.85, 0.8, 0.2), &paint);
	let dashboard =
		part(vec3(1.7, 0.2, 0.35), vec3(0.0, 0.9, -0.65), &interior);
	let seat = part(vec3(0.5, 0.2, 0.5), vec3(-0.4, 0.7, 0.35), &interior);
	let seat_back =
		part(vec3(0.5, 0.6, 0.1), vec3(-0.4, 1.05, 0.65), &interior);
	let spoke = part(vec3(0.3, 0.03, 0.03), Vec3::ZERO, &interior);

	commands.spawn((
		Car::default(),
		CarInput::default(),
		Transform::default(),
		Visibility::default(),
		children![
			tub,
			hood,
			left_door,
			right_door,
			dashboard,
			seat,
			seat_back,
			(
				SteeringWheel,
				Mesh3d(meshes.add(Torus::new(0.15, 0.18))),
				MeshMaterial3d(interior.clone()),
				Transform::from_xyz(-0.4, 1.0, -0.3)
					.with_rotation(steering_wheel_rotation(0.0)),
				children![spoke],
			),
			(
				DriverHead,
				Transform::from_translation(DRIVER_HEAD),
				Visibility::default(),
			),
		],
	));
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

fn turn_steering_wheel(
	inputs: Query<&CarInput>,
	mut wheels: Query<(&mut Transform, &ChildOf), With<SteeringWheel>>,
) {
	for (mut transform, child_of) in &mut wheels {
		if let Ok(input) = inputs.get(child_of.parent()) {
			transform.rotation = steering_wheel_rotation(input.steer);
		}
	}
}

/// The torus lies flat, so stand it up facing the driver, tilted back,
/// then spin it around its own axis. Clockwise (as the driver sees it)
/// for positive steer.
fn steering_wheel_rotation(steer: f32) -> Quat {
	let face_driver = Quat::from_rotation_x(FRAC_PI_2 - STEERING_WHEEL_TILT);
	let spin = Quat::from_rotation_y(-steer * STEERING_WHEEL_LOCK);

	face_driver * spin
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
