#![allow(dead_code)]

use asr::future::next_tick;
use asr::game_engine::unity::mono::{Module, Version};
use asr::time::Duration;
use asr::timer;
use asr::{print_message, set_tick_rate};
use asr::{Address, Process};

asr::async_main!(stable);

const SPLITS: [Rect; 10] = [
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
];

// Let's switch to using a game state bitmask in the livesplit integration
// The bits in question:
// 1000
// └────    "Should reset?" bit
// 0100
//  └───    "Is paused?" bit
// 0011
//   └┼─    Scene bits:
//    ├──── 11: Custom map
//    ├──── 10: Main menu
//    ├──── 01: Reward
//    └──── 00: Default Map

async fn main() -> () {
    set_tick_rate(1000.0);

    loop {
        let process = Process::wait_attach("GettingOverIt.exe").await;

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
                    mask: 0b0011,
                };
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

                    #[cfg(debug_assertions)]
                    {
                        asr::timer::set_variable(
                            "pause-bits",
                            format!(
                                "{}",
                                (((last_state.mask & 0b100) >> 1) | ((state.mask & 0b100) >> 2))
                            )
                            .as_str(),
                        );
                    }

                    // The timer should only be reset on the first millisecond that the should reset bit is active
                    if last_state.mask & 0b1000 != 8 && state.mask & 0b1000 == 8 {
                        timer::reset();
                        split = 0;
                    }

                    // Start the timer if we're in the main game scene
                    if state.mask & 0b0011 == 0 {
                        timer::start();
                    }

                    // Get pause-bit of both last and current frame and shift them to the twos and ones place respectively
                    // 3 == both last and current frame are paused, 0 == both last and current frame are unpaused
                    // 1 == last frame was unpaused and current frame is paused and 2 is vice versa
                    match ((last_state.mask & 0b100) >> 1) | ((state.mask & 0b100) >> 2) {
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

                    if split < 10 && state.position.in_bounds(&SPLITS[split]) {
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
    scene: Scene, // This is only here for if in the future I decide to support custom maps
    position: Vector2,
    time: f32,
    mask: u32,
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
        // Check if it's within horizontal boundary
        let within_x = self.x > bounds.c1.x && self.x < bounds.c2.x;
        // Check if it's within vertical boundary
        let within_y = self.y > bounds.c2.y && self.y < bounds.c1.y;
        within_x && within_y
    }
}

#[derive(Clone, Copy, Debug)]
struct Rect {
    c1: Vector2, // Top-Left Corner
    c2: Vector2, // Bottom-Right Corner
}

impl Rect {
    pub const fn new(left: f32, right: f32, bottom: f32, top: f32) -> Self {
        Rect {
            c1: Vector2::new(left, top),
            c2: Vector2::new(right, bottom),
        }
    }
}

enum Scene {
    Main,    // 00
    Reward,  // 01
    Menu,    // 10
    Unknown, // 11
}

impl Scene {
    pub fn parse(state: u32) -> Scene {
        match state & 0b11 {
            3 => Scene::Unknown,
            2 => Scene::Menu,
            1 => Scene::Reward,
            0 => Scene::Main,
            _ => unreachable!(),
        }
    }
}
