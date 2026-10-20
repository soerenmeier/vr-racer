//! Plays a script of timed inputs and screenshots, so the game can be
//! tested without a person at the keyboard. Run with
//! `cargo run --features dev -- --script scripts/drive.txt`.
//!
//! Each line is `<seconds> <command>`:
//! - `press <key>` / `release <key>` (keys: W, A, S, D)
//! - `screenshot <name>` saves `screenshots/<name>.png`
//! - `record <name> <seconds> <fps>` saves `screenshots/<name>-000.png`, ...
//! - `exit`
//!
//! Time advances by exactly one 60 Hz frame per update, so runs are
//! repeatable no matter how fast the machine renders.

use std::fs;
use std::time::Duration;

use bevy::input::InputSystems;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::time::TimeUpdateStrategy;

const OUTPUT_DIR: &str = "screenshots";
const FRAME_TIME: Duration = Duration::from_nanos(1_000_000_000 / 60);

pub struct ScriptPlugin;

impl Plugin for ScriptPlugin {
	fn build(&self, app: &mut App) {
		let Some(path) =
			std::env::args().skip_while(|a| a != "--script").nth(1)
		else {
			return;
		};

		let text = fs::read_to_string(&path).expect("can't read script");
		let steps = parse(&text).expect("invalid script");

		// Start from an empty folder so old frames don't mix with new ones.
		let _ = fs::remove_dir_all(OUTPUT_DIR);
		fs::create_dir_all(OUTPUT_DIR).expect("can't create output folder");

		app.insert_resource(Script {
			steps,
			next: 0,
			recording: None,
		})
		.insert_resource(TimeUpdateStrategy::ManualDuration(FRAME_TIME))
		.add_systems(
			PreUpdate,
			(run_steps, record_frames).chain().after(InputSystems),
		);
	}
}

#[derive(Resource)]
struct Script {
	steps: Vec<Step>,
	next: usize,
	recording: Option<Recording>,
}

#[derive(Debug, PartialEq)]
struct Step {
	at: f32,
	command: Command,
}

#[derive(Debug, PartialEq, Clone)]
enum Command {
	Press(KeyCode),
	Release(KeyCode),
	Screenshot(String),
	Record {
		name: String,
		seconds: f32,
		fps: f32,
	},
	Exit,
}

struct Recording {
	name: String,
	until: f32,
	interval: f32,
	next_at: f32,
	frame: u32,
}

fn run_steps(
	time: Res<Time>,
	mut script: ResMut<Script>,
	mut keys: ResMut<ButtonInput<KeyCode>>,
	mut commands: Commands,
	mut exit: MessageWriter<AppExit>,
) {
	let now = time.elapsed_secs();

	while let Some(step) = script.steps.get(script.next)
		&& step.at <= now
	{
		let command = step.command.clone();
		script.next += 1;

		match command {
			Command::Press(key) => keys.press(key),
			Command::Release(key) => keys.release(key),
			Command::Screenshot(name) => screenshot(&mut commands, &name),
			Command::Record { name, seconds, fps } => {
				script.recording = Some(Recording {
					name,
					until: now + seconds,
					interval: 1.0 / fps,
					next_at: now,
					frame: 0,
				});
			}
			Command::Exit => {
				exit.write(AppExit::Success);
			}
		}
	}
}

fn record_frames(
	time: Res<Time>,
	mut script: ResMut<Script>,
	mut commands: Commands,
) {
	let now = time.elapsed_secs();
	let Some(recording) = &mut script.recording else {
		return;
	};

	if now > recording.until {
		script.recording = None;
		return;
	}

	if now >= recording.next_at {
		let name = format!("{}-{:03}", recording.name, recording.frame);
		screenshot(&mut commands, &name);

		recording.frame += 1;
		recording.next_at += recording.interval;
	}
}

fn screenshot(commands: &mut Commands, name: &str) {
	commands
		.spawn(Screenshot::primary_window())
		.observe(save_to_disk(format!("{OUTPUT_DIR}/{name}.png")));
}

fn parse(text: &str) -> Result<Vec<Step>, String> {
	let mut steps = Vec::new();

	for (i, line) in text.lines().enumerate() {
		let line = line.trim();
		if line.is_empty() || line.starts_with('#') {
			continue;
		}

		let words: Vec<&str> = line.split_whitespace().collect();
		let error = || format!("line {}: can't understand `{line}`", i + 1);
		let number = |word: &str| word.parse::<f32>().map_err(|_| error());

		let command = match words[1..] {
			["press", key] => Command::Press(parse_key(key).ok_or_else(error)?),
			["release", key] => {
				Command::Release(parse_key(key).ok_or_else(error)?)
			}
			["screenshot", name] => Command::Screenshot(name.to_string()),
			["record", name, seconds, fps] => Command::Record {
				name: name.to_string(),
				seconds: number(seconds)?,
				fps: number(fps)?,
			},
			["exit"] => Command::Exit,
			_ => return Err(error()),
		};

		steps.push(Step {
			at: number(words[0])?,
			command,
		});
	}

	// Steps run in time order, whatever order the lines are in.
	steps.sort_by(|a, b| a.at.total_cmp(&b.at));

	Ok(steps)
}

fn parse_key(name: &str) -> Option<KeyCode> {
	match name {
		"W" => Some(KeyCode::KeyW),
		"A" => Some(KeyCode::KeyA),
		"S" => Some(KeyCode::KeyS),
		"D" => Some(KeyCode::KeyD),
		_ => None,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parses_commands_and_skips_comments() {
		let steps =
			parse("# comment\n\n0.5 press W\n2 record turn 1.5 10\n3 exit")
				.unwrap();

		assert_eq!(
			steps,
			vec![
				Step {
					at: 0.5,
					command: Command::Press(KeyCode::KeyW),
				},
				Step {
					at: 2.0,
					command: Command::Record {
						name: "turn".to_string(),
						seconds: 1.5,
						fps: 10.0,
					},
				},
				Step {
					at: 3.0,
					command: Command::Exit,
				},
			]
		);
	}

	#[test]
	fn rejects_unknown_lines() {
		assert!(parse("1 jump").is_err());
		assert!(parse("1 press X").is_err());
		assert!(parse("soon exit").is_err());
	}
}
