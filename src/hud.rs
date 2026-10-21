//! Speed and score in the corner of the screen.

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::car::Car;
use crate::race::{RaceState, Score};

const FONT_SIZE: f32 = 28.0;
const MARGIN: f32 = 16.0;
const BACKGROUND: Color = Color::srgba(0.0, 0.0, 0.0, 0.5);

pub struct HudPlugin;

impl Plugin for HudPlugin {
	fn build(&self, app: &mut App) {
		app.add_systems(Startup, spawn_hud)
			.add_systems(Update, update_hud);
	}
}

#[derive(Component)]
struct HudText;

fn spawn_hud(mut commands: Commands) {
	commands.spawn((
		HudText,
		Text::default(),
		TextFont {
			font_size: FontSize::Px(FONT_SIZE),
			..default()
		},
		Node {
			position_type: PositionType::Absolute,
			top: px(MARGIN),
			left: px(MARGIN),
			padding: UiRect::all(px(MARGIN / 2.0)),
			..default()
		},
		BackgroundColor(BACKGROUND),
	));
}

fn update_hud(
	mut text: Single<&mut Text, With<HudText>>,
	velocity: Single<&LinearVelocity, With<Car>>,
	score: Res<Score>,
	state: Res<State<RaceState>>,
) {
	let speed = velocity.length() * 3.6;
	let mut hud = format!(
		"{speed:.0} km/h\nHits: {}\nOvertakes: {}",
		score.hits, score.overtakes
	);

	if *state.get() == RaceState::Grid {
		hud.push_str("\n\nPress W to start");
	}

	text.0 = hud;
}
