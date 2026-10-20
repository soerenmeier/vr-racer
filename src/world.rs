use bevy::prelude::*;

const GROUND_SIZE: f32 = 400.0;
const MARKER_SPACING: f32 = 10.0;
const MARKERS_PER_SIDE: i32 = 20;
const SKY_COLOR: Color = Color::srgb(0.55, 0.75, 0.95);

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
	fn build(&self, app: &mut App) {
		app.insert_resource(ClearColor(SKY_COLOR))
			.add_systems(Startup, (spawn_ground, spawn_markers, spawn_sun));
	}
}

fn spawn_ground(
	mut commands: Commands,
	mut meshes: ResMut<Assets<Mesh>>,
	mut materials: ResMut<Assets<StandardMaterial>>,
) {
	commands.spawn((
		Mesh3d(
			meshes
				.add(Plane3d::default().mesh().size(GROUND_SIZE, GROUND_SIZE)),
		),
		MeshMaterial3d(materials.add(Color::srgb(0.3, 0.32, 0.3))),
	));
}

/// Posts in a grid on the ground, so you can see that you are moving.
fn spawn_markers(
	mut commands: Commands,
	mut meshes: ResMut<Assets<Mesh>>,
	mut materials: ResMut<Assets<StandardMaterial>>,
) {
	let mesh = meshes.add(Cuboid::new(0.3, 1.0, 0.3));
	let material = materials.add(Color::srgb(0.95, 0.6, 0.1));

	// Offset by half a spacing so no post stands at the origin, where the
	// car spawns.
	let start = -(MARKERS_PER_SIDE as f32 - 1.0) * MARKER_SPACING / 2.0;
	for x in 0..MARKERS_PER_SIDE {
		for z in 0..MARKERS_PER_SIDE {
			commands.spawn((
				Mesh3d(mesh.clone()),
				MeshMaterial3d(material.clone()),
				Transform::from_xyz(
					start + x as f32 * MARKER_SPACING,
					0.5,
					start + z as f32 * MARKER_SPACING,
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
