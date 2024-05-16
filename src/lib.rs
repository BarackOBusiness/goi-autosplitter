#![allow(dead_code)]

use asr::future::next_tick;
use asr::game_engine::unity::mono::{Module, Version};
use asr::print_message;
use asr::time::Duration;
use asr::timer::{self, TimerState};
use asr::Process;

asr::async_main!(stable);

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

async fn main() -> () {
    loop {
        let process = Process::wait_attach("GettingOverIt.exe").await;

        let module = if let Some(v) = Module::attach(&process, Version::V2) {
            v
        } else {
            next_tick().await;
            continue;
        };

        let image = if let Some(v) = module.get_image(&process, "LivesplitIntegration") {
            v
        } else {
            next_tick().await;
            continue;
        };

        let assistant_class = image
            .wait_get_class(&process, &module, "LivesplitIntegration")
            .await;

        let table = assistant_class
            .wait_get_static_table(&process, &module)
            .await;

        let time_offset = assistant_class
            .wait_get_field_offset(&process, &module, "time")
            .await;

        let x_offset = assistant_class
            .wait_get_field_offset(&process, &module, "xPos")
            .await;

        let y_offset = assistant_class
            .wait_get_field_offset(&process, &module, "yPos")
            .await;

        process
            .until_closes(async {
                print_message("We're in");

                let mut last_time: f32 = 0.0;
                let mut split = 0;

                loop {
                    let time = match process.read::<f32>(table + time_offset) {
                        Ok(v) => v,
                        Err(e) => {
                            print_message(
                                format!("Error occurred when reading time value: {:?}", e).as_str(),
                            );
                            break;
                        }
                    };

                    let x = match process.read::<f32>(table + x_offset) {
                        Ok(v) => v,
                        Err(e) => {
                            print_message(
                                format!("Error occurred when reading x position: {:?}", e).as_str(),
                            );
                            break;
                        }
                    };

                    let y = match process.read::<f32>(table + y_offset) {
                        Ok(v) => v,
                        Err(e) => {
                            print_message(
                                format!("Error occurred when reading y position: {:?}", e).as_str(),
                            );
                            break;
                        }
                    };

                    // Time is -1.0 when in the main menu,
                    // it is set to 0.0 when loading the game scene
                    // and finally to -2 when at the reward screen.
                    // When closing the game, time is set to -3
                    if time == -3.0 {
                        break;
                    }

                    if time == -2.0 {
                        if last_time != time {
                            print_message(format!("{:?}", timer::state()).as_str());
                        }
                        last_time = time;
                        next_tick().await;
                        continue;
                    }

                    if time == -1.0 {
                        if timer::state() == TimerState::Ended
                            || timer::state() == TimerState::Running
                        {
                            split = 0;
                            timer::reset();
                        }
                        last_time = time;
                        next_tick().await;
                        continue;
                    }

                    if time > -1.0 {
                        if last_time == -1.0 {
                            timer::start();
                        } else if time < last_time {
                            split = 0;
                            timer::reset();
                            timer::start();
                        }
                    }

                    {
                        // Set the game time
                        let ms: i64 = (time * 1000.0).trunc() as i64;

                        timer::set_game_time(Duration::milliseconds(ms));
                    }

                    let pos = Vector2::new(x, y);
                    if split < 10 && pos.in_bounds(&SPLITS[split]) {
                        timer::split();
                        split = split + 1;
                    }

                    last_time = time;
                    next_tick().await;
                }
            })
            .await;
        timer::reset();
    }
}
