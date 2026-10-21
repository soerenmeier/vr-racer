//! A long, straight, three-lane highway heading towards -Z.

use avian3d::prelude::*;
use bevy::prelude::*;

pub const ROAD_LENGTH: f32 = 20_000.0;
/// How far the road reaches behind the start.
const ROAD_BEHIND_START: f32 = 100.0;
pub const LANES: usize = 3;
const LANE_WIDTH: f32 = 3.75;
const SHOULDER_WIDTH: f32 = 1.5;
const ROAD_WIDTH: f32 = LANES as f32 * LANE_WIDTH + 2.0 * SHOULDER_WIDTH;
const GROUND_WIDTH: f32 = 1000.0;

const LINE_WIDTH: f32 = 0.15;
const DASH_LENGTH: f32 = 6.0;
const DASH_GAP: f32 = 12.0;
const RAIL_HEIGHT: f32 = 0.75;
const RAIL_THICKNESS: f32 = 0.3;
const POST_SPACING: f32 = 50.0;
const POST_SIZE: Vec3 = Vec3::new(0.12, 1.0, 0.12);

const SKY_COLOR: Color = Color::srgb(0.55, 0.75, 0.95);
const GRASS_COLOR: Color = Color::srgb(0.25, 0.4, 0.2);
const ASPHALT_COLOR: Color = Color::srgb(0.18, 0.18, 0.2);
const LINE_COLOR: Color = Color::srgb(0.9, 0.9, 0.9);
const RAIL_COLOR: Color = Color::srgb(0.6, 0.62, 0.65);

#[derive(Component)]
pub struct GuardRail;

/// Lane 0 is the right lane, the highest is the left (overtaking) lane.
pub fn lane_x(lane: usize) -> f32 {
	(lane as f32 - (LANES - 1) as f32 / 2.0) * -LANE_WIDTH
}

pub struct HighwayPlugin;

impl Plugin for HighwayPlugin {
	fn build(&self, app: &mut App) {
		app.insert_resource(ClearColor(SKY_COLOR)).add_systems(
			Startup,
			(spawn_ground_and_road, spawn_lines, spawn_rails, spawn_sun),
		);
	}
}

/// A box that runs along the whole road.
fn along_road(width: f32, height: f32, x: f32, y: f32) -> (Cuboid, Transform) {
	let length = ROAD_LENGTH + ROAD_BEHIND_START;
	let z = (ROAD_BEHIND_START - ROAD_LENGTH) / 2.0;

	(
		Cuboid::new(width, height, length),
		Transform::from_xyz(x, y, z),
	)
}

fn spawn_ground_and_road(
	mut commands: Commands,
	mut meshes: ResMut<Assets<Mesh>>,
	mut materials: ResMut<Assets<StandardMaterial>>,
) {
	let (ground, ground_transform) = along_road(GROUND_WIDTH, 0.0, 0.0, 0.0);
	commands.spawn((
		RigidBody::Static,
		Collider::half_space(Vec3::Y),
		Mesh3d(meshes.add(ground)),
		MeshMaterial3d(materials.add(GRASS_COLOR)),
		ground_transform,
	));

	// Flat boxes raised a little, so they don't flicker against the ground.
	let (road, road_transform) = along_road(ROAD_WIDTH, 0.0, 0.0, 0.01);
	commands.spawn((
		Mesh3d(meshes.add(road)),
		MeshMaterial3d(materials.add(ASPHALT_COLOR)),
		road_transform,
	));
}

fn spawn_lines(
	mut commands: Commands,
	mut meshes: ResMut<Assets<Mesh>>,
	mut materials: ResMut<Assets<StandardMaterial>>,
) {
	let material = materials.add(LINE_COLOR);

	let edge = LANES as f32 * LANE_WIDTH / 2.0;
	for x in [-edge, edge] {
		let (line, transform) = along_road(LINE_WIDTH, 0.0, x, 0.02);
		commands.spawn((
			Mesh3d(meshes.add(line)),
			MeshMaterial3d(material.clone()),
			transform,
		));
	}

	let dash = meshes.add(Cuboid::new(LINE_WIDTH, 0.0, DASH_LENGTH));
	let dashes = (ROAD_LENGTH / (DASH_LENGTH + DASH_GAP)) as usize;
	for lane in 1..LANES {
		let x = (lane_x(lane - 1) + lane_x(lane)) / 2.0;
		for i in 0..dashes {
			let z = -(i as f32) * (DASH_LENGTH + DASH_GAP);
			commands.spawn((
				Mesh3d(dash.clone()),
				MeshMaterial3d(material.clone()),
				Transform::from_xyz(x, 0.02, z),
			));
		}
	}
}

/// Guard rails on both sides, with reflector posts behind them so you can
/// see how fast you are going.
fn spawn_rails(
	mut commands: Commands,
	mut meshes: ResMut<Assets<Mesh>>,
	mut materials: ResMut<Assets<StandardMaterial>>,
) {
	let rail_material = materials.add(RAIL_COLOR);
	let post = meshes.add(Cuboid::from_size(POST_SIZE));
	let post_material = materials.add(Color::WHITE);
	let posts = (ROAD_LENGTH / POST_SPACING) as usize;

	for side in [-1.0, 1.0] {
		let x = side * (ROAD_WIDTH / 2.0 + RAIL_THICKNESS / 2.0);
		let (rail, transform) =
			along_road(RAIL_THICKNESS, RAIL_HEIGHT, x, RAIL_HEIGHT / 2.0);
		commands.spawn((
			GuardRail,
			RigidBody::Static,
			Collider::cuboid(rail.size().x, rail.size().y, rail.size().z),
			Mesh3d(meshes.add(rail)),
			MeshMaterial3d(rail_material.clone()),
			transform,
		));

		let post_x = x + side * 1.0;
		for i in 0..posts {
			commands.spawn((
				Mesh3d(post.clone()),
				MeshMaterial3d(post_material.clone()),
				Transform::from_xyz(
					post_x,
					POST_SIZE.y / 2.0,
					-(i as f32) * POST_SPACING,
				),
			));
		}
	}
}

fn spawn_sun(mut commands: Commands) {
	commands.spawn((
		DirectionalLight {
			shadow_maps_enabled: true,
			..default()
		},
		Transform::from_xyz(4.0, 10.0, 2.0).looking_at(Vec3::ZERO, Vec3::Y),
	));
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn lanes_run_right_to_left() {
		assert_eq!(lane_x(0), LANE_WIDTH);
		assert_eq!(lane_x(1), 0.0);
		assert_eq!(lane_x(2), -LANE_WIDTH);
	}
}
