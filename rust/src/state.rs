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
    /// Optional boardwalk NPC easter-egg cutscene (`chillary.rs`); non-blocking for Level 2 progress.
    ChillaryCutscene,
    /// Pub 217 10-beers table & women's bathroom magic portal cutscene (`pub_portal.rs`).
    PubPortalCutscene,
    /// Level 3: Dedicated spacious Tendy Palace Casino hall (`casino.rs`).
    CasinoPlay,
    /// Willy Wonka + 2001 Stargate 5-hero roller coaster cutscene (`club.rs`).
    CoasterCutscene,
    /// Level 4: Shiny Trippy Velvet Disco Club (`club.rs`).
    ClubPlay,
    /// Turn-based 5-hero RPG showdown against Giant Lexie (`lexie_battle.rs`).
    LexieBattle,
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
    /// Restores Stan to the River Star pier after `ChillaryCutscene` without resetting the city run.
    pub chillary_return_city: bool,
    /// Caps the optional boardwalk easter-egg reward (`+250` score, `+1` heart) to once per run;
    /// does not affect Level 2 crew recruitment or boss progression.
    pub chillary_chilled: bool,
    /// Crew recruitment flags (TJ uses `tj_recruited`). These are the source of truth for the
    /// Level 2 crew gate; the city NPC components mirror them.
    pub derek_recruited: bool,
    pub nate_recruited: bool,
    pub glenn_recruited: bool,
    /// Stan has finished the Mike (smoke break) cutscene this city run; the last gate step.
    pub mike_smoked: bool,
    /// Set once the "crew assembled" moment has played this city run, so it fires only once.
    pub crew_unlock_announced: bool,
    /// Bumped to re-show the transient HUD hint even when `hint_text` is unchanged.
    pub hint_bump: u32,
    /// Test-link preset (`city_crew=1` / `city_crew_kaiju=1`): fresh city entry starts with the
    /// whole gate done and this many tendies eaten.
    pub test_crew_preset: Option<u32>,
    /// Gate progress restored from a checkpoint (bits: 1 TJ, 2 Derek, 4 Nate, 8 Glenn, 16 Mike),
    /// applied on the next fresh city entry.
    pub restore_gate_mask: u8,
    pub saved_city_pos: Vec3,
    pub driving_car: Option<Entity>,
    pub stamina_regen_delay: f32,
    pub estus_flasks: u32,
    pub max_estus_flasks: u32,
    pub estus_drink_timer: f32,
    pub heavy_attack: bool,
    pub lock_on_active: bool,
    pub tendy_credits: u32,
    /// Persistent Tendy Wendies currency wallet wagered in the Cedar Rapids Casino and tracked
    /// across Story Mode, Free Mode, reloads, and future features (`localStorage.tendy_wendies_v1`).
    pub tendy_wendies: u32,
    /// True while seated inside a Tendy Palace Casino game table or slot cabinet (`casino.rs`).
    pub casino_open: bool,
    /// True while playing one of the 3D physics arcade machines (`casino_arcade.rs`).
    pub arcade_open: bool,
    /// One-frame grace flag set when closing the casino/bartender modal so Esc doesn't also open Pause.
    pub casino_esc_grace: bool,
    /// True while inside the standalone Level 3 Tendy Palace Casino (`GameState::CasinoPlay`).
    pub in_casino_level: bool,
    /// True while inside the standalone Level 4 Velvet Disco Club (`GameState::ClubPlay`).
    pub in_club_level: bool,
    /// True while a VIP disco dance camera sequence is active inside `ClubPlay`.
    pub club_dance_open: bool,
    /// True while the Men's Room toilet flush caught cutscene is active inside `ClubPlay`.
    pub club_cutscene_open: bool,
    /// True when the player dies from a 30s fentanyl overdose in `ClubPlay` so Game Over returns to the club.
    pub club_fentanyl_death: bool,
    /// True while a Velvet Disco Club modal dialog is open (locks movement & unlocks cursor).
    pub club_modal_open: bool,
    /// True while seated in the Public Lobby stage-side seats drinking beer.
    pub club_seated: bool,
    /// Remaining seconds of Derek's psychedelic camera distortion inside `ClubPlay`.
    pub club_psychedelic_timer: f32,
    /// One-shot QA/preview flag so `?club=1` / `?club_broke=1` can set test TW without reading localStorage.
    pub club_skip_tw_sync: bool,
    /// Set when stepping into the Casino South Portal to return directly to Pub 217 (`CityPlay`).
    pub casino_return_city: bool,
    /// True while talking to Maria the bartender inside Pub 217 (`pub_portal.rs`).
    pub bartender_open: bool,
    /// True while the Free Mode multiplayer Character Select screen is open (`net.rs` / `index.html`).
    pub char_select_open: bool,
    /// True once Mom is defeated in Story Mode so entering Cedar Rapids opens Character Select.
    pub needs_city_char_select: bool,
    /// Stan's Cedar Rapids scale saved before warping into Casino Level 3, restored on return.
    pub saved_casino_city_scale: f32,
    pub scooter_app_unlocked: bool,
    pub riding_scooter: Option<Entity>,
    /// Current ridden scooter gravity multiplier (`1.0` = normal, -10% per landed backflip).
    pub scooter_gravity_scale: f32,
    /// On the riverfront Ferris wheel (boarding, riding or stepping out; see ferris.rs).
    pub riding_wheel: bool,
    pub phone_open: bool,
    pub high_timer: f32,
    /// Seconds left of being drunk from the Pub 217 beer tower (`city::update_beer_tower`).
    pub drunk_timer: f32,
    /// Seconds until the beer tower pours another beer.
    pub beer_cooldown: f32,
    /// Driver's license found (city pickup). Until then no car can be driven. Persisted as
    /// checkpoint field 8 and kept across city retries and into free roam.
    pub has_license: bool,
    /// A car has refused Stan at least once this run: the radar then shows the license card.
    pub license_refused: bool,
    /// Cooldown on the "no license" refusal so mashing F does not spam it.
    pub license_deny_cd: f32,
    /// The on-screen DRIVE button was tapped next to the parked car: `update_city_driving`
    /// treats it as an interact press this frame (keyboard / gamepad go there directly).
    pub car_prompt_tap: bool,
    pub wanted_stars: u8,
    pub wanted_decay_timer: f32,
    /// Red screen-edge damage flash, 1 on a hit and decaying to 0 (drawn by `ui::update_hurt_flash`).
    pub hurt_flash: f32,
    /// Camera shake requested by gameplay systems that can't reach `CameraOrbit`; `update_camera`
    /// folds it into `CameraOrbit::shake` and clears it.
    pub shake_request: f32,
    /// Post-game free roam (after beating Lexie): Stan stays 180 cm, no Lexie, the crew idle at
    /// their first-found spots without replaying cutscenes, and city tendies only add score.
    pub free_roam: bool,
    /// Lexie has been beaten at least once (persisted as checkpoint field 7, see
    /// `ui::save_game_checkpoint`). Survives restarts so a later save never drops the win.
    pub game_beaten: bool,
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
            chillary_return_city: false,
            chillary_chilled: false,
            derek_recruited: false,
            nate_recruited: false,
            glenn_recruited: false,
            mike_smoked: false,
            crew_unlock_announced: false,
            hint_bump: 0,
            test_crew_preset: None,
            restore_gate_mask: 0,
            saved_city_pos: Vec3::new(0.0, 0.0, 82.0),
            driving_car: None,
            stamina_regen_delay: 0.0,
            estus_flasks: 3,
            max_estus_flasks: 3,
            estus_drink_timer: 0.0,
            heavy_attack: false,
            lock_on_active: false,
            tendy_credits: 0,
            tendy_wendies: 100,
            casino_open: false,
            arcade_open: false,
            casino_esc_grace: false,
            in_casino_level: false,
            in_club_level: false,
            club_dance_open: false,
            club_cutscene_open: false,
            club_fentanyl_death: false,
            club_modal_open: false,
            club_seated: false,
            club_psychedelic_timer: 0.0,
            club_skip_tw_sync: false,
            casino_return_city: false,
            bartender_open: false,
            char_select_open: false,
            needs_city_char_select: false,
            saved_casino_city_scale: 1.8,
            scooter_app_unlocked: false,
            riding_scooter: None,
            scooter_gravity_scale: 1.0,
            riding_wheel: false,
            phone_open: false,
            high_timer: 0.0,
            drunk_timer: 0.0,
            beer_cooldown: 0.0,
            has_license: false,
            license_refused: false,
            license_deny_cd: 0.0,
            car_prompt_tap: false,
            wanted_stars: 0,
            wanted_decay_timer: 0.0,
            hurt_flash: 0.0,
            shake_request: 0.0,
            free_roam: false,
            game_beaten: false,
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
        if self.in_casino_level || self.in_club_level {
            return 1.8;
        }
        if self.in_city && self.free_roam {
            return 1.8;
        }
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

    /// Crew members recruited so far in Level 2 (TJ, Derek, Nate, Glenn).
    pub fn crew_count(&self) -> u32 {
        [self.tj_recruited, self.derek_recruited, self.nate_recruited, self.glenn_recruited]
            .iter()
            .filter(|&&r| r)
            .count() as u32
    }

    /// All four crew members found: city tendies become collectable.
    pub fn crew_assembled(&self) -> bool {
        self.crew_count() >= 4
    }

    /// Gate steps done out of 5: the four crew recruits plus the smoke break with Mike.
    pub fn gate_steps_done(&self) -> u32 {
        self.crew_count() + self.mike_smoked as u32
    }

    /// Gate steps still to do, in story order (crew names, then Mike's smoke break).
    pub fn missing_gate_steps(&self) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = self.missing_crew();
        if let Some(first) = out.first_mut() {
            *first = match *first {
                "TJ" => "recruit TJ",
                "Derek" => "recruit Derek",
                "Nate" => "recruit Nate",
                _ => "recruit Glenn",
            };
        }
        if !self.mike_smoked {
            out.push("smoke with Mike");
        }
        out
    }

    /// Names of crew members still missing, in story order.
    pub fn missing_crew(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if !self.tj_recruited {
            out.push("TJ");
        }
        if !self.derek_recruited {
            out.push("Derek");
        }
        if !self.nate_recruited {
            out.push("Nate");
        }
        if !self.glenn_recruited {
            out.push("Glenn");
        }
        out
    }

    /// The Level 2 gate: all four crew recruited AND the smoke break with Mike done.
    pub fn gate_open(&self) -> bool {
        self.crew_assembled() && self.mike_smoked
    }

    /// Gate progress as one bitmask (1 TJ, 2 Derek, 4 Nate, 8 Glenn, 16 Mike) for checkpoints.
    pub fn gate_mask(&self) -> u8 {
        (self.tj_recruited as u8)
            | (self.derek_recruited as u8) << 1
            | (self.nate_recruited as u8) << 2
            | (self.glenn_recruited as u8) << 3
            | (self.mike_smoked as u8) << 4
    }

    pub fn apply_gate_mask(&mut self, mask: u8) {
        self.tj_recruited = mask & 1 != 0;
        self.derek_recruited = mask & 2 != 0;
        self.nate_recruited = mask & 4 != 0;
        self.glenn_recruited = mask & 8 != 0;
        self.mike_smoked = mask & 16 != 0;
    }

    /// Sets up post-game free roam for the next fresh `CityPlay` entry (applied again by
    /// `enter_city_play`): city, 180 cm, whole gate done, Lexie beaten, scooter app unlocked.
    /// Why: intentionally leaves `self.game_beaten` untouched so entering Free Mode directly from
    /// the Start Menu (or `?free_roam=1`) does not falsely mark an unfinished campaign as beaten.
    /// Gameplay impact: lets players jump straight into Free Mode anytime while keeping story saves safe.
    pub fn prepare_free_roam(&mut self) {
        self.free_roam = true;
        self.lexie_defeated = true;
        self.in_city = true;
        self.boss_mode = false;
        self.tendies = 0;
        self.total_tendies = 50;
        self.player_scale = 1.8;
        self.wave = 4;
        self.scooter_app_unlocked = true;
        self.apply_gate_mask(31);
        self.crew_unlock_announced = true;
        self.restore_gate_mask = 0;
        self.test_crew_preset = None;
    }

    /// Credits `delta` Tendy Wendies to the player's persistent wallet (saved immediately).
    pub fn add_tendy_wendies(&mut self, delta: u32) {
        self.tendy_wendies = self.tendy_wendies.saturating_add(delta);
        crate::audio::save_tendy_wendies(self.tendy_wendies);
    }

    /// Attempts to spend `cost` Tendy Wendies; returns `false` if balance is insufficient.
    pub fn spend_tendy_wendies(&mut self, cost: u32) -> bool {
        if self.tendy_wendies < cost {
            return false;
        }
        self.tendy_wendies -= cost;
        crate::audio::save_tendy_wendies(self.tendy_wendies);
        true
    }

    /// Loads the persisted Tendy Wendies balance from `localStorage` into `PlayerStats`.
    pub fn sync_tendy_wendies_from_save(&mut self) {
        self.tendy_wendies = crate::audio::load_tendy_wendies();
    }

    /// Scale Stan's model is drawn at (and his saber and camera are sized for): `player_scale`
    /// in the apartment, `city_body_scale(player_scale)` in the city & Casino level.
    pub fn body_scale(&self) -> f32 {
        if self.in_city || self.in_casino_level || self.in_club_level {
            city_body_scale(self.player_scale)
        } else {
            self.player_scale
        }
    }

    /// City tendies can be eaten once the gate is open. A run that already holds every tendy
    /// (the `city_titan=1` preview) counts as unlocked so nothing there reads as locked.
    pub fn city_tendies_unlocked(&self) -> bool {
        self.gate_open() || self.tendies >= self.total_tendies
    }

    pub fn height_cm(&self) -> u32 {
        if self.in_city || self.in_casino_level || self.in_club_level {
            // 1.8 scale is a 180 cm adult and 18.0 is the 6500 cm kaiju; interpolate between them
            // so the HUD does not claim 650 cm at the start of the city.
            let t = ((self.player_scale - 1.8) / (18.0 - 1.8)).clamp(0.0, 1.0);
            return (180.0 + t * (6500.0 - 180.0)).round() as u32;
        }
        ((self.player_scale / 6.0) * 180.0)
            .round()
            .clamp(15.0, 180.0) as u32
    }

    pub fn size_tier(&self) -> &'static str {
        if self.in_club_level {
            return "Velvet Disco VIP";
        }
        if self.in_casino_level {
            return "Tendy Palace VIP";
        }
        if self.in_city && self.free_roam {
            return "Free Roam";
        }
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
    /// Smoothed beer sway amplitude (slower, rollier than the high sway).
    pub beer_amp: f32,
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
    /// Seconds of vehicle speed kick left (scooter turbo / boost re-arm it every frame).
    pub speed_kick_t: f32,
    /// Smoothed collision push-out offset (eases instead of snapping the camera out of boxes).
    pub push_offset: Vec3,
    /// Hero facing recorded when leaving `CityPlay`, restored when resuming from a city cutscene.
    pub saved_city_facing: f32,
    /// Decaying camera-shake intensity (0 = none). Hits and kaiju footsteps raise it; it decays
    /// quickly in `update_camera`, which turns it into a small rotational jitter.
    pub shake: f32,
}

