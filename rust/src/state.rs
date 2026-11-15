use bevy::prelude::*;

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GameState {
    #[default]
    Tutorial,
    Playing,
    StageClear,
    GameOver,
}

#[derive(Resource)]
pub struct PlayerStats {
    pub hearts: i32,
    pub max_hearts: i32,
    pub score: u32,
    pub tendies: u32,
    pub total_tendies: u32,
    pub dash_energy: f32,
    pub speed_boost_timer: f32,
    pub has_ranch_shield: bool,
    pub wave: u32,
    pub audio_muted: bool,
    pub player_scale: f32,
    pub fridge_open: f32,
    pub hint_text: String,
}

impl Default for PlayerStats {
    fn default() -> Self {
        Self {
            hearts: 5,
            max_hearts: 5,
            score: 0,
            tendies: 0,
            total_tendies: 12,
            dash_energy: 100.0,
            speed_boost_timer: 0.0,
            has_ranch_shield: false,
            wave: 1,
            audio_muted: false,
            player_scale: 0.55,
            fridge_open: 0.0,
            hint_text: String::new(),
        }
    }
}

impl PlayerStats {
    pub fn target_scale(&self) -> f32 {
        match self.tendies {
            0..=4 => 0.55 + (self.tendies as f32) * 0.20,
            5..=8 => 1.35 + ((self.tendies - 4) as f32) * 0.4125,
            _ => 3.00 + ((self.tendies.min(12) - 8) as f32) * 0.75,
        }
    }

    pub fn height_cm(&self) -> u32 {
        ((self.player_scale / 6.0) * 180.0)
            .round()
            .clamp(15.0, 180.0) as u32
    }

    pub fn size_tier(&self) -> &'static str {
        match self.tendies {
            0..=3 => "Tiny Toy",
            4..=7 => "Counter Climber",
            8..=11 => "Kid Size",
            _ => "ADULT — OPEN FRIDGE!",
        }
    }
}

#[derive(Component)]
pub struct FridgeGoal;

#[allow(dead_code)]
#[derive(Component)]
pub struct StatusHint(pub String);

#[derive(Resource)]
pub struct CameraOrbit {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
}

impl Default for CameraOrbit {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.32,
            distance: 6.8,
        }
    }
}

#[derive(Component, Clone, Copy)]
pub struct PlatformBox {
    pub half_extents: Vec3,
}

#[derive(Component)]
pub struct LevelEntity;

#[derive(Component)]
pub struct TendyPickup {
    pub base_y: f32,
    pub phase: f32,
    #[allow(dead_code)]
    pub is_final_goal: bool,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum PowerupKind {
    HoneyMustard,
    RanchShield,
}

#[derive(Component)]
pub struct PowerupPickup {
    pub kind: PowerupKind,
    pub base_y: f32,
    pub phase: f32,
}

#[derive(Component, Clone, Copy)]
pub struct SceneRetryPath {
    pub path: &'static str,
    pub attempts: u8,
}

impl SceneRetryPath {
    pub fn new(path: &'static str) -> Self {
        Self { path, attempts: 0 }
    }
}
