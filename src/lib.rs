#![allow(dead_code)]

use asr::future::next_tick;
use asr::game_engine::unity::mono::{Module, Version};
use asr::time::Duration;
use asr::timer;
use asr::{print_message, set_tick_rate};
use asr::{Address, Process};

asr::async_main!(stable);

// (Make sure to synchronize this to the bepinex plugin when editing)
// Reference for the game state bitmask:
// 0001
//    └─── "Is paused?" bit
// 0010
//   └──── "Should reset?" bit
// 1100
// └┼───── Scene bits:
//  ├───── 00: Main Menu
//  ├───── 01: Default Map (Mian Scene)
//  ├───── 10: Reward
//  └───── 11: Custom Map
// All following these bits is a mapping to custom levels. << 4 all values to get their appropriate bit positions.
// 1: Cavern Map ("Gems and Minerals" scene)

async fn main() -> () {
	set_tick_rate(1000.0);

	loop {
		let process: Process;

		// For standard windows builds
		#[cfg(not(feature = "linux"))]
		{
			process = Process::wait_attach("GettingOverIt.exe").await;
		}

		#[cfg(feature = "linux")]
		{
			process = Process::wait_attach("GettingOverIt.e").await;
		}

		let module = if let Some(v) = Module::attach(&process, Version::V2) {
			v
		} else {
			for _ in 1..=500 {
				// Wait half a second before retrying
				next_tick().await;
			}
			continue;
		};

		let image = if let Some(v) = module.get_image(&process, "LivesplitIntegration.BepInEx5") {
			v
		} else if let Some(v) = module.get_image(&process, "LivesplitIntegration.BepInEx6") {
			v
		} else {
			for _ in 1..=500 {
				next_tick().await;
			}
			continue;
		};

		let assistant_class = image
			.wait_get_class(&process, &module, "LivesplitIntegration")
			.await;

		let table = assistant_class
			.wait_get_static_table(&process, &module)
			.await;

		let (state_offset, time_offset, x_offset, y_offset) = (
			assistant_class
				.wait_get_field_offset(&process, &module, "state")
				.await,
			assistant_class
				.wait_get_field_offset(&process, &module, "time")
				.await,
			assistant_class
				.wait_get_field_offset(&process, &module, "xPos")
				.await,
			assistant_class
				.wait_get_field_offset(&process, &module, "yPos")
				.await,
		);

		process
			.until_closes(async {
				print_message("We're in");

				let mut last_state = State {
					position: Vector2::new(0.0, 0.0),
					scene: Scene::Unknown,
					time: 0.0,
					mask: 0b1100,
				};
				let mut splits: Vec<Rect> = Vec::new();
				let mut split = 0;

				loop {
					let state = match read_all_state(
						&process,
						table,
						state_offset,
						time_offset,
						x_offset,
						y_offset,
					) {
						Ok((v1, v2, v3, v4)) => State {
							position: Vector2::new(v3, v4),
							scene: Scene::parse(v1),
							time: v2,
							mask: v1,
						},
						Err(e) => {
							print_message(
								format!("Error occurred while reading state memory: {:?}", e)
									.as_str(),
							);
							break;
						}
					};

					// The timer should only be reset on the first tick that the should reset bit is active
					if last_state.mask & 0b10 != 0b10 && state.mask & 0b10 == 0b10 {
						timer::reset();
						split = 0;
					}

					// We parse the mask for the game scene every tick, so it's fair to just match the scene here
					match state.scene {
						Scene::Main => {
							// Reset bit was set last tick, this occurs every time the main scene is loaded
							// so the splits will only load a few times at the start of the run rather than every tick during the run
							if last_state.mask & 0b10 == 0b10 {
								timer::start();
								splits = Splits::default_map();
							}
						}
						Scene::Cavern => {
							match last_state.scene {
								Scene::Cavern => (), // Do nothing if we're already here
								_ => {
									// Load the correct splits if the last scene was non cavern
									// It should always be because the main scene will load first
									splits = Splits::cavern_map();
								}
							}
						}
						// If the end screen loaded, and the last scene was cavern
						// that means they finished, since cavern loads the end screen instantly.
						// So complete the last split. (Coin Tower)
						// (This is really just a workaround for the odd collider of the finish trigger in cavern)
						Scene::Reward => match last_state.scene {
							Scene::Cavern => timer::split(),
							_ => (),
						},
						_ => (),
					}

					// Get pause-bit of both last and current frame and shift last frame's to the twos place
					// 3 == both last and current frame are paused, 0 == both last and current frame are unpaused
					// 1 == last frame was unpaused and current frame is paused and 2 is vice versa
					match ((last_state.mask & 0b1) << 1) | (state.mask & 0b1) {
						3 => (),
						1 => timer::pause_game_time(),
						2 => {
							timer::resume_game_time();
							let ms = (state.time * 1000.0).trunc() as i64;
							timer::set_game_time(Duration::milliseconds(ms));
						}
						0 => {
							let ms = (state.time * 1000.0).trunc() as i64;
							timer::set_game_time(Duration::milliseconds(ms));
						}
						_ => unreachable!(),
					}

					// As long as we're on the segment prior to the last bound we can check if we're inside one, other maps may have special cases for the ending
					if split < splits.len() && state.position.in_bounds(&splits[split]) {
						timer::split();
						split = split + 1;
					}

					last_state = state;
					next_tick().await;
				}
			})
			.await;
		timer::reset();
	}
}