/// Share of the gameplay scale that hero and crew models are drawn at while human-sized in the
/// city. Their models stand about 1.65 m per unit of scale, so the "180 cm" city scale of 1.8
/// drew them close to 3 m tall beside 1.8 m pedestrians and 1.7 m cars.
pub const CITY_HUMAN_SIZE: f32 = 0.68;

/// Drawn scale for a hero or crew member whose gameplay scale in the city is `s`:
/// `CITY_HUMAN_SIZE` x at the 1.8 starting size, easing back to 1x by scale 6 so the kaiju
/// sizes against the skyline are unchanged. Gameplay numbers (speed, reach of pickups, growth
/// thresholds) keep using the gameplay scale.
pub fn city_body_scale(s: f32) -> f32 {
    let t = ((s - 1.8) / 4.2).clamp(0.0, 1.0);
    s * (CITY_HUMAN_SIZE + (1.0 - CITY_HUMAN_SIZE) * t)
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
            beer_amp: 0.0,
            since_look: 99.0,
            blend_request: false,
            blend_t: CAMERA_BLEND_SECS,
            blend_from_pos: Vec3::ZERO,
            blend_from_rot: Quat::IDENTITY,
            blend_from_fov: std::f32::consts::FRAC_PI_4,
            fov_kick: 0.0,
            speed_kick_t: 0.0,
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
