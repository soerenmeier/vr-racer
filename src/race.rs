//! The race: traffic waits on the grid until the player sets off, then
//! hits and overtakes are counted.

use avian3d::prelude::*;
use bevy::ecs::entity::EntityHashMap;
use bevy::prelude::*;

use crate::car::{Car, CarInput};
use crate::highway::GuardRail;
use crate::traffic::TrafficCar;

/// Things that count as a hit when the player runs into them.
type Obstacle = Or<(With<TrafficCar>, With<GuardRail>)>;

pub struct RacePlugin;

impl Plugin for RacePlugin {
	fn build(&self, app: &mut App) {
		app.init_state::<RaceState>()
			.init_resource::<Score>()
			.add_systems(Update, start_race.run_if(in_state(RaceState::Grid)))
			.add_systems(
				FixedUpdate,
				(count_hits, count_overtakes)
					.run_if(in_state(RaceState::Racing)),
			);
	}
}

#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum RaceState {
	/// Everyone waits until the player presses the throttle.
	#[default]
	Grid,
	Racing,
}

#[derive(Resource, Default)]
pub struct Score {
	pub hits: u32,
	/// Cars passed, minus cars that passed the player back.
	pub overtakes: i32,
}

fn start_race(
	input: Single<&CarInput, With<Car>>,
	mut next_state: ResMut<NextState<RaceState>>,
) {
	if input.throttle > 0.0 {
		next_state.set(RaceState::Racing);
	}
}

/// One hit for each new contact with a traffic car or guard rail, so
/// scraping along a rail counts once.
fn count_hits(
	mut collisions: MessageReader<CollisionStart>,
	player: Single<Entity, With<Car>>,
	obstacles: Query<(), Obstacle>,
	mut score: ResMut<Score>,
) {
	for collision in collisions.read() {
		let (Some(body1), Some(body2)) = (collision.body1, collision.body2)
		else {
			continue;
		};

		let other = if body1 == *player {
			body2
		} else if body2 == *player {
			body1
		} else {
			continue;
		};

		if obstacles.contains(other) {
			score.hits += 1;
		}
	}
}

/// Counts cars moving from ahead of the player to behind, and back.
fn count_overtakes(
	player: Single<&Position, With<Car>>,
	cars: Query<(Entity, &Position), With<TrafficCar>>,
	mut was_ahead: Local<EntityHashMap<bool>>,
	mut score: ResMut<Score>,
) {
	for (entity, position) in &cars {
		// The road heads towards -Z.
		let ahead = position.z < player.z;

		match was_ahead.insert(entity, ahead) {
			Some(true) if !ahead => score.overtakes += 1,
			Some(false) if ahead => score.overtakes -= 1,
			_ => {}
		}
	}
}
