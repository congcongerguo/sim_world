use bevy::prelude::*;

// ---------------------------------------------------------------------------
// Time units — 1 tick = 1 day, 1 month = 30 days, 1 year = 12 months = 360 days
// ---------------------------------------------------------------------------

/// 1 month in ticks (30 days).
pub const MONTH: f64 = 30.0;
/// 1 year in ticks (12 months = 360 days).
pub const YEAR: f64 = 360.0;

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

/// Time speed multiplier — separate from SimTime to avoid system conflicts.
#[derive(Resource)]
pub struct TimeScale {
    pub speed: f64,
    pre_pause_speed: f64,
}

impl Default for TimeScale {
    fn default() -> Self {
        Self { speed: 8.0, pre_pause_speed: 8.0 }
    }
}

impl TimeScale {
    pub fn toggle_pause(&mut self) {
        if self.speed == 0.0 {
            self.speed = self.pre_pause_speed;
        } else {
            self.pre_pause_speed = self.speed;
            self.speed = 0.0;
        }
    }

    pub fn speed_up(&mut self) {
        let levels = [0.5, 1.0, 2.0, 4.0, 8.0];
        if let Some(next) = levels.iter().find(|&&l| l > self.speed) {
            self.speed = *next;
        }
        if self.speed != 0.0 {
            self.pre_pause_speed = self.speed;
        }
    }

    pub fn slow_down(&mut self) {
        let levels = [0.5, 1.0, 2.0, 4.0, 8.0];
        if let Some(prev) = levels.iter().rev().find(|&&l| l < self.speed) {
            self.speed = *prev;
        }
        if self.speed != 0.0 {
            self.pre_pause_speed = self.speed;
        }
    }
}

/// Accumulated simulation time.
#[derive(Resource, Default)]
pub struct SimTime {
    pub elapsed: f64,
}

impl SimTime {
    /// Returns (years, months, days) since epoch.
    pub fn date(&self) -> (u64, u64, u64) {
        let d = self.elapsed;
        let y = (d / YEAR) as u64;
        let rem = d % YEAR;
        let m = (rem / MONTH) as u64;
        let day = (rem % MONTH) as u64;
        (y, m, day)
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct SimTimePlugin;

impl Plugin for SimTimePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TimeScale>();
        app.init_resource::<SimTime>();
        app.add_systems(Update, (advance_sim_time, time_control_input));
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

fn advance_sim_time(time: Res<Time>, scale: Res<TimeScale>, mut sim: ResMut<SimTime>) {
    sim.elapsed += time.delta_secs_f64() * scale.speed;
}

fn time_control_input(keys: Res<ButtonInput<KeyCode>>, mut scale: ResMut<TimeScale>) {
    if keys.just_pressed(KeyCode::Space) {
        scale.toggle_pause();
    }
    if keys.just_pressed(KeyCode::F3) {
        scale.speed_up();
    }
    if keys.just_pressed(KeyCode::F4) {
        scale.slow_down();
    }
}
