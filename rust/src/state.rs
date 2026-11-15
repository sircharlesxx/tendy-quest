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
}

impl Default for PlayerStats {
    fn default() -> Self {
        Self {
            hearts: 5,
            max_hearts: 5,
            score: 0,
            tendies: 0,
            total_tendies: 10,
            dash_energy: 100.0,
            speed_boost_timer: 0.0,
            has_ranch_shield: false,
            wave: 1,
            audio_muted: false,
        }
    }
}

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
