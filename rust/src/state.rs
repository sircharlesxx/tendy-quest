use bevy::prelude::*;

/// Top-level state machine separating Level 1 (Apartment), Level 2 (Cedar Rapids / Pub 217),
/// boss fights, and isolated cutscene stages.
///
/// Cutscenes intentionally live in their own states rather than pausing `Playing`/`CityPlay`
/// so each cutscene can hijack `Camera3d` cleanly and despawn its own `Entity` marker on exit
/// without interfering with gameplay camera/physics systems.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GameState {
    #[default]
    Tutorial,
    Playing,
    HeadphoneCutscene,
    Cutscene,
    BossFight,
    MomVictoryCutscene,
    CityIntroCutscene,
    CityPlay,
    TjCutscene,
    MikeCutscene,
    DerekCutscene,
    NateCutscene,
    GlennCutscene,
    LexieVictoryCutscene,
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
    pub boss_mode: bool,
    pub boss_hp: f32,
    pub boss_max_hp: f32,
    pub mom_quote: String,
    pub cutscene_timer: f32,
    pub cutscene_beat: u8,
    pub in_city: bool,
    pub lexie_defeated: bool,
    pub saved_apartment_pos: Vec3,
    pub saved_apartment_scale: f32,
    pub saved_apartment_yaw: f32,
    pub headphone_return_boss: bool,
    pub headphones_vow_taken: bool,
    pub enraged: bool,
    pub rage_level: u32,
    pub rage_swell_timer: f32,
    pub rage_swell_ticks: u32,
    pub rage_pulse_flash: f32,
    pub rage_cooldown_timer: f32,
    /// Set before transitioning from a cutscene back to `CityPlay` so `enter_city` restores
    /// `saved_city_pos` and skips resetting city pickups, vehicles, and already-triggered NPCs.
    pub tj_recruited: bool,
    pub tj_return_city: bool,
    pub mike_return_city: bool,
    pub derek_return_city: bool,
    pub nate_return_city: bool,
    pub glenn_return_city: bool,
    pub saved_city_pos: Vec3,
    pub driving_car: Option<Entity>,
    pub stamina_regen_delay: f32,
    pub estus_flasks: u32,
    pub max_estus_flasks: u32,
    pub estus_drink_timer: f32,
    pub heavy_attack: bool,
    pub lock_on_active: bool,
    pub tendy_credits: u32,
    pub scooter_app_unlocked: bool,
    pub riding_scooter: Option<Entity>,
    pub phone_open: bool,
    pub high_timer: f32,
    pub wanted_stars: u8,
    pub wanted_decay_timer: f32,
    /// Red screen-edge damage flash, 1 on a hit and decaying to 0 (drawn by `ui::update_hurt_flash`).
    pub hurt_flash: f32,
    /// Camera shake requested by gameplay systems that can't reach `CameraOrbit`; `update_camera`
    /// folds it into `CameraOrbit::shake` and clears it.
    pub shake_request: f32,
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
            boss_mode: false,
            boss_hp: 100.0,
            boss_max_hp: 100.0,
            mom_quote: String::new(),
            cutscene_timer: 0.0,
            cutscene_beat: 0,
            in_city: false,
            lexie_defeated: false,
            saved_apartment_pos: Vec3::new(0.0, -6.0, 5.5),
            saved_apartment_scale: 1.35,
            saved_apartment_yaw: 0.0,
            headphone_return_boss: false,
            headphones_vow_taken: false,
            enraged: false,
            rage_level: 0,
            rage_swell_timer: 0.0,
            rage_swell_ticks: 0,
            rage_pulse_flash: 0.0,
            rage_cooldown_timer: 0.0,
            tj_recruited: false,
            tj_return_city: false,
            mike_return_city: false,
            derek_return_city: false,
            nate_return_city: false,
            glenn_return_city: false,
            saved_city_pos: Vec3::new(0.0, 0.0, 82.0),
            driving_car: None,
            stamina_regen_delay: 0.0,
            estus_flasks: 3,
            max_estus_flasks: 3,
            estus_drink_timer: 0.0,
            heavy_attack: false,
            lock_on_active: false,
            tendy_credits: 0,
            scooter_app_unlocked: false,
            riding_scooter: None,
            phone_open: false,
            high_timer: 0.0,
            wanted_stars: 0,
            wanted_decay_timer: 0.0,
            hurt_flash: 0.0,
            shake_request: 0.0,
        }
    }
}

impl PlayerStats {
    /// Computes Stan's target world scale from tendies eaten.
    ///
    /// - Level 2 (`in_city`): scales from `1.8` (180cm adult) up to `18.0` (65m skyscraper kaiju)
    ///   with a slight `powf(1.08)` curve so late-game growth feels dramatic.
    /// - Level 1 (Apartment): uses a piecewise linear curve (`0.55` -> `6.0`) because early
    ///   tendies need gentle increments around small counter props while the final 4 tendies
    ///   must rapidly vault Stan to adult height (`6.0`) to reach the refrigerator handle.
    pub fn target_scale(&self) -> f32 {
        if self.in_city {
            let t = self.tendies.min(50) as f32 / 50.0;
            return 1.8 + t.powf(1.08) * 16.2;
        }
        let base = if self.boss_mode {
            3.2
        } else {
            match self.tendies {
                0..=4 => 0.55 + (self.tendies as f32) * 0.20,
                5..=8 => 1.35 + ((self.tendies - 4) as f32) * 0.4125,
                _ => 3.00 + ((self.tendies.min(12) - 8) as f32) * 0.75,
            }
        };
        if self.enraged {
            let max_cap = if self.boss_mode { 5.6 } else { 6.8 };
            let rage_bonus =
                (self.rage_swell_ticks as f32) * 0.22 * (self.rage_level.max(1) as f32);
            return (base + rage_bonus).min(max_cap);
        }
        base
    }

