//! Displays in the cockpit: a small head-up display with the speed, and
//! hits and overtakes on the infotainment screen. Each one is UI drawn into
//! an image that is shown on a panel in the car, so it also works in VR.

use avian3d::prelude::*;
use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

use crate::car::{Car, THRUSTMASTER};
use crate::race::{RaceState, Score};

struct Panel {
	/// In car space.
	center: Vec3,
	/// In metres.
	size: Vec2,
	/// How far the top leans away from the driver.
	tilt: f32,
	pixels: UVec2,
	background: Color,
	font_size: f32,
	text_color: Color,
}

/// Measured on the model by casting rays from the driver's eyes, then
/// moved 3 mm towards the driver so it covers the screen.
const INFO_SCREEN: Panel = Panel {
	center: Vec3::new(0.002, 0.772, -0.389),
	size: Vec2::new(0.128, 0.075),
	tilt: 16.4_f32.to_radians(),
	pixels: UVec2::new(512, 298),
	background: Color::srgb(0.02, 0.03, 0.06),
	font_size: 52.0,
	text_color: Color::WHITE,
};

/// Low in the driver's view: above the steering wheel rim, inside the
/// windscreen.
const HEAD_UP: Panel = Panel {
	center: Vec3::new(-0.335, 1.06, -0.5),
	size: Vec2::new(0.22, 0.04),
	tilt: 0.0,
	pixels: UVec2::new(880, 160),
	background: Color::NONE,
	font_size: 88.0,
	text_color: Color::srgb(0.6, 1.0, 0.8),
};

pub struct HudPlugin;

impl Plugin for HudPlugin {
	fn build(&self, app: &mut App) {
		app.add_observer(spawn_displays)
			.add_systems(Update, (update_info_screen, update_head_up));
	}
}

#[derive(Component)]
struct InfoText;

#[derive(Component)]
struct HeadUpText;

fn spawn_displays(
	add: On<Add<Car>>,
	mut commands: Commands,
	mut images: ResMut<Assets<Image>>,
	mut meshes: ResMut<Assets<Mesh>>,
	mut materials: ResMut<Assets<StandardMaterial>>,
) {
	for (panel, info) in [(&INFO_SCREEN, true), (&HEAD_UP, false)] {
		let text = spawn_panel(
			&mut commands,
			&mut images,
			&mut meshes,
			&mut materials,
			panel,
			add.entity,
		);

		if info {
			commands.entity(text).insert(InfoText);
		} else {
			commands.entity(text).insert(HeadUpText);
		}
	}
}

/// A panel in the car showing text, which is drawn into an image by its own
/// UI camera. Returns the text entity.
fn spawn_panel(
	commands: &mut Commands,
	images: &mut Assets<Image>,
	meshes: &mut Assets<Mesh>,
	materials: &mut Assets<StandardMaterial>,
	panel: &Panel,
	car: Entity,
) -> Entity {
	let image = images.add(Image::new_target_texture(
		panel.pixels.x,
		panel.pixels.y,
		TextureFormat::Bgra8UnormSrgb,
		None,
	));
	let camera = commands
		.spawn((
			Camera2d,
			Camera {
				// Draw before the main camera, which shows the image.
				order: -1,
				clear_color: ClearColorConfig::Custom(panel.background),
				..default()
			},
			RenderTarget::Image(image.clone().into()),
		))
		.id();

	let alpha_mode = if panel.background.alpha() < 1.0 {
		AlphaMode::Blend
	} else {
		AlphaMode::Opaque
	};
	// A rectangle faces +Z, towards the driver.
	commands.spawn((
		Mesh3d(meshes.add(Rectangle::from_size(panel.size))),
		MeshMaterial3d(materials.add(StandardMaterial {
			base_color_texture: Some(image),
			unlit: true,
			alpha_mode,
			..default()
		})),
		Transform::from_translation(panel.center)
			.with_rotation(Quat::from_rotation_x(-panel.tilt)),
		ChildOf(car),
	));

	let text = commands
		.spawn((
			Text::default(),
			TextFont {
				font_size: FontSize::Px(panel.font_size),
				..default()
			},
			TextColor(panel.text_color),
			// Keeps the head-up display readable against bright cars.
			TextShadow {
				offset: Vec2::splat(3.0),
				color: Color::srgba(0.0, 0.0, 0.0, 0.8),
			},
			TextLayout::justify(Justify::Center),
		))
		.id();

	commands
		.spawn((
			Node {
				width: percent(100),
				height: percent(100),
				justify_content: JustifyContent::Center,
				align_items: AlignItems::Center,
				..default()
			},
			UiTargetCamera(camera),
		))
		.add_child(text);

	text
}

fn update_info_screen(
	mut text: Single<&mut Text, With<InfoText>>,
	score: Res<Score>,
) {
	text.0 = format!("Hits: {}\nPassed: {}", score.hits, score.overtakes);
}

fn update_head_up(
	mut text: Single<&mut Text, With<HeadUpText>>,
	velocity: Single<&LinearVelocity, With<Car>>,
	state: Res<State<RaceState>>,
	gamepads: Query<&Gamepad>,
) {
	let wheel = gamepads
		.iter()
		.any(|gamepad| gamepad.vendor_id() == Some(THRUSTMASTER));

	text.0 = if *state.get() == RaceState::Grid && wheel {
		"Gas to start".to_string()
	} else if *state.get() == RaceState::Grid {
		"Press W to start".to_string()
	} else {
		format!("{:.0} km/h", velocity.length() * 3.6)
	};
}