fn read_all_state(
	process: &Process,
	table: Address,
	state_offset: u32,
	time_offset: u32,
	x_offset: u32,
	y_offset: u32,
) -> Result<(u32, f32, f32, f32), asr::Error> {
	let state = match process.read::<u32>(table + state_offset) {
		Ok(v) => v,
		Err(e) => return Err(e),
	};
	let time = match process.read::<f32>(table + time_offset) {
		Ok(v) => v,
		Err(e) => return Err(e),
	};
	let x = match process.read::<f32>(table + x_offset) {
		Ok(v) => v,
		Err(e) => return Err(e),
	};
	let y = match process.read::<f32>(table + y_offset) {
		Ok(v) => v,
		Err(e) => return Err(e),
	};

	Ok((state, time, x, y))
}

struct State {
	scene: Scene,
	position: Vector2,
	time: f32,
	mask: u32,
}

struct Splits;

impl Splits {
	pub fn default_map() -> Vec<Rect> {
		[
			Rect::new(-12.0, -8.0, 7.0, 22.0),   // Tutorial
			Rect::new(24.0, 28.0, 82.0, 96.0),   // Chimney
			Rect::new(12.0, 18.0, 123.0, 128.0), // Slide
			Rect::new(4.0, 6.0, 162.0, 167.0),   // Furni
			Rect::new(18.0, 24.0, 216.0, 240.0), // Orange
			Rect::new(73.0, 77.0, 249.0, 259.0), // Anvil
			Rect::new(18.0, 22.0, 281.0, 295.0), // Bucket
			Rect::new(42.0, 46.0, 317.0, 331.0), // Ice
			Rect::new(0.0, 200.0, 359.0, 361.0), // Tower
			Rect::new(0.0, 200.0, 470.0, 472.0), // Space
		]
		.to_vec()
	}

	pub fn cavern_map() -> Vec<Rect> {
		// 367.8215 59.0722 10.5625
		[
			Rect::new(367.75, 373.75, 59.0, 62.0),   // One Penny
			Rect::new(365.25, 368.25, 120.5, 126.5), // Revolving Doors
			Rect::new(360.0, 363.0, 138.5, 144.5),   // Three
			Rect::new(346.5, 349.5, 172.0, 178.0),   // Swag sector
			Rect::new(345.5, 348.5, 210.5, 216.5),   // The obamid
			Rect::new(352.5, 355.0, 230.25, 236.25), // Red
			Rect::new(339.5, 342.0, 270.5, 276.5),   // Blue
		]
		.to_vec()
	}
}

#[derive(Clone, Copy, Debug)]
struct Vector2 {
	x: f32,
	y: f32,
}

impl Vector2 {
	pub const fn new(x: f32, y: f32) -> Self {
		Vector2 { x, y }
	}

	pub fn in_bounds(self, bounds: &Rect) -> bool {
		bounds.c1.x < self.x && self.x < bounds.c2.x && bounds.c1.y < self.y && self.y < bounds.c2.y
	}
}

// I'm just going to construct these in order from bottom left, top left, bottom right, top right
// so I don't have to relearn any sorting algorithms
#[derive(Clone, Copy, Debug)]
struct Rect {
	c1: Vector2, // Bottom-Left corner
	c2: Vector2, // Top-Right corner
}

impl Rect {
	pub const fn new(left: f32, right: f32, bottom: f32, top: f32) -> Self {
		Self {
			c1: Vector2::new(left, bottom),
			c2: Vector2::new(right, top),
		}
	}
}

enum Scene {
	Menu,    // 00
	Main,    // 01
	Reward,  // 10
	Unknown, // 11
	Cavern,  // 1 (Of custom map bits)
}

impl Scene {
	pub fn parse(state: u32) -> Scene {
		match (state & 0b1100) >> 2 {
			0 => Scene::Menu,
			1 => Scene::Main,
			2 => Scene::Reward,
			// Match custom map bits separately, that way they match up with the given identifiers.
			3 => match state >> 4 {
				1 => Scene::Cavern,
				_ => Scene::Unknown,
			},
			_ => unreachable!(),
		}
	}
}