    pub fn height_cm(&self) -> u32 {
        if self.in_city {
            return ((self.player_scale / 18.0) * 6500.0)
                .round()
                .clamp(180.0, 6500.0) as u32;
        }
        ((self.player_scale / 6.0) * 180.0)
            .round()
            .clamp(15.0, 180.0) as u32
    }

    pub fn size_tier(&self) -> &'static str {
        if self.in_city {
            return match self.tendies {
                0..=11 => "NYC Street Brawler",
                12..=24 => "Subway Crusher",
                25..=37 => "Midrise Mutant",
                38..=49 => "Empire Titan",
                _ => "SKYSCRAPER KAIJU - SMASH LEXIE!",
            };
        }
        if self.enraged {
            return if !self.boss_mode && self.player_scale >= 5.95 {
                "ENRAGED TITAN - OPEN FRIDGE!"
            } else {
                "ENRAGED! SWELLING RED FURY!"
            };
        }
        if self.boss_mode {
            return "VIKING ONESIE KNIGHT";
        }
        match self.tendies {
            0..=3 => "Tiny Toy",
            4..=7 => "Counter Climber",
            8..=11 => "Kid Size",
            _ => "ADULT - OPEN FRIDGE!",
        }
    }
}

#[derive(Component)]
pub struct FridgeGoal;

#[derive(Component)]
pub struct HeadphoneHazard {
    pub radius: f32,
}

#[allow(dead_code)]
#[derive(Component)]
pub struct StatusHint(pub String);

#[derive(Resource)]
pub struct CameraOrbit {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub current_dist: f32,
    /// Damped follow point (player feet position, smoothed). The camera boom hangs off this
    /// instead of the raw player translation so jumps/bobs don't shake the view.
    pub follow_pos: Vec3,
    /// Raw player position last frame, used to detect teleports/respawns.
    pub last_player_pos: Vec3,
    /// When true, `update_camera` snaps the follow point and boom length next frame.
    pub snap_follow: bool,
    /// Smoothed "drunk" sway amplitude (0 = perfectly steady camera).
    pub wobble_amp: f32,
    /// Seconds since the player last gave look input (mouse/touch/stick).
    /// Vehicle chase cams pause auto-align while this is small.
    pub since_look: f32,
    /// Set on entering a gameplay state; `update_camera` captures the current camera pose and
    /// eases from it into the follow pose over `CAMERA_BLEND_SECS`.
    pub blend_request: bool,
    /// Elapsed blend time; `>= CAMERA_BLEND_SECS` means no blend is active.
    pub blend_t: f32,
    pub blend_from_pos: Vec3,
    pub blend_from_rot: Quat,
    pub blend_from_fov: f32,
    /// Smoothed FOV widening while dashing.
    pub fov_kick: f32,
    /// Smoothed collision push-out offset (eases instead of snapping the camera out of boxes).
    pub push_offset: Vec3,
    /// Hero facing recorded when leaving `CityPlay`, restored when resuming from a city cutscene.
    pub saved_city_facing: f32,
    /// Decaying camera-shake intensity (0 = none). Hits and kaiju footsteps raise it; it decays
    /// quickly in `update_camera`, which turns it into a small rotational jitter.
    pub shake: f32,
}

/// Duration of the ease from a cutscene's last shot back into the gameplay follow camera.
pub const CAMERA_BLEND_SECS: f32 = 0.4;

impl CameraOrbit {
    /// 0 while the player is actively looking around (and for ~1.5 s after), easing back to 1,
    /// so vehicle auto-align never fights manual look input.
    pub fn auto_align_weight(&self) -> f32 {
        let x = ((self.since_look - 1.5) / 1.0).clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
    }
}

impl Default for CameraOrbit {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.45,
            distance: 10.6,
            current_dist: 10.6,
            follow_pos: Vec3::ZERO,
            last_player_pos: Vec3::ZERO,
            snap_follow: true,
            wobble_amp: 0.0,
            since_look: 99.0,
            blend_request: false,
            blend_t: CAMERA_BLEND_SECS,
            blend_from_pos: Vec3::ZERO,
            blend_from_rot: Quat::IDENTITY,
            blend_from_fov: std::f32::consts::FRAC_PI_4,
            fov_kick: 0.0,
            push_offset: Vec3::ZERO,
            saved_city_facing: 0.0,
            shake: 0.0,
        }
    }
}

/// Axis-aligned bounding box used for custom floor/counter/ramp landing without a full physics crate.
/// `half_extents` are measured from the entity's `Transform.translation` center.
#[derive(Component, Clone, Copy)]
pub struct PlatformBox {
    pub half_extents: Vec3,
}

/// Marker for Level 1 entities so `GameOver` / `StageClear` / transitions can despawn the apartment cleanly.
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

/// Tracks transient `.glb` load failures on WebAssembly/GitHub Pages so `main.rs` can retry
/// fetching the `Scene` handle a few times before giving up.
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
