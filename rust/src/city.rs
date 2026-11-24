use bevy::prelude::*;
use crate::audio::{
    is_voice_speaking, play_cutscene_music, play_sfx, play_viking_chant_music,
    set_touch_boss_mode, set_touch_playing, speak_voice, take_cutscene_start_time,
};
use crate::boss::{CutsceneEyeReflection, HeroCosmetic, spawn_mom_mesh_hierarchy};
use crate::enemies::{Enemy, FiberPuddle};
use crate::player::Player;
use crate::state::{CameraOrbit, GameState, LevelEntity, PlatformBox, PlayerStats, TendyPickup};
use crate::ui::HudOverlay;

// Cedar Rapids uses a 4x4 city block grid spaced every 46m (`ROAD_COORDS` = avenues,
// `BLOCK_COORDS` = building block centers), with Lexie's Citadel anchoring the north end
// of the central x = 0 avenue.
pub(crate) const CITY_HALF_EXTENT: f32 = 135.0;
const ROAD_COORDS: [f32; 5] = [-92.0, -46.0, 0.0, 46.0, 92.0];
const BLOCK_COORDS: [f32; 4] = [-69.0, -23.0, 23.0, 69.0];
const CITADEL_POS: Vec3 = Vec3::new(0.0, 0.0, -82.0);
const LEXIE_TERRACE_POS: Vec3 = Vec3::new(0.0, 38.0, -75.5);
/// Pub 217's east doorway, the point the whole pub is built and scaled around. The pub sits on
/// the (-23, 23) block (sidewalk x -36.25..-9.75, z 9.75..36.25) with its door facing 3rd St SE
/// (x = 0): footprint x -32.6..-12.3, z 12.85..33.15, about 2.5-3.7 m of sidewalk on every side.
pub const PUB217_ANCHOR: Vec3 = Vec3::new(-13.0, 0.0, 23.0);
/// The pub's interior layout (walls, bar, stools, beer tower) is authored in a fixed design frame
/// whose doorway is here; pieces are placed at `(design_pos - PUB217_LAYOUT_ORIGIN)` under the
/// pub root, so moving `PUB217_ANCHOR` moves the whole building.
const PUB217_LAYOUT_ORIGIN: Vec3 = Vec3::new(-11.75, 0.0, 34.0);
/// Where a fresh city entry drops Stan: the brick walk on the west side of 3rd St SE, just
/// south of the pub block, facing north with Pub 217's door about 17 m ahead on his left.
const CITY_START_POS: Vec3 = Vec3::new(-8.4, 0.0, 39.0);

/// Pub 217 is built at a fixed `1.45x` around `PUB217_ANCHOR` (its east doorway). It no longer
/// grows with Stan: the crew gate keeps him at 180cm until TJ, Derek, Nate and Glenn are all
/// recruited, so a grown Stan treats the pub like any other small building.
pub const PUB217_SCALE: f32 = 1.45;
/// Above this hero scale the pub doorway is plugged (see `update_pub217_door`), so a growing
/// Stan can't wedge himself into the bar with his head through the ceiling.
const PUB217_DOOR_MAX_SCALE: f32 = 3.2;

/// True while `pos` is inside the pub interior (footprint and below the ceiling). Standing on
/// the roof doesn't count. `_player_scale` is unused since the pub no longer scales.
pub fn is_in_pub217(pos: Vec3, _player_scale: f32) -> bool {
    let s = PUB217_SCALE;
    let min_x = PUB217_ANCHOR.x - 13.5 * s;
    let max_x = PUB217_ANCHOR.x + 0.5 * s;
    let min_z = PUB217_ANCHOR.z - 7.0 * s;
    let max_z = PUB217_ANCHOR.z + 7.0 * s;
    pos.x >= min_x
        && pos.x <= max_x
        && pos.z >= min_z
        && pos.z <= max_z
        && pos.y < 6.2 * s
}

#[derive(Component)]
pub struct Pub217Root;

/// Invisible collider across the pub doorway, active only once Stan has outgrown the door.
#[derive(Component)]
pub struct Pub217DoorPlug;

// ---------------------------------------------------------------------------------------------
// Scattered crew: TJ, Derek, Nate and Glenn each wait somewhere in Cedar Rapids (Mike stays in
// Pub 217). In post-game free roam all four hang out in the pub instead.
// ---------------------------------------------------------------------------------------------

/// Crew indices shared by `CrewSpots`, the radar and the call-outs.
pub const CREW_TJ: usize = 0;
pub const CREW_DEREK: usize = 1;
pub const CREW_NATE: usize = 2;
pub const CREW_GLENN: usize = 3;
const CREW_NAMES: [&str; 4] = ["TJ", "DEREK", "NATE", "GLENN"];
/// A waiting crew member calls out (and shows on the radar) once Stan is this close; their
/// floating marker is only drawn inside the same range.
pub const CREW_REVEAL_RANGE: f32 = 35.0;

/// This build's waiting spot (position, yaw facing the street) per crew member, re-rolled every
/// time the city world is built, and whether each has called out to Stan yet.
#[derive(Resource)]
pub struct CrewSpots {
    pub spots: [(Vec3, f32); 4],
    pub revealed: [bool; 4],
}

/// Free-roam hangout spots inside Pub 217, as pre-scale offsets from `PUB217_ANCHOR` (TJ by the
/// door, Derek, Nate and Glenn at their old bar spots).
const CREW_PUB_OFFSETS: [Vec3; 4] = [
    Vec3::new(-3.0, 0.0, -4.6),
    Vec3::new(-9.6, 0.0, -3.4),
    Vec3::new(-7.4, 0.0, -4.4),
    Vec3::new(-11.2, 0.0, 2.2),
];

/// Mike's spot at the bar (pre-scale offset from `PUB217_ANCHOR`).
pub fn mike_pub_spot() -> Vec3 {
    PUB217_ANCHOR + Vec3::new(-4.5, 0.0, 4.3) * PUB217_SCALE
}

/// Where crew member `i` stands while not following Stan and which way they face: inside Pub 217
/// in free roam (facing Stan), otherwise at this build's scattered spot, facing the street until
/// Stan is close, then turning to him.
pub fn crew_wait_pose(i: usize, stats: &PlayerStats, spots: Option<&CrewSpots>, stan: Vec3) -> (Vec3, Quat) {
    let (pos, street_yaw, in_pub) = match spots {
        Some(c) if !stats.free_roam => (c.spots[i].0, c.spots[i].1, false),
        _ => (PUB217_ANCHOR + CREW_PUB_OFFSETS[i] * PUB217_SCALE, 0.0, true),
    };
    let to_p = Vec2::new(stan.x - pos.x, stan.z - pos.z);
    let d = to_p.length();
    let yaw = if d > 0.1 && (in_pub || d < 12.0) { to_p.x.atan2(to_p.y) } else { street_yaw };
    (pos, Quat::from_rotation_y(yaw))
}

/// Curated waiting spots: each on a block sidewalk 11 m out from the block centre (2-2.5 m in
/// front of the facade, 12 m from the road centre, so clear of every lane and the parking line),
/// facing the street (yaw), away from the stunt circuits, Greene Square, Pub 217 and the
/// sidewalk NPC patrols.
/// (x, z, yaw)
pub(crate) const CREW_SPOT_CANDIDATES: [(f32, f32, f32); 16] = [
    (-80.0, -23.0, -std::f32::consts::FRAC_PI_2),
    (-69.0, 34.0, 0.0),
    (-23.0, -34.0, std::f32::consts::PI),
    (34.0, -27.0, std::f32::consts::FRAC_PI_2),
    (34.0, 23.0, std::f32::consts::FRAC_PI_2),
    (58.0, -69.0, -std::f32::consts::FRAC_PI_2),
    (69.0, -12.0, 0.0),
    (-23.0, 80.0, 0.0),
    (34.0, 69.0, std::f32::consts::FRAC_PI_2),
    (-80.0, -69.0, -std::f32::consts::FRAC_PI_2),
    (103.0, 23.0, -std::f32::consts::FRAC_PI_2),
    (-23.0, -58.0, 0.0),
    (69.0, 34.0, 0.0),
    (12.0, -69.0, -std::f32::consts::FRAC_PI_2),
    (-69.0, 58.0, std::f32::consts::PI),
    (23.0, 103.0, std::f32::consts::PI),
];

/// Picks four waiting spots from `CREW_SPOT_CANDIDATES`: shuffled by `seed`, then taken greedily
/// so each sits in a different district (3 x 3 split of the map), 45 m+ from the others, and at
/// most one within 45 m of Stan's start. Falls back to looser spacing if a shuffle can't fit.
fn pick_crew_spots(seed: u64) -> [(Vec3, f32); 4] {
    let flat_d = |a: Vec3, b: Vec3| Vec2::new(a.x - b.x, a.z - b.z).length();
    let stunt = crate::stunts::stunt_tendy_spots();
    let mut cands: Vec<(Vec3, f32)> = CREW_SPOT_CANDIDATES
        .iter()
        .map(|&(x, z, yaw)| (Vec3::new(x, 0.0, z), yaw))
        .filter(|(p, _)| {
            is_clear_of_skyscrapers(p.x, p.z, 1.5)
                && is_clear_of_skyscrapers(p.x, p.z, 0.0)
                && flat_d(*p, Vec3::new(69.0, 0.0, 69.0)) > 22.0
                && stunt.iter().all(|&q| flat_d(*p, q) > 10.0)
        })
        .collect();
    let mut s = seed | 1;
    let mut next = move || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        s
    };
    for i in (1..cands.len()).rev() {
        let j = (next() % (i as u64 + 1)) as usize;
        cands.swap(i, j);
    }
    let district = |p: Vec3| {
        let cell = |v: f32| if v < -35.0 { 0 } else if v > 35.0 { 2 } else { 1 };
        (cell(p.x), cell(p.z))
    };
    for (min_d, distinct) in [(45.0_f32, true), (30.0, true), (20.0, false)] {
        let mut picked: Vec<(Vec3, f32)> = Vec::with_capacity(4);
        for &(p, yaw) in &cands {
            if picked.len() == 4 {
                break;
            }
            let near_start = flat_d(p, CITY_START_POS) < 45.0;
            if near_start && picked.iter().any(|(q, _)| flat_d(*q, CITY_START_POS) < 45.0) {
                continue;
            }
            if picked
                .iter()
                .any(|(q, _)| flat_d(*q, p) < min_d || (distinct && district(*q) == district(p)))
            {
                continue;
            }
            picked.push((p, yaw));
        }
        if picked.len() == 4 {
            return [picked[0], picked[1], picked[2], picked[3]];
        }
    }
    let c = |i: usize| (Vec3::new(CREW_SPOT_CANDIDATES[i].0, 0.0, CREW_SPOT_CANDIDATES[i].1), CREW_SPOT_CANDIDATES[i].2);
    [c(0), c(4), c(9), c(12)]
}

/// Fixed scatter seed for the `city_start=crew` debug view.
const DEBUG_CREW_SEED: u64 = 217;

/// Small floating chevron + ground ring over a waiting crew member, shown only within
/// `CREW_REVEAL_RANGE` so it helps at street level without giving them away across the map.
#[derive(Component)]
pub struct CrewWaitTell(pub usize);

fn spawn_crew_tells(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    spots: &[(Vec3, f32); 4],
) {
    let glow = materials.add(StandardMaterial {
        base_color: Color::srgb(0.45, 1.0, 0.55),
        emissive: LinearRgba::rgb(0.9, 3.2, 1.2),
        unlit: true,
        ..default()
    });
    let ring_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.45, 1.0, 0.55, 0.45),
        emissive: LinearRgba::rgb(0.5, 1.8, 0.7),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let cone = meshes.add(Cone::new(0.26, 0.5));
    let ring = meshes.add(Torus::new(1.15, 1.3));
    for (i, &(pos, _)) in spots.iter().enumerate() {
        commands
            .spawn((
                LevelEntity,
                CityWorldMarker,
                CrewWaitTell(i),
                Transform::from_translation(pos),
                Visibility::Hidden,
            ))
            .with_children(|t| {
                t.spawn((
                    Mesh3d(cone.clone()),
                    MeshMaterial3d(glow.clone()),
                    Transform::from_xyz(0.0, 4.5, 0.0).with_rotation(Quat::from_rotation_x(std::f32::consts::PI)),
                ));
                t.spawn((
                    Mesh3d(ring.clone()),
                    MeshMaterial3d(ring_mat.clone()),
                    Transform::from_xyz(0.0, 0.1, 0.0).with_scale(Vec3::new(1.0, 0.3, 1.0)),
                ));
            });
    }
}

fn crew_recruited(stats: &PlayerStats, i: usize) -> bool {
    match i {
        CREW_TJ => stats.tj_recruited,
        CREW_DEREK => stats.derek_recruited,
        CREW_NATE => stats.nate_recruited,
        _ => stats.glenn_recruited,
    }
}

/// Waiting crew call out once Stan gets within `CREW_REVEAL_RANGE` (hint toast, chime, spoken
/// line; from then on their radar marker shows) and their floating marker is drawn in range.
pub fn update_crew_callouts(
    time: Res<Time>,
    mut stats: ResMut<PlayerStats>,
    spots: Option<ResMut<CrewSpots>>,
    player_q: Query<&Transform, With<Player>>,
    mut tell_q: Query<(&CrewWaitTell, &mut Transform, &mut Visibility), Without<Player>>,
) {
    let (Some(mut spots), Ok(p_tf)) = (spots, player_q.get_single()) else {
        return;
    };
    let p = p_tf.translation;
    let t = time.elapsed_secs();
    for (tell, mut tf, mut vis) in &mut tell_q {
        let i = tell.0.min(3);
        let pos = spots.spots[i].0;
        let waiting = !stats.free_roam && !crew_recruited(&stats, i);
        let d = Vec2::new(p.x - pos.x, p.z - pos.z).length();
        let want = if waiting && d < CREW_REVEAL_RANGE { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
        tf.translation = pos + Vec3::Y * (t * 2.4 + i as f32).sin() * 0.18;
        tf.rotation = Quat::from_rotation_y(t * 1.6);
    }
    if stats.free_roam {
        return;
    }
    for i in 0..4 {
        if spots.revealed[i] || crew_recruited(&stats, i) {
            continue;
        }
        let pos = spots.spots[i].0;
        if Vec2::new(p.x - pos.x, p.z - pos.z).length() >= CREW_REVEAL_RANGE {
            continue;
        }
        spots.revealed[i] = true;
        let (line, spoken, role) = match i {
            CREW_TJ => ("Stan! Over here, by the curb!", "Stan! Over here!", "tj"),
            CREW_DEREK => ("Yo, Stan! Over here, champ!", "Yo, Stan! Over here!", "derek"),
            CREW_NATE => ("Stan? Stan! Over here, man!", "Stan! Over here, man!", "nate"),
            _ => ("Bro! Over here! Come say hi!", "Bro! Over here!", "glenn"),
        };
        stats.hint_text = format!("{}: \"{}\" ({} is on your radar.)", CREW_NAMES[i], line, CREW_NAMES[i]);
        stats.hint_bump = stats.hint_bump.wrapping_add(1);
        play_sfx("hack_success");
        speak_voice(spoken, role);
    }
}

/// Crew-gate look of the city tendies: `lit` is the look currently applied by
/// `update_city_crew_gate` (`None` right after a world spawn, so it is applied once).
#[derive(Resource)]
pub struct CityTendyLook {
    pub lit: Option<bool>,
}

/// Translucent grey "sealed" bubble around a city tendy, shown while the crew gate is locked.
#[derive(Component)]
pub struct CityTendySeal;

/// Golden sky beacon above a city tendy, shown once the crew gate is open.
#[derive(Component)]
pub struct CityTendyBeacon;

#[derive(Component)]
pub struct CityWorldMarker;

#[derive(Component)]
pub struct CityTendy;

#[derive(Component)]
pub struct AnimeStageEntity;

#[derive(Component)]
pub struct AnimeOverlay;

#[derive(Component)]
pub struct AnimeSpeakerText;

#[derive(Component)]
pub struct AnimeSubtitleText;

#[derive(Component)]
pub struct AnimeBeatBadge;

#[derive(Component)]
pub struct AnimeSakuraPetal {
    pub drift_phase: f32,
    pub fall_speed: f32,
    pub sway_amp: f32,
}

#[derive(Component)]
pub struct AnimeCutsceneMom;

#[derive(Component)]
pub struct CityIntroOverlay;

#[derive(Component)]
pub struct CityIntroSpeakerText;

#[derive(Component)]
pub struct CityIntroSubtitleText;

#[derive(Component)]
pub struct CityIntroBeatBadge;

#[derive(Component)]
pub struct CityPlayUi;

#[derive(Component)]
pub struct LexieBossHudRoot;

#[derive(Component)]
pub struct CityMinimapPlayerDot;

#[derive(Component)]
pub struct CityMinimapBossDot;

#[derive(Component)]
pub struct CityMinimapTendyDot;

/// Radar marker: 0-3 = TJ, Derek, Nate, Glenn (`CREW_*`; only after they have called out to
/// Stan), 4 = Pub 217 (Mike, until the smoke break), 5 = the driver's license card.
#[derive(Component)]
pub struct CityMinimapCrewDot(pub u8);

#[derive(Component)]
pub struct LexieBossHudText;

#[derive(Component)]
pub struct LexieBossHpFill;

#[derive(Component)]
pub struct CityCar {
    pub axis_x: bool,
    pub dir: f32,
    pub speed: f32,
    pub horn_cooldown: f32,
    pub occupied: bool,
    pub parked: bool,
    pub yaw: f32,
    pub velocity: Vec2,
    pub pitch_lean: f32,
    pub roll_lean: f32,
    pub steer_input: f32,
    pub knock_vel: Vec2,
    pub drift_sfx_cooldown: f32,
    pub car_hp: f32,
    /// Height of the highest part of the roof: `CAR_ROOF_TOP`, or `CAR_CAB_SIGN_TOP` for cabs.
    pub roof_h: f32,
    /// Wedge ramp ride state (`stunts::update_ramp_cars`): body height, vertical speed, airborne,
    /// and the ground's nose-up pitch / right-up roll under the wheels.
    pub ride: CarRide,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct CarRide {
    pub y: f32,
    pub vy: f32,
    pub air: bool,
    pub pitch: f32,
    pub roll: f32,
}

// Car dimensions as built by `car_meshes` (cars spawn at scale 1, origin on the road):
// body 2.15 x 4.45 (4.66 with bumpers), hood/trunk top 1.13 m, glass cabin + roof over
// z = -1.275..0.975 topping out at 1.72 m, cab roof signs at 1.89 m.
pub const CAR_HALF_W: f32 = 1.075;
pub const CAR_HALF_L: f32 = 2.33;
pub const CAR_BODY_TOP: f32 = 1.13;
pub const CAR_ROOF_TOP: f32 = 1.72;
pub const CAR_CAB_SIGN_TOP: f32 = 1.89;
const CAR_CABIN_Z: (f32, f32) = (-1.275, 0.975);
/// How far below a car surface something can be and still count as on top of it (snaps up).
/// Covers one frame of fall plus the car sliding sideways under a descending scooter.
pub const CAR_TOP_STEP: f32 = 0.45;

/// Where a point (a scooter's wheel line) sits relative to a car.
#[derive(Clone, Copy, PartialEq)]
pub enum CarContact {
    /// Outside the car's footprint (grown by `pad`).
    Clear,
    /// Over the car and high enough to ride on it: world height of the surface below.
    Over(f32),
    /// Inside the car's body: a collision.
    Inside,
}

/// Height-aware contact between `p` and a car: the hood/trunk is `CAR_BODY_TOP` high, the
/// cabin strip `roof_h`. A point within `CAR_TOP_STEP` below a surface counts as on top of it.
pub fn city_car_contact(car_tf: &Transform, car: &CityCar, p: Vec3, pad: f32) -> CarContact {
    let local = car_tf.rotation.inverse() * (p - car_tf.translation);
    if local.x.abs() > CAR_HALF_W + pad || local.z.abs() > CAR_HALF_L + pad {
        return CarContact::Clear;
    }
    let base = car_tf.translation.y;
    let in_cabin = local.z > CAR_CABIN_Z.0 - pad && local.z < CAR_CABIN_Z.1 + pad;
    if in_cabin && local.y >= car.roof_h - CAR_TOP_STEP {
        CarContact::Over(base + car.roof_h)
    } else if local.y >= CAR_BODY_TOP - CAR_TOP_STEP {
        // Below the cabin top in the cabin strip only happens when riding off the hood into
        // the windscreen; ride along at hood height rather than popping onto the roof.
        CarContact::Over(base + CAR_BODY_TOP)
    } else {
        CarContact::Inside
    }
}

/// World XZ rect of Pub 217's shell (outer wall faces), shared by traffic and drivable cars.
fn pub217_rect() -> (f32, f32, f32, f32) {
    let s = PUB217_SCALE;
    (
        PUB217_ANCHOR.x - 13.5 * s,
        PUB217_ANCHOR.x + 0.5 * s,
        PUB217_ANCHOR.z - 7.0 * s,
        PUB217_ANCHOR.z + 7.0 * s,
    )
}

/// Pushes a car's footprint out of Pub 217 along the shallowest axis (cars never enter the
/// bar, doorway included). Returns the outward normal when it had to push. The pub sits on its
/// own block clear of every lane, so this is only a safety net for cars Stan drives (or that
/// get knocked) onto the sidewalk.
fn push_car_out_of_pub217(car_tf: &mut Transform, yaw: f32) -> Option<Vec2> {
    let (min_x, max_x, min_z, max_z) = pub217_rect();
    let (s, c) = (yaw.sin().abs(), yaw.cos().abs());
    let hx = s * CAR_HALF_L + c * CAR_HALF_W + 0.2;
    let hz = c * CAR_HALF_L + s * CAR_HALF_W + 0.2;
    let p = car_tf.translation;
    let (l, r, b, f) = (p.x - (min_x - hx), (max_x + hx) - p.x, p.z - (min_z - hz), (max_z + hz) - p.z);
    if l <= 0.0 || r <= 0.0 || b <= 0.0 || f <= 0.0 {
        return None;
    }
    let m = l.min(r).min(b).min(f);
    Some(if m == l {
        car_tf.translation.x = min_x - hx;
        Vec2::NEG_X
    } else if m == r {
        car_tf.translation.x = max_x + hx;
        Vec2::X
    } else if m == b {
        car_tf.translation.z = min_z - hz;
        Vec2::NEG_Y
    } else {
        car_tf.translation.z = max_z + hz;
        Vec2::Y
    })
}

#[derive(Component)]
pub struct CityCarWheel {
    pub is_front: bool,
}

#[derive(Component)]
pub struct CityNpc {
    pub home_x: f32,
    pub home_z: f32,
    pub patrol_radius: f32,
    pub angle: f32,
    pub speed: f32,
    pub walk_phase: f32,
}

#[derive(Component)]
pub struct CityNpcLimb {
    pub side: f32,
    pub is_arm: bool,
}

#[derive(Component)]
pub struct LexieBoss {
    pub hp: f32,
    pub max_hp: f32,
    pub on_tower: bool,
    pub throw_cooldown: f32,
    pub chew_timer: f32,
    pub debris_timer: f32,
    pub hit_flash: f32,
    pub roar_cooldown: f32,
    /// `Player::swing_id` of the last saber swing that connected (one hit per swing).
    pub last_saber_swing: u32,
}

#[derive(Component)]
pub struct LexieThrowArm;

#[derive(Component)]
pub struct GarlicProjectile {
    pub velocity: Vec3,
    pub spin: f32,
    pub age: f32,
    /// Batted away by a perfect saber block: harmless, and fizzles without a stench cloud.
    pub deflected: bool,
}

#[derive(Component)]
pub struct GarlicStenchCloud {
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub radius: f32,
    pub damage_tick: f32,
}

#[derive(Component)]
pub struct CityTowerDebris {
    pub velocity: Vec3,
    pub spin: Vec3,
    pub age: f32,
}

// =========================================================================
// PART 1: POST-MOM JAPANESE ANIME THANK-YOU CUTSCENE
// =========================================================================

pub fn enter_mom_victory_cutscene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut stats: ResMut<PlayerStats>,
    mut player_q: Query<(&mut Transform, &mut Player)>,
    mut hud_q: Query<&mut Node, With<HudOverlay>>,
    old_level_q: Query<Entity, (Or<(With<LevelEntity>, With<Enemy>, With<FiberPuddle>)>, Without<Player>)>,
) {
    for mut node in &mut hud_q {
        node.display = Display::None;
    }
    stats.needs_city_char_select = true;
    crate::audio::mark_mom_defeated();
    crate::audio::mark_mom_defeated();
    crate::ui::save_game_checkpoint("city", &stats);
    for e in &old_level_q {
        commands.entity(e).despawn_recursive();
    }

    let start_t = take_cutscene_start_time();
    stats.cutscene_timer = if start_t > 0.0 { start_t } else { 0.0 };
    stats.cutscene_beat = 0;
    stats.boss_mode = false;
    stats.mom_quote.clear();
    stats.hint_text.clear();

    set_touch_playing(false);
    set_touch_boss_mode(false);
    play_cutscene_music();
    play_sfx("powerup");

    // Warm Japanese anime twilight sunset sky & cherry-blossom ambiance
    commands.insert_resource(ClearColor(Color::srgb(0.95, 0.46, 0.34)));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(1.0, 0.78, 0.62),
        brightness: 820.0,
    });

    if let Ok((mut p_tf, mut player)) = player_q.get_single_mut() {
        stats.player_scale = 3.2;
        p_tf.translation = Vec3::new(0.0, 0.0, 2.2);
        p_tf.scale = Vec3::splat(3.2);
        player.velocity = Vec3::ZERO;
        player.facing_yaw = 0.0; // Facing -Z straight toward Mom
        p_tf.rotation = Quat::from_rotation_y(0.0);
    }

    // Spawn Mom proudly smiling in anime lighting facing Stan (+Z)
    let mom_ent = spawn_mom_mesh_hierarchy(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(0.0, 0.0, -3.4),
        3.2,
    );
    commands.entity(mom_ent).insert((AnimeStageEntity, AnimeCutsceneMom));
    commands.entity(mom_ent).insert(
        Transform::from_xyz(0.0, 0.0, -3.4)
            .with_rotation(Quat::from_rotation_y(std::f32::consts::PI))
            .with_scale(Vec3::splat(3.2)),
    );

    // Anime Dojo / Shrine Courtyard Stage
    let wood_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.42, 0.24, 0.14),
        perceptual_roughness: 0.68,
        ..default()
    });
    let mat_runner = materials.add(StandardMaterial {
        base_color: Color::srgb(0.72, 0.12, 0.18),
        perceptual_roughness: 0.75,
        ..default()
    });
    let torii_red = materials.add(StandardMaterial {
        base_color: Color::srgb(0.88, 0.14, 0.12),
        perceptual_roughness: 0.35,
        ..default()
    });
    let sun_gold = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.85, 0.45),
        emissive: LinearRgba::rgb(4.2, 2.4, 0.6),
        unlit: true,
        ..default()
    });
    let speedline_gold = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.92, 0.65),
        emissive: LinearRgba::rgb(2.6, 1.8, 0.5),
        unlit: true,
        ..default()
    });
    let stone_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.52, 0.49),
        perceptual_roughness: 0.9,
        ..default()
    });
    let lantern_glow = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.86, 0.42),
        emissive: LinearRgba::rgb(3.5, 2.2, 0.6),
        ..default()
    });

    // Raised wooden shrine floor + crimson ceremonial runner
    commands.spawn((
        AnimeStageEntity,
        Mesh3d(meshes.add(Cuboid::new(26.0, 0.6, 26.0))),
        MeshMaterial3d(wood_mat),
        Transform::from_xyz(0.0, -0.3, -1.0),
    ));
    commands.spawn((
        AnimeStageEntity,
        Mesh3d(meshes.add(Cuboid::new(4.2, 0.06, 14.0))),
        MeshMaterial3d(mat_runner),
        Transform::from_xyz(0.0, 0.03, -1.0),
    ));

    // Anime Red Torii Gate framing the sunset horizon
    for sx in [-4.5_f32, 4.5_f32] {
        commands.spawn((
            AnimeStageEntity,
            Mesh3d(meshes.add(Cylinder::new(0.42, 10.5))),
            MeshMaterial3d(torii_red.clone()),
            Transform::from_xyz(sx, 5.2, -9.8),
        ));
    }
    commands.spawn((
        AnimeStageEntity,
        Mesh3d(meshes.add(Cuboid::new(13.2, 0.72, 0.95))),
        MeshMaterial3d(torii_red.clone()),
        Transform::from_xyz(0.0, 10.3, -9.8),
    ));
    commands.spawn((
        AnimeStageEntity,
        Mesh3d(meshes.add(Cuboid::new(10.8, 0.52, 0.70))),
        MeshMaterial3d(torii_red),
        Transform::from_xyz(0.0, 8.6, -9.8),
    ));

    // Giant glowing Anime Rising Sun halo + radial sunbeams
    commands.spawn((
        AnimeStageEntity,
        Mesh3d(meshes.add(Cylinder::new(6.8, 0.25))),
        MeshMaterial3d(sun_gold),
        Transform::from_xyz(0.0, 5.8, -14.2)
            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
    ));
    for i in 0..14 {
        let ang = (i as f32 / 14.0) * std::f32::consts::TAU;
        commands.spawn((
            AnimeStageEntity,
            Mesh3d(meshes.add(Cuboid::new(0.24, 15.0, 0.1))),
            MeshMaterial3d(speedline_gold.clone()),
            Transform::from_xyz(ang.cos() * 8.0, 5.8 + ang.sin() * 8.0, -14.0)
                .with_rotation(Quat::from_rotation_z(ang + std::f32::consts::FRAC_PI_2)),
        ));
    }

    // Traditional glowing stone lanterns flanking the ceremony
    for (lx, lz) in [(-5.4_f32, -2.0_f32), (5.4, -2.0), (-5.4, 3.6), (5.4, 3.6)] {
        commands.spawn((
            AnimeStageEntity,
            Mesh3d(meshes.add(Cuboid::new(0.9, 1.8, 0.9))),
            MeshMaterial3d(stone_mat.clone()),
            Transform::from_xyz(lx, 0.9, lz),
        ));
        commands.spawn((
            AnimeStageEntity,
            Mesh3d(meshes.add(Cuboid::new(0.68, 0.72, 0.68))),
            MeshMaterial3d(lantern_glow.clone()),
            Transform::from_xyz(lx, 2.1, lz),
        ));
        commands.spawn((
            AnimeStageEntity,
            Mesh3d(meshes.add(Cone::new(0.88, 0.65))),
            MeshMaterial3d(stone_mat.clone()),
            Transform::from_xyz(lx, 2.75, lz),
        ));
    }

    // 42 Drifting Anime Sakura Cherry Blossom Petals
    let petal_mesh = meshes.add(Cuboid::new(0.18, 0.03, 0.12));
    let petal_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.74, 0.86),
        emissive: LinearRgba::rgb(0.45, 0.18, 0.28),
        ..default()
    });
    for i in 0..42 {
        let fi = i as f32;
        let x = ((fi * 3.17).sin()) * 9.5;
        let y = 1.2 + ((fi * 1.91).rem_euclid(11.0));
        let z = -9.5 + ((fi * 2.43).cos()) * 6.5;
        commands.spawn((
            AnimeStageEntity,
            AnimeSakuraPetal {
                drift_phase: fi * 0.73,
                fall_speed: 1.15 + (i % 5) as f32 * 0.28,
                sway_amp: 0.7 + (i % 4) as f32 * 0.35,
            },
            Mesh3d(petal_mesh.clone()),
            MeshMaterial3d(petal_mat.clone()),
            Transform::from_xyz(x, y, z),
        ));
    }

    // Spawn stylized Anime Subtitle Banner UI
    commands
        .spawn((
            AnimeOverlay,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(14.0),
                justify_content: JustifyContent::Center,
                padding: UiRect::horizontal(Val::Px(14.0)),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    padding: UiRect::axes(Val::Px(20.0), Val::Px(14.0)),
                    width: Val::Percent(94.0),
                    max_width: Val::Px(780.0),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.10, 0.03, 0.08, 0.90)),
                BorderColor(Color::srgb(1.0, 0.65, 0.78)),
                BorderRadius::all(Val::Px(14.0)),
            ))
            .with_children(|card| {
                card.spawn((
                    Node {
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                ))
                .with_children(|hdr| {
                    hdr.spawn((
                        AnimeSpeakerText,
                        Text::new("STAN - HONORABLE SON"),
                        TextFont {
                            font_size: 15.0,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.78, 0.88)),
                    ));
                    hdr.spawn((
                        AnimeBeatBadge,
                        Text::new("ANIME FINALE | SCENE 1/4"),
                        TextFont {
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.86, 0.45)),
                    ));
                });
                card.spawn((
                    AnimeSubtitleText,
                    Text::new(
                        "\"Okasan... Forgive this foolish warrior for raising his tendy-blade against his own mother!\"",
                    ),
                    TextFont {
                        font_size: 20.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });
        });

    if stats.cutscene_timer < 1.0 {
        speak_voice(
            "Mother-san! Forgive this foolish warrior for raising his tendy blade against his own mother!",
            "stan",
        );
    }
}

pub fn update_mom_victory_cutscene(
    time: Res<Time>,
    mut stats: ResMut<PlayerStats>,
    mut next_state: crate::transition::SceneTransition,
    mut cam_q: Query<&mut Transform, (With<Camera3d>, Without<AnimeSakuraPetal>)>,
    mut petal_q: Query<(&mut Transform, &mut AnimeSakuraPetal), Without<Camera3d>>,
) {
    let dt = time.delta_secs().min(0.05);
    stats.cutscene_timer += dt;
    // Hold-to-skip: jump to the end so the normal end path below runs.
    let skipping = crate::skip::skipping();
    if skipping {
        stats.cutscene_timer = stats.cutscene_timer.max(22.8);
    }
    let t = stats.cutscene_timer;
    let total_elapsed = time.elapsed_secs();

    // Animate drifting sakura blossom petals across shrine
    for (mut tf, petal) in &mut petal_q {
        tf.translation.y -= petal.fall_speed * dt;
        tf.translation.x += (total_elapsed * 1.6 + petal.drift_phase).sin() * petal.sway_amp * dt;
        tf.translation.z += (total_elapsed * 1.1 + petal.drift_phase).cos() * 0.35 * dt;
        tf.rotate_y(2.4 * dt);
        tf.rotate_x(1.7 * dt);
        if tf.translation.y < 0.08 {
            tf.translation.y = 12.2;
        }
    }

    let (beat_idx, speech, role, sfx): (u8, &str, &str, Option<&str>) = if t < 5.0 {
        (0, "", "stan", None)
    } else if t < 11.2 {
        (
            1,
            "Arigatou gozaimasu, Mother! Every crispy tendy and every veggie lesson forged my Viking spirit! I am who I am today because of you!",
            "stan",
            Some("tendy"),
        )
    } else if t < 17.5 {
        (
            2,
            "Rise, my sweet Viking samurai, Stan. You have surpassed your mother! Now go to New York City. A massive titan named Lexie is destroying Manhattan with pickled garlic!",
            "mom",
            Some("powerup"),
        )
    } else {
        (
            3,
            "With honor restored and gratitude in his heart, Stan embarks for a vast new world. Next stop: New York City!",
            "narrator",
            Some("thunder"),
        )
    };

    // Hold on previous beat cleanly until TTS speech finishes!
    let mut active_beat = stats.cutscene_beat;
    if beat_idx > stats.cutscene_beat && (skipping || !is_voice_speaking()) {
        stats.cutscene_beat = beat_idx;
        active_beat = beat_idx;
        // A skip jumps straight to the last beat; its line and sting are not started.
        if !skipping {
            if let Some(name) = sfx {
                play_sfx(name);
            }
            if !speech.is_empty() {
                speak_voice(speech, role);
            }
        }
    } else if beat_idx > stats.cutscene_beat && is_voice_speaking() {
        stats.cutscene_timer -= dt;
    }

    // Update camera with dramatic anime framing per beat
    if let Ok(mut cam_tf) = cam_q.get_single_mut() {
        let (eye, target) = match active_beat {
            0 => (
                Vec3::new(6.2, 3.8 + (t * 0.12).min(0.6), 4.4),
                Vec3::new(0.0, 3.0, -1.0),
            ),
            1 => {
                // Heroic anime three-quarter shot framing bowing Stan & proud Mom beneath Torii & Rising Sun
                let u = ((t - 5.0) / 5.5).clamp(0.0, 1.0);
                (
                    Vec3::new(9.2 - u * 1.4, 3.8, 2.6 - u * 1.0),
                    Vec3::new(0.0, 2.8, -0.8),
                )
            }
            2 => (
                // Dramatic anime close-up of proud 3.2x Mom beneath the Torii gate & Rising Sun
                Vec3::new(2.2, 5.2, 1.8),
                Vec3::new(0.0, 5.1, -3.4),
            ),
            _ => {
                // Slow anime pull-up toward sunset sky before Manhattan transition
                let u = ((t - 17.5) / 4.0).clamp(0.0, 1.0);
                (
                    Vec3::new(0.0, 5.2 + u * 6.5, 10.5 + u * 5.0),
                    Vec3::new(0.0, 4.2 + u * 2.5, -6.0),
                )
            }
        };
        cam_tf.translation = cam_tf.translation.lerp(eye, (dt * 4.5).min(1.0));
        cam_tf.look_at(target, Vec3::Y);
    }

    // Wait: subtitle text updates run in a tiny helper system or inline — keep mutable UI updates separate!
    if active_beat >= 3 && t >= 22.8 && (skipping || !is_voice_speaking()) {
        next_state.set(GameState::CityIntroCutscene);
    }
}

pub fn update_mom_victory_subtitles(
    stats: Res<PlayerStats>,
    mut spk_q: Query<&mut Text, With<AnimeSpeakerText>>,
    mut badge_q: Query<&mut Text, (With<AnimeBeatBadge>, Without<AnimeSpeakerText>)>,
    mut sub_q: Query<
        &mut Text,
        (
            With<AnimeSubtitleText>,
            Without<AnimeSpeakerText>,
            Without<AnimeBeatBadge>,
        ),
    >,
) {
    let (speaker, badge, line) = match stats.cutscene_beat {
        0 => (
            "STAN - HONORABLE SON",
            "ANIME FINALE | SCENE 1/4",
            "\"Okasan... Forgive this foolish warrior for raising his tendy-blade against the one who gave him life!\"",
        ),
        1 => (
            "STAN - DEEPEST BOW",
            "ANIME FINALE | SCENE 2/4",
            "\"Arigatou gozaimasu, Mother! Every crispy tendy, every veggie warning, forged my Viking spirit! I am who I am today because of you!\"",
        ),
        2 => (
            "MOM - PROUD SENSEI",
            "ANIME FINALE | SCENE 3/4",
            "\"Rise, my sweet Viking samurai. You have surpassed your old mother! Now fly to New York... an enormous menace named Lexie is wrecking Manhattan!\"",
        ),
        _ => (
            "NARRATOR - NEXT CHAPTER",
            "ANIME FINALE | SCENE 4/4",
            "\"With tears of honor and fifty Manhattan tendies calling his name, Stan leaps into a new world... NEXT STAGE: NEW YORK CITY!\"",
        ),
    };
    if let Ok(mut t) = spk_q.get_single_mut() {
        t.0 = speaker.to_string();
    }
    if let Ok(mut t) = badge_q.get_single_mut() {
        t.0 = badge.to_string();
    }
    if let Ok(mut t) = sub_q.get_single_mut() {
        t.0 = line.to_string();
    }
}

pub fn exit_mom_victory_cutscene(
    mut commands: Commands,
    stage_q: Query<Entity, With<AnimeStageEntity>>,
    ui_q: Query<Entity, With<AnimeOverlay>>,
    cosmetic_q: Query<Entity, With<HeroCosmetic>>,
    refl_q: Query<Entity, With<CutsceneEyeReflection>>,
) {
    for e in &stage_q {
        commands.entity(e).despawn_recursive();
    }
    for e in &ui_q {
        commands.entity(e).despawn_recursive();
    }
    for e in &cosmetic_q {
        commands.entity(e).despawn_recursive();
    }
    for e in &refl_q {
        commands.entity(e).despawn_recursive();
    }
}

// =========================================================================
// PART 2: NEW YORK CITY LEVEL GENERATION & LEXIE COLOSSAL BOSS MODEL
// =========================================================================

pub(crate) fn is_clear_of_skyscrapers(x: f32, z: f32, margin: f32) -> bool {
    // The riverfront Ferris wheel's plaza and kiosks (ferris.rs).
    if crate::ferris::blocks_spot(x, z, margin) {
        return false;
    }
    // Five Seasons Park (terraces and the avenue stubs it absorbed).
    if crate::skatepark::in_bigpark(x, z, margin) {
        return false;
    }
    // Pub 217 (its own low building on the (-23, 23) block)
    let (pmin_x, pmax_x, pmin_z, pmax_z) = pub217_rect();
    if x > pmin_x - margin && x < pmax_x + margin && z > pmin_z - margin && z < pmax_z + margin {
        return false;
    }
    // Empire Citadel Tower footprint: 21x21 centered at (0, -82)
    if (x - CITADEL_POS.x).abs() < 11.2 + margin && (z - CITADEL_POS.z).abs() < 11.2 + margin {
        return false;
    }
    // Inner 4x4 city grid blocks (17m buildings at bx, bz in [-69, -23, 23, 69])
    for &bx in &BLOCK_COORDS {
        for &bz in &BLOCK_COORDS {
            if (x - bx).abs() < 9.0 + margin && (z - bz).abs() < 9.0 + margin {
                return false;
            }
        }
    }
    // Outer skyline blocks at -114 / +114
    for &bx in &[-114.0_f32, -69.0, -23.0, 23.0, 69.0, 114.0] {
        for &bz in &[-114.0_f32, 114.0] {
            if (x - bx).abs() < 9.2 + margin && (z - bz).abs() < 9.2 + margin {
                return false;
            }
        }
    }
    for &bz in &BLOCK_COORDS {
        for &bx in &[-114.0_f32, 114.0] {
            if (x - bx).abs() < 9.2 + margin && (z - bz).abs() < 9.2 + margin {
                return false;
            }
        }
    }
    true
}

pub fn spawn_nyc_world_if_needed(
    commands: &mut Commands,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    existing_city_q: &Query<Entity, With<CityWorldMarker>>,
    old_level_q: &Query<Entity, (Or<(With<LevelEntity>, With<Enemy>, With<FiberPuddle>)>, Without<Player>)>,
) {
    if !existing_city_q.is_empty() {
        return;
    }
    for e in old_level_q {
        commands.entity(e).despawn_recursive();
    }

    commands.insert_resource(ClearColor(Color::srgb(0.03, 0.04, 0.09)));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.75, 0.80, 0.95),
        brightness: 440.0,
    });

    // Unit box with UVs in world metres for a given transform scale, so detail textures tile at
    // their real size instead of stretching across non-uniformly scaled boxes.
    fn tiled_unit(meshes: &mut Assets<Mesh>, world: Vec3, tile: f32) -> Handle<Mesh> {
        meshes.add(crate::gfx::tiled_box(Vec3::ONE, world, Vec2::splat(tile), 0.0, None))
    }
    let unit_box = meshes.add(Cuboid::from_size(Vec3::ONE));
    let unit_cyl = meshes.add(Cylinder::new(1.0, 1.0));
    let unit_sphere = meshes.add(Sphere::new(1.0));
    let unit_cone = meshes.add(Cone::new(1.0, 1.0));

    let asphalt = materials.add(crate::gfx::city_textured(StandardMaterial {
        base_color: Color::srgb(0.22, 0.23, 0.25),
        perceptual_roughness: 0.86,
        ..default()
    }, crate::gfx::CityTex::Asphalt));
    let sidewalk_mat = materials.add(crate::gfx::city_textured(StandardMaterial {
        base_color: Color::srgb(0.64, 0.62, 0.58),
        perceptual_roughness: 0.88,
        ..default()
    }, crate::gfx::CityTex::Pavers));
    let brick_walk_mat = materials.add(crate::gfx::city_textured(StandardMaterial {
        base_color: Color::srgb(0.52, 0.27, 0.22),
        perceptual_roughness: 0.90,
        ..default()
    }, crate::gfx::CityTex::Brick));
    let bike_lane_mat = materials.add(crate::gfx::city_textured(StandardMaterial {
        base_color: Color::srgb(0.24, 0.56, 0.30),
        perceptual_roughness: 0.82,
        ..default()
    }, crate::gfx::CityTex::Paint));
    let yellow_stripe = materials.add(crate::gfx::city_textured(StandardMaterial {
        base_color: Color::srgb(0.95, 0.74, 0.14),
        perceptual_roughness: 0.55,
        ..default()
    }, crate::gfx::CityTex::Paint));
    let white_stripe = materials.add(crate::gfx::city_textured(StandardMaterial {
        base_color: Color::srgb(0.92, 0.92, 0.92),
        perceptual_roughness: 0.55,
        ..default()
    }, crate::gfx::CityTex::Paint));

    // Solid Manhattan street slab covering full 270m x 270m grid (top_y = 0.0)
    commands.spawn((
        LevelEntity,
        CityWorldMarker,
        Mesh3d(meshes.add(crate::gfx::tiled_box(
            Vec3::new(CITY_HALF_EXTENT * 2.0, 1.0, CITY_HALF_EXTENT * 2.0),
            Vec3::new(CITY_HALF_EXTENT * 2.0, 1.0, CITY_HALF_EXTENT * 2.0),
            Vec2::splat(12.0),
            -1.0,
            None,
        ))),
        MeshMaterial3d(asphalt.clone()),
        Transform::from_xyz(0.0, -0.5, 0.0),
        PlatformBox {
            half_extents: Vec3::new(CITY_HALF_EXTENT, 0.5, CITY_HALF_EXTENT),
        },
    ));

    let stripe_meshes = [
        tiled_unit(meshes, Vec3::new(0.22, 0.02, CITY_HALF_EXTENT * 1.96), 1.5),
        tiled_unit(meshes, Vec3::new(CITY_HALF_EXTENT * 1.96, 0.02, 0.22), 1.5),
        tiled_unit(meshes, Vec3::new(0.18, 0.02, 4.2), 1.5),
        tiled_unit(meshes, Vec3::new(4.2, 0.02, 0.18), 1.5),
    ];
    // Yellow Avenue double centerlines + White cross-street dashes
    for &r in &ROAD_COORDS {
        for off in [-0.28_f32, 0.28_f32] {
            commands.spawn((
                LevelEntity,
                CityWorldMarker,
                Mesh3d(stripe_meshes[0].clone()),
                MeshMaterial3d(yellow_stripe.clone()),
                Transform::from_xyz(r + off, 0.015, 0.0)
                    .with_scale(Vec3::new(0.22, 0.02, CITY_HALF_EXTENT * 1.96)),
            ));
            commands.spawn((
                LevelEntity,
                CityWorldMarker,
                Mesh3d(stripe_meshes[1].clone()),
                MeshMaterial3d(yellow_stripe.clone()),
                Transform::from_xyz(0.0, 0.015, r + off)
                    .with_scale(Vec3::new(CITY_HALF_EXTENT * 1.96, 0.02, 0.22)),
            ));
        }
        // Dashed lane dividers along avenues & streets
        for d in (-120..=120).step_by(14) {
            for side in [-5.0_f32, 5.0_f32] {
                commands.spawn((
                    LevelEntity,
                    CityWorldMarker,
                    Mesh3d(stripe_meshes[2].clone()),
                    MeshMaterial3d(white_stripe.clone()),
                    Transform::from_xyz(r + side, 0.016, d as f32)
                        .with_scale(Vec3::new(0.18, 0.02, 4.2)),
                ));
                commands.spawn((
                    LevelEntity,
                    CityWorldMarker,
                    Mesh3d(stripe_meshes[3].clone()),
                    MeshMaterial3d(white_stripe.clone()),
                    Transform::from_xyz(d as f32, 0.016, r + side)
                        .with_scale(Vec3::new(4.2, 0.02, 0.18)),
                ));
            }
        }
    }

    // Architectural palette for Historic Downtown Cedar Rapids (Red Brick, Brownstone, Limestone, Glass)
    let building_mats = [
        materials.add(crate::gfx::city_textured(StandardMaterial {
            base_color: Color::srgb(0.48, 0.23, 0.18),
            perceptual_roughness: 0.82,
            ..default()
        }, crate::gfx::CityTex::FacadeBrick)),
        materials.add(crate::gfx::city_textured(StandardMaterial {
            base_color: Color::srgb(0.58, 0.46, 0.36),
            perceptual_roughness: 0.78,
            ..default()
        }, crate::gfx::CityTex::FacadeConcrete)),
        materials.add(crate::gfx::city_textured(StandardMaterial {
            base_color: Color::srgb(0.72, 0.68, 0.60),
            perceptual_roughness: 0.72,
            ..default()
        }, crate::gfx::CityTex::FacadeConcrete)),
        materials.add(crate::gfx::city_textured(StandardMaterial {
            base_color: Color::srgb(0.36, 0.22, 0.20),
            perceptual_roughness: 0.84,
            ..default()
        }, crate::gfx::CityTex::FacadeBrick)),
        materials.add(crate::gfx::city_textured(StandardMaterial {
            base_color: Color::srgb(0.32, 0.44, 0.50),
            metallic: 0.45,
            perceptual_roughness: 0.28,
            ..default()
        }, crate::gfx::CityTex::FacadeGlass)),
    ];
    let window_glow = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.20, 0.26),
        metallic: 0.55,
        perceptual_roughness: 0.22,
        ..default()
    });
    let billboard_mats = [
        materials.add(crate::gfx::city_textured(StandardMaterial {
            base_color: Color::srgb(0.86, 0.76, 0.52),
            perceptual_roughness: 0.6,
            ..default()
        }, crate::gfx::CityTex::Billboard)),
        materials.add(crate::gfx::city_textured(StandardMaterial {
            base_color: Color::srgb(0.16, 0.44, 0.26),
            perceptual_roughness: 0.7,
            ..default()
        }, crate::gfx::CityTex::Billboard)),
        materials.add(crate::gfx::city_textured(StandardMaterial {
            base_color: Color::srgb(0.68, 0.22, 0.18),
            perceptual_roughness: 0.7,
            ..default()
        }, crate::gfx::CityTex::Billboard)),
    ];

    // Spawn Downtown Cedar Rapids blocks + Cedar River & Mays Island on the West (x = -114) + Greene Square Park at (69, 69)
    let water_mat = materials.add(crate::gfx::city_textured(StandardMaterial {
        base_color: Color::srgba(0.05, 0.22, 0.42, 0.92),
        metallic: 0.35,
        perceptual_roughness: 0.18,
        ..default()
    }, crate::gfx::CityTex::Water));
    let pub_brick_mat = materials.add(crate::gfx::city_textured(StandardMaterial {
        base_color: Color::srgb(0.38, 0.14, 0.12),
        perceptual_roughness: 0.78,
        ..default()
    }, crate::gfx::CityTex::Brick));
    let pub_sign_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.78, 0.16),
        emissive: LinearRgba::rgb(2.6, 1.6, 0.25),
        ..default()
    });

    // Cedar River along West Downtown (x = -114) + Mays Island Courthouse + 1st/2nd/3rd Ave Bridges
    commands.spawn((
        LevelEntity,
        CityWorldMarker,
        Mesh3d(tiled_unit(meshes, Vec3::new(34.0, 0.06, 270.0), 10.0)),
        MeshMaterial3d(water_mat),
        Transform::from_xyz(-114.0, 0.03, 0.0).with_scale(Vec3::new(34.0, 0.06, 270.0)),
    ));
    // Mays Island (Linn County Courthouse / City Hall)
    commands.spawn((
        LevelEntity,
        CityWorldMarker,
        Mesh3d(tiled_unit(meshes, Vec3::new(18.0, 0.24, 34.0), 3.0)),
        MeshMaterial3d(sidewalk_mat.clone()),
        Transform::from_xyz(-114.0, 0.12, 0.0).with_scale(Vec3::new(18.0, 0.24, 34.0)),
    ));
    let bridge_mesh = tiled_unit(meshes, Vec3::new(36.0, 0.18, 12.0), 12.0);
    for &bz in &[-46.0_f32, 0.0, 46.0] {
        commands.spawn((
            LevelEntity,
            CityWorldMarker,
            Mesh3d(bridge_mesh.clone()),
            MeshMaterial3d(asphalt.clone()),
            Transform::from_xyz(-114.0, 0.16, bz).with_scale(Vec3::new(36.0, 0.18, 12.0)),
        ));
    }

    // Green Bike Lanes + Brick Sidewalk Trim + Leafy Street Trees along 3rd St SE (x = 0)
    let bike_lane_mesh = tiled_unit(meshes, Vec3::new(1.8, 0.02, CITY_HALF_EXTENT * 1.9), 3.0);
    let brick_walk_mesh = tiled_unit(meshes, Vec3::new(2.2, 0.06, CITY_HALF_EXTENT * 1.9), 1.2);
    for sx in [-5.8_f32, 5.8_f32] {
        commands.spawn((
            LevelEntity,
            CityWorldMarker,
            Mesh3d(bike_lane_mesh.clone()),
            MeshMaterial3d(bike_lane_mat.clone()),
            Transform::from_xyz(sx, 0.014, 0.0).with_scale(Vec3::new(1.8, 0.02, CITY_HALF_EXTENT * 1.9)),
        ));
        commands.spawn((
            LevelEntity,
            CityWorldMarker,
            Mesh3d(brick_walk_mesh.clone()),
            MeshMaterial3d(brick_walk_mat.clone()),
            Transform::from_xyz(sx * 1.45, 0.03, 0.0).with_scale(Vec3::new(2.2, 0.06, CITY_HALF_EXTENT * 1.9)),
        ));
    }
    let trunk_mat = materials.add(crate::gfx::city_textured(StandardMaterial {
        base_color: Color::srgb(0.32, 0.22, 0.14),
        perceptual_roughness: 0.9,
        ..default()
    }, crate::gfx::CityTex::Bark));
    let leaf_mat = materials.add(crate::gfx::city_textured(StandardMaterial {
        base_color: Color::srgb(0.20, 0.52, 0.22),
        perceptual_roughness: 0.85,
        uv_transform: bevy::math::Affine2::from_scale(Vec2::new(4.0, 2.0)),
        ..default()
    }, crate::gfx::CityTex::Foliage));
    let bench_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.10, 0.10, 0.12),
        metallic: 0.4,
        ..default()
    });
    for tz in STREET_TREE_Z {
        for tx in [-STREET_TREE_X, STREET_TREE_X] {
            commands.spawn((
                LevelEntity,
                CityWorldMarker,
                Mesh3d(unit_cyl.clone()),
                MeshMaterial3d(trunk_mat.clone()),
                Transform::from_xyz(tx, 1.5, tz).with_scale(Vec3::new(0.22, 3.0, 0.22)),
            ));
            commands.spawn((
                LevelEntity,
                CityWorldMarker,
                Mesh3d(unit_sphere.clone()),
                MeshMaterial3d(leaf_mat.clone()),
                Transform::from_xyz(tx, 3.6, tz).with_scale(Vec3::new(1.8, 1.4, 1.8)),
            ));
            if (tz as i32) % 24 == 0 {
                commands.spawn((
                    LevelEntity,
                    CityWorldMarker,
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(bench_mat.clone()),
                    Transform::from_xyz(tx * 1.08, 0.35, tz + 3.0).with_scale(Vec3::new(0.6, 0.5, 1.8)),
                ));
            }
        }
    }

    // Walk-In Pub 217 (217 3rd St SE) on its own block west of 3rd St SE (see `PUB217_ANCHOR`).
    // Open doorway on the East face, onto the 3rd St SE sidewalk, so Stan walks straight in to find Mike!
    let pub_cream_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.84, 0.80, 0.72),
        perceptual_roughness: 0.78,
        ..default()
    });
    let pub_black_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.08, 0.10),
        ..default()
    });
    let pub_navy_mat = materials.add(crate::gfx::city_textured(StandardMaterial {
        base_color: Color::srgb(0.11, 0.14, 0.22),
        perceptual_roughness: 0.75,
        ..default()
    }, crate::gfx::CityTex::Brick));
    let pub_wood_mat = materials.add(crate::gfx::city_textured(StandardMaterial {
        base_color: Color::srgb(0.24, 0.11, 0.07),
        metallic: 0.15,
        perceptual_roughness: 0.35,
        ..default()
    }, crate::gfx::CityTex::Wood));
    let pub_floor_mat = materials.add(crate::gfx::city_textured(StandardMaterial {
        base_color: Color::srgb(0.32, 0.22, 0.14),
        perceptual_roughness: 0.72,
        ..default()
    }, crate::gfx::CityTex::Wood));
    let cooler_glow_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.92, 0.96, 1.0),
        emissive: LinearRgba::rgb(1.3, 1.4, 1.6),
        ..default()
    });
    let tv_glow_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.78, 1.0),
        emissive: LinearRgba::rgb(0.6, 1.3, 1.8),
        ..default()
    });
    let pendant_glow_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.86, 0.55),
        emissive: LinearRgba::rgb(2.4, 1.8, 0.8),
        ..default()
    });

    // Warm streetlights & Pub 217 entrance + interior lights so 217 pops clearly at night
    let pa = PUB217_ANCHOR;
    for (lx, ly, lz, intensity, range) in [
        (pa.x + 1.55, 4.5_f32, pa.z, 42000.0_f32, 24.0_f32),
        (pa.x - 6.75, 5.2, pa.z, 52000.0, 26.0),
        (0.0, 7.5, pa.z + 4.0, 38000.0, 32.0),
    ] {
        commands.spawn((
            LevelEntity,
            CityWorldMarker,
            PointLight {
                color: Color::srgb(1.0, 0.86, 0.62),
                intensity,
                range,
                ..default()
            },
            Transform::from_xyz(lx, ly, lz),
        ));
    }

    let pub_floor_mesh = tiled_unit(meshes, Vec3::new(13.5, 0.08, 14.0), 2.0);
    let beer_glow_mesh = meshes.add(Torus::new(1.5, 1.75));
    let beer_glow_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.72, 0.18, 0.55),
        emissive: LinearRgba::rgb(2.2, 1.4, 0.25),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    // Pub 217 Visual Root (scales around East doorway anchor PUB217_ANCHOR so Stan always fits inside!)
    commands
        .spawn((
            LevelEntity,
            CityWorldMarker,
            Pub217Root,
            crate::gfx::CityOccluderGroup,
            Transform::from_translation(PUB217_ANCHOR).with_scale(Vec3::splat(PUB217_SCALE)),
        ))
        .with_children(|pub_root| {
            // Floor Slab
            pub_root.spawn((
                Mesh3d(pub_floor_mesh),
                MeshMaterial3d(pub_floor_mat),
                Transform::from_translation(Vec3::new(-18.25, 0.04, 34.0) - PUB217_LAYOUT_ORIGIN)
                    .with_scale(Vec3::new(13.5, 0.08, 14.0)),
            ));
            // Shell Walls (West, North, South, East-North, East-South leaving open front door at z = 34)
            for (wx, wy, wz, sx, sy, sz, mat) in [
                (-24.75_f32, 3.2_f32, 34.0_f32, 0.5_f32, 6.4_f32, 14.0_f32, pub_navy_mat.clone()),
                (-18.25, 3.2, 27.25, 13.5, 6.4, 0.5, pub_brick_mat.clone()),
                (-18.25, 3.2, 40.75, 13.5, 6.4, 0.5, pub_brick_mat.clone()),
                (-11.75, 3.2, 29.5, 0.5, 6.4, 4.5, pub_brick_mat.clone()),
                (-11.75, 3.2, 38.5, 0.5, 6.4, 4.5, pub_brick_mat.clone()),
                (-11.75, 5.4, 34.0, 0.5, 2.0, 4.6, pub_cream_mat.clone()),
                (-18.25, 6.5, 34.0, 13.5, 0.35, 14.0, pub_black_mat.clone()),
            ] {
                pub_root.spawn((
                    Mesh3d(tiled_unit(meshes, Vec3::new(sx, sy, sz), 1.2)),
                    MeshMaterial3d(mat),
                    Transform::from_translation(Vec3::new(wx, wy, wz) - PUB217_LAYOUT_ORIGIN)
                        .with_scale(Vec3::new(sx, sy, sz)),
                ));
            }
            // Glowing PUB 217 blade sign + exterior cream trim
            pub_root.spawn((
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(pub_cream_mat),
                Transform::from_translation(Vec3::new(-11.42, 1.8, 38.5) - PUB217_LAYOUT_ORIGIN)
                    .with_scale(Vec3::new(0.16, 3.4, 4.2)),
            ));
            pub_root.spawn((
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(pub_black_mat.clone()),
                Transform::from_translation(Vec3::new(-10.5, 4.2, 37.0) - PUB217_LAYOUT_ORIGIN)
                    .with_scale(Vec3::new(1.6, 2.8, 0.45)),
            ));
            pub_root.spawn((
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(pub_sign_mat.clone()),
                Transform::from_translation(Vec3::new(-10.5, 4.2, 37.0) - PUB217_LAYOUT_ORIGIN)
                    .with_scale(Vec3::new(1.2, 2.3, 0.52)),
            ));
            // Bold "2 1 7" 3D digits on both faces of the blade sign + glowing doorway arch
            for z_face in [-0.28_f32, 0.28_f32] {
                let sign_c = Vec3::new(-10.5, 4.2, 37.0 + z_face) - PUB217_LAYOUT_ORIGIN;
                // "2" (top y +0.62)
                for (dx, dy, sx, sy) in [
                    (0.0_f32, 0.78_f32, 0.50_f32, 0.11_f32),
                    (0.20, 0.65, 0.11, 0.22),
                    (0.0, 0.54, 0.50, 0.11),
                    (-0.20, 0.43, 0.11, 0.22),
                    (0.0, 0.32, 0.50, 0.11),
                    // "1" (middle)
                    (0.0, -0.02, 0.12, 0.44),
                    // "7" (bottom y -0.60)
                    (0.0, -0.44, 0.50, 0.11),
                    (0.18, -0.64, 0.11, 0.42),
                ] {
                    pub_root.spawn((
                        Mesh3d(unit_box.clone()),
                        MeshMaterial3d(pub_black_mat.clone()),
                        Transform::from_translation(sign_c + Vec3::new(dx, dy, 0.0))
                            .with_scale(Vec3::new(sx, sy, 0.06)),
                    ));
                }
            }
            // Glowing doorway frame pillars + header bar so the entrance pops at night
            for dz in [-2.35_f32, 2.35_f32] {
                pub_root.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(pub_sign_mat.clone()),
                    Transform::from_translation(Vec3::new(-11.42, 2.2, 34.0 + dz) - PUB217_LAYOUT_ORIGIN)
                        .with_scale(Vec3::new(0.18, 4.4, 0.18)),
                ));
            }
            pub_root.spawn((
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(pub_sign_mat.clone()),
                Transform::from_translation(Vec3::new(-11.42, 4.45, 34.0) - PUB217_LAYOUT_ORIGIN)
                    .with_scale(Vec3::new(0.22, 0.22, 4.9)),
            ));
            // L-shaped mahogany bar counter + 4 tall wooden barstools
            pub_root.spawn((
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(pub_wood_mat.clone()),
                Transform::from_translation(Vec3::new(-19.5, 0.62, 32.5) - PUB217_LAYOUT_ORIGIN)
                    .with_scale(Vec3::new(1.1, 1.24, 6.4)),
            ));
            pub_root.spawn((
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(pub_wood_mat.clone()),
                Transform::from_translation(Vec3::new(-21.2, 0.62, 35.2) - PUB217_LAYOUT_ORIGIN)
                    .with_scale(Vec3::new(3.8, 1.24, 1.1)),
            ));
            for sz in [30.2_f32, 31.6, 33.0, 34.4] {
                pub_root
                    .spawn(Transform::from_translation(Vec3::new(-18.3, 0.0, sz) - PUB217_LAYOUT_ORIGIN))
                    .with_children(|s| {
                        s.spawn((
                            Mesh3d(unit_box.clone()),
                            MeshMaterial3d(pub_wood_mat.clone()),
                            Transform::from_xyz(0.0, 0.82, 0.0).with_scale(Vec3::new(0.55, 0.08, 0.55)),
                        ));
                        s.spawn((
                            Mesh3d(unit_box.clone()),
                            MeshMaterial3d(pub_wood_mat.clone()),
                            Transform::from_xyz(0.24, 1.22, 0.0).with_scale(Vec3::new(0.08, 0.78, 0.50)),
                        ));
                        for (lx, lz) in [(-0.22, -0.22), (0.22, -0.22), (-0.22, 0.22), (0.22, 0.22)] {
                            s.spawn((
                                Mesh3d(unit_box.clone()),
                                MeshMaterial3d(pub_wood_mat.clone()),
                                Transform::from_xyz(lx, 0.40, lz).with_scale(Vec3::new(0.07, 0.80, 0.07)),
                            ));
                        }
                    });
            }
            // Hanging industrial pendant lights over the bar
            for pz in [29.8_f32, 32.4, 35.0, 37.6] {
                pub_root.spawn((
                    Mesh3d(unit_cyl.clone()),
                    MeshMaterial3d(pub_black_mat.clone()),
                    Transform::from_translation(Vec3::new(-18.6, 5.2, pz) - PUB217_LAYOUT_ORIGIN)
                        .with_scale(Vec3::new(0.03, 2.2, 0.03)),
                ));
                pub_root.spawn((
                    Mesh3d(unit_cone.clone()),
                    MeshMaterial3d(pub_black_mat.clone()),
                    Transform::from_translation(Vec3::new(-18.6, 4.0, pz) - PUB217_LAYOUT_ORIGIN)
                        .with_scale(Vec3::new(0.45, 0.45, 0.45)),
                ));
                pub_root.spawn((
                    Mesh3d(unit_sphere.clone()),
                    MeshMaterial3d(pendant_glow_mat.clone()),
                    Transform::from_translation(Vec3::new(-18.6, 3.78, pz) - PUB217_LAYOUT_ORIGIN)
                        .with_scale(Vec3::splat(0.14)),
                ));
            }
            // Back bar coolers, TVs, tap tower, POS terminal
            for cz in [29.4_f32, 33.4] {
                pub_root.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(cooler_glow_mat.clone()),
                    Transform::from_translation(Vec3::new(-24.2, 1.35, cz) - PUB217_LAYOUT_ORIGIN)
                        .with_scale(Vec3::new(0.55, 2.1, 2.2)),
                ));
                pub_root.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(tv_glow_mat.clone()),
                    Transform::from_translation(Vec3::new(-24.3, 3.35, cz) - PUB217_LAYOUT_ORIGIN)
                        .with_scale(Vec3::new(0.16, 1.15, 2.0)),
                ));
            }
            pub_root.spawn((
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(tv_glow_mat),
                Transform::from_translation(Vec3::new(-24.3, 3.35, 31.4) - PUB217_LAYOUT_ORIGIN)
                    .with_scale(Vec3::new(0.16, 0.95, 1.5)),
            ));
            pub_root.spawn((
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(sidewalk_mat.clone()),
                Transform::from_translation(Vec3::new(-22.2, 1.45, 31.4) - PUB217_LAYOUT_ORIGIN)
                    .with_scale(Vec3::new(0.28, 0.65, 1.8)),
            ));
            pub_root.spawn((
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(cooler_glow_mat),
                Transform::from_translation(Vec3::new(-20.6, 1.55, 34.4) - PUB217_LAYOUT_ORIGIN)
                    .with_scale(Vec3::new(0.18, 0.42, 0.55)),
            ));
            // Left corner: Table with 7-tier Beer Tower Pyramid reaching nearly to the ceiling!
            pub_root.spawn((
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(pub_wood_mat.clone()),
                Transform::from_translation(Vec3::new(-22.6, 0.55, 38.6) - PUB217_LAYOUT_ORIGIN)
                    .with_scale(Vec3::new(2.2, 1.1, 2.2)),
            ));
            // Stacked pyramid of "Bash Lyte" tall boys on the table top, plus the hero can.
            crate::beer_fx::spawn_pub_beer_tower(
                pub_root,
                meshes,
                materials,
                Vec3::new(-22.6, 1.1, 38.6) - PUB217_LAYOUT_ORIGIN,
            );
            // Amber floor ring around the beer tower table while Stan can drink there.
            pub_root.spawn((
                BeerTowerGlow,
                Mesh3d(beer_glow_mesh),
                MeshMaterial3d(beer_glow_mat),
                Transform::from_translation(BEER_TOWER_PUB_POS + Vec3::Y * 0.06 - PUB217_LAYOUT_ORIGIN)
                    .with_scale(Vec3::new(1.0, 0.15, 1.0)),
                Visibility::Hidden,
            ));
        });

    // PlatformBox colliders for Pub 217 walls, bar, and beer-pyramid table (fixed pub scale)
    for (wx, wy, wz, sx, sy, sz) in [
        (-24.75_f32, 3.2_f32, 34.0_f32, 0.5_f32, 6.4_f32, 14.0_f32),
        (-18.25, 3.2, 27.25, 13.5, 6.4, 0.5),
        (-18.25, 3.2, 40.75, 13.5, 6.4, 0.5),
        (-11.75, 3.2, 29.5, 0.5, 6.4, 4.5),
        (-11.75, 3.2, 38.5, 0.5, 6.4, 4.5),
        (-11.75, 5.4, 34.0, 0.5, 2.0, 4.6),
        (-18.25, 6.5, 34.0, 13.5, 0.35, 14.0),
        (-19.5, 0.62, 32.5, 1.1, 1.24, 6.4),
        (-21.2, 0.62, 35.2, 3.8, 1.24, 1.1),
        (-22.6, 0.55, 38.6, 2.2, 1.1, 2.2),
    ] {
        let base_local = Vec3::new(wx, wy, wz) - PUB217_LAYOUT_ORIGIN;
        let base_half = Vec3::new(sx * 0.5, sy * 0.5, sz * 0.5);
        commands.spawn((
            LevelEntity,
            CityWorldMarker,
            Transform::from_translation(PUB217_ANCHOR + base_local * PUB217_SCALE),
            PlatformBox {
                half_extents: base_half * PUB217_SCALE,
            },
        ));
    }
    // Doorway plug: parked underground until Stan outgrows the door (`update_pub217_door`).
    commands.spawn((
        LevelEntity,
        CityWorldMarker,
        Pub217DoorPlug,
        Transform::from_translation(PUB217_ANCHOR + Vec3::new(0.0, -50.0, 0.0)),
        PlatformBox {
            half_extents: Vec3::new(0.25, 0.1, 0.1),
        },
    ));

    let block_walk_mesh = tiled_unit(meshes, Vec3::new(26.5, 0.08, 26.5), 3.0);
    let kerb_ring = meshes.add(crate::gfx::kerb_ring_mesh(26.54, 0.32, 0.11, 1.5));
    let kerb_mat = materials.add(crate::gfx::city_textured(StandardMaterial {
        base_color: Color::srgb(0.70, 0.69, 0.66),
        perceptual_roughness: 0.85,
        ..default()
    }, crate::gfx::CityTex::Pavers));
    let roof_mats = building_mats.clone().map(|h| {
        let c = materials.get(&h).map(|m| m.base_color).unwrap_or(Color::WHITE);
        materials.add(crate::gfx::city_roof_material(c))
    });
    // Roof membrane caps per footprint (17 m inner, 18 m outer): [main roof, penthouse roof].
    let roof_caps = [17.0_f32, 18.0].map(|fp| {
        [
            tiled_unit(meshes, Vec3::new(fp, 0.04, fp), 6.0),
            tiled_unit(meshes, Vec3::new(fp * 0.62, 0.04, fp * 0.62), 6.0),
        ]
    });
    let mut b_idx = 0_usize;
    let block_all = [-114.0_f32, -69.0, -23.0, 23.0, 69.0, 114.0];
    for &bx in &block_all {
        for &bz in &block_all {
            // Skip Cedar River blocks on West (except Mays Island at z == 0)
            if bx < -100.0 && bz.abs() > 1.0 {
                continue;
            }
            // Block (-23, 23) is Pub 217: sidewalk and kerb, no tower.
            let is_pub = (bx + 23.0).abs() < 1.0 && (bz - 23.0).abs() < 1.0;
            if is_pub {
                commands.spawn((
                    LevelEntity,
                    CityWorldMarker,
                    Mesh3d(block_walk_mesh.clone()),
                    MeshMaterial3d(sidewalk_mat.clone()),
                    Transform::from_xyz(bx, 0.04, bz).with_scale(Vec3::new(26.5, 0.08, 26.5)),
                ));
                commands.spawn((
                    LevelEntity,
                    CityWorldMarker,
                    Mesh3d(kerb_ring.clone()),
                    MeshMaterial3d(kerb_mat.clone()),
                    Transform::from_xyz(bx, 0.0, bz),
                ));
                continue;
            }
            // Five Seasons Park's three blocks: `skatepark::spawn_skate_park` builds the ground, terraces
            // and greenery. Each still takes its turn in the tower sequence so every other tower
            // keeps its height and look.
            if crate::skatepark::is_bigpark_block(bx, bz) {
                b_idx += 1;
                continue;
            }
            // Greene Square (69, 69) and the lot south of it (69, 114) are the skate park:
            // `skatepark::spawn_skate_park` builds their ground, pieces and greenery. The lot still
            // takes its turn in the tower sequence so every other tower keeps its height and look.
            if crate::skatepark::is_park_block(bx, bz) {
                if (bz - crate::skatepark::LOT.y).abs() < 1.0 {
                    b_idx += 1;
                }
                continue;
            }
            let is_outer = bx.abs() > 90.0 || bz.abs() > 90.0;
            commands.spawn((
                LevelEntity,
                CityWorldMarker,
                Mesh3d(block_walk_mesh.clone()),
                MeshMaterial3d(sidewalk_mat.clone()),
                Transform::from_xyz(bx, 0.04, bz).with_scale(Vec3::new(26.5, 0.08, 26.5)),
            ));
            commands.spawn((
                LevelEntity,
                CityWorldMarker,
                Mesh3d(kerb_ring.clone()),
                MeshMaterial3d(kerb_mat.clone()),
                Transform::from_xyz(bx, 0.0, bz),
            ));

            let height = 18.0 + ((b_idx * 17 + 7) % 28) as f32 + if is_outer { 10.0 } else { 0.0 };
            let footprint = if is_outer { 18.0 } else { 17.0 };
            let b_mat = building_mats[b_idx % building_mats.len()].clone();

            commands
                .spawn((
                    LevelEntity,
                    CityWorldMarker,
                    Mesh3d(meshes.add(crate::gfx::tiled_box(
                        Vec3::new(footprint, height, footprint),
                        Vec3::new(footprint, height, footprint),
                        crate::gfx::FACADE_TILE,
                        0.0,
                        Some(crate::gfx::FACADE_FLAT_UV),
                    ))),
                    crate::gfx::CityBuilding,
                    MeshMaterial3d(b_mat.clone()),
                    Transform::from_xyz(bx, height * 0.5, bz),
                    PlatformBox {
                        half_extents: Vec3::new(footprint * 0.5, height * 0.5, footprint * 0.5),
                    },
                ))
                .with_children(|b| {
                    // Architectural setback roof penthouse & spire
                    b.spawn((
                        Mesh3d(meshes.add(crate::gfx::tiled_box(
                            Vec3::ONE,
                            Vec3::new(footprint * 0.62, 6.4, footprint * 0.62),
                            crate::gfx::FACADE_TILE,
                            height,
                            Some(crate::gfx::FACADE_FLAT_UV),
                        ))),
                        MeshMaterial3d(b_mat.clone()),
                        Transform::from_xyz(0.0, height * 0.5 + 3.2, 0.0)
                            .with_scale(Vec3::new(footprint * 0.62, 6.4, footprint * 0.62)),
                    ));
                    // Roof membrane caps on the main roof and the penthouse roof
                    let caps = &roof_caps[usize::from(is_outer)];
                    for (cap, cy, cs) in [
                        (&caps[0], height * 0.5 + 0.02, footprint),
                        (&caps[1], height * 0.5 + 6.42, footprint * 0.62),
                    ] {
                        b.spawn((
                            Mesh3d(cap.clone()),
                            MeshMaterial3d(roof_mats[b_idx % roof_mats.len()].clone()),
                            Transform::from_xyz(0.0, cy, 0.0).with_scale(Vec3::new(cs, 0.04, cs)),
                        ));
                    }
                    if b_idx % 2 == 0 {
                        b.spawn((
                            Mesh3d(unit_cyl.clone()),
                            MeshMaterial3d(white_stripe.clone()),
                            Transform::from_xyz(0.0, height * 0.5 + 10.0, 0.0)
                                .with_scale(Vec3::new(0.28, 7.8, 0.28)),
                        ));
                    }
                    // Illuminated floor window bands on all 4 facades
                    let bands = ((height - 6.0) / 5.5).floor() as i32;
                    for floor_i in 0..bands.min(9) {
                        let ly = -height * 0.5 + 4.5 + floor_i as f32 * 5.2;
                        b.spawn((
                            Mesh3d(unit_box.clone()),
                            MeshMaterial3d(window_glow.clone()),
                            crate::gfx::CityWindowBand,
                            Transform::from_xyz(0.0, ly, 0.0)
                                .with_scale(Vec3::new(footprint + 0.22, 1.35, footprint * 0.84)),
                        ));
                        b.spawn((
                            Mesh3d(unit_box.clone()),
                            MeshMaterial3d(window_glow.clone()),
                            crate::gfx::CityWindowBand,
                            Transform::from_xyz(0.0, ly, 0.0)
                                .with_scale(Vec3::new(footprint * 0.84, 1.35, footprint + 0.22)),
                        ));
                    }
                    // Times Square Neon Billboard facing 5th Avenue on central towers
                    if bx.abs() < 35.0 && !is_outer {
                        b.spawn((
                            Mesh3d(unit_box.clone()),
                            MeshMaterial3d(billboard_mats[b_idx % billboard_mats.len()].clone()),
                            Transform::from_xyz(0.0, -height * 0.5 + 11.0, footprint * 0.5 + 0.35)
                                .with_scale(Vec3::new(10.5, 4.4, 0.4)),
                        ));
                    }
                });
            b_idx += 1;
        }
    }

    // Skate park: Greene Square plaza + the transition lot across 9th St (see skatepark.rs).
    crate::skatepark::spawn_skate_park(commands, meshes, materials);

    // Iconic Tiered EMPIRE CITADEL TOWER at (0, 0, -82)
    let citadel_stone = materials.add(crate::gfx::city_textured(StandardMaterial {
        base_color: Color::srgb(0.74, 0.72, 0.68),
        perceptual_roughness: 0.52,
        metallic: 0.22,
        ..default()
    }, crate::gfx::CityTex::FacadeConcrete));
    let citadel_gold = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.78, 0.24),
        metallic: 0.82,
        perceptual_roughness: 0.24,
        ..default()
    });
    // Base Lobby (top = 16.0)
    commands.spawn((
        LevelEntity,
        CityWorldMarker,
        Mesh3d(meshes.add(crate::gfx::tiled_box(
            Vec3::new(22.0, 16.0, 22.0),
            Vec3::new(22.0, 16.0, 22.0),
            crate::gfx::FACADE_TILE,
            0.0,
            Some(crate::gfx::FACADE_FLAT_UV),
        ))),
        crate::gfx::CityBuilding,
        MeshMaterial3d(citadel_stone.clone()),
        Transform::from_xyz(CITADEL_POS.x, 8.0, CITADEL_POS.z),
        PlatformBox {
            half_extents: Vec3::new(11.0, 8.0, 11.0),
        },
    ));
    // Lexie Main Balcony Tier (y = 16..38 -> top = 38.0!)
    commands.spawn((
        LevelEntity,
        CityWorldMarker,
        Mesh3d(meshes.add(crate::gfx::tiled_box(
            Vec3::new(18.0, 22.0, 18.0),
            Vec3::new(18.0, 22.0, 18.0),
            crate::gfx::FACADE_TILE,
            16.0,
            Some(crate::gfx::FACADE_FLAT_UV),
        ))),
        crate::gfx::CityBuilding,
        MeshMaterial3d(citadel_stone.clone()),
        Transform::from_xyz(CITADEL_POS.x, 27.0, CITADEL_POS.z),
        PlatformBox {
            half_extents: Vec3::new(9.0, 11.0, 9.0),
        },
    ));
    // Upper King-Kong Spire Shaft + Golden Needle behind Lexie Terrace (y = 38..84)
    commands
        .spawn((
            LevelEntity,
            CityWorldMarker,
            Mesh3d(meshes.add(crate::gfx::tiled_box(
                Vec3::new(9.5, 24.0, 9.5),
                Vec3::new(9.5, 24.0, 9.5),
                crate::gfx::FACADE_TILE,
                38.0,
                Some(crate::gfx::FACADE_FLAT_UV),
            ))),
            crate::gfx::CityBuilding,
            MeshMaterial3d(citadel_stone),
            Transform::from_xyz(CITADEL_POS.x, 50.0, CITADEL_POS.z - 4.0),
        ))
        .with_children(|sp| {
            sp.spawn((
                Mesh3d(unit_cyl.clone()),
                MeshMaterial3d(citadel_gold.clone()),
                Transform::from_xyz(0.0, 14.5, 0.0).with_scale(Vec3::new(4.2, 5.5, 4.2)),
            ));
            sp.spawn((
                Mesh3d(unit_cone.clone()),
                MeshMaterial3d(citadel_gold),
                Transform::from_xyz(0.0, 22.0, 0.0).with_scale(Vec3::new(1.5, 11.0, 1.5)),
            ));
        });

    // Riverfront pier with the rideable RIVER STAR Ferris wheel in the river corner (ferris.rs).
    crate::ferris::spawn_ferris_wheel(commands, meshes, materials);
    // Why: spawns an optional local Cedar Rapids easter-egg NPC just outside the Ferris wheel pier
    // to reward boardwalk exploration (`chillary.rs`). Gameplay impact: non-essential; does not gate crew/boss.
    crate::chillary::spawn_city_chillary(commands, meshes, materials);

    // Spawn Colossal Overweight Blonde LEXIE atop Empire Citadel Tower Terrace!
    spawn_lexie_boss(
        commands,
        meshes,
        materials,
        unit_box.clone(),
        unit_sphere.clone(),
        unit_cyl.clone(),
    );

    // Spawn 50 Collectible Golden Tendies across open Manhattan Streets, Avenues & Plazas
    spawn_50_city_tendies(commands, asset_server, meshes, materials);

    // Spawn 28 Moving NYC Cars & Taxis on Avenues & Streets
    spawn_city_traffic(
        commands,
        meshes,
        materials,
    );

    // Spawn 38 Sidewalk Manhattan NPCs
    spawn_city_npcs(commands, meshes, materials);
}

fn spawn_lexie_boss(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    _unit_box: Handle<Mesh>,
    unit_sphere: Handle<Mesh>,
    unit_cyl: Handle<Mesh>,
) {
    let skin = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.78, 0.68),
        perceptual_roughness: 0.45,
        ..default()
    });
    let blonde_hair = materials.add(StandardMaterial {
        base_color: Color::srgb(0.98, 0.84, 0.28),
        perceptual_roughness: 0.38,
        metallic: 0.08,
        ..default()
    });
    let pink_hoodie = materials.add(StandardMaterial {
        base_color: Color::srgb(0.92, 0.22, 0.56),
        perceptual_roughness: 0.62,
        ..default()
    });
    let denim_shorts = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.36, 0.62),
        perceptual_roughness: 0.78,
        ..default()
    });
    let eye_dark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.06, 0.05, 0.07),
        ..default()
    });
    let red_lip = materials.add(StandardMaterial {
        base_color: Color::srgb(0.88, 0.14, 0.24),
        perceptual_roughness: 0.25,
        ..default()
    });
    let jar_glass = materials.add(StandardMaterial {
        base_color: Color::srgba(0.68, 0.92, 0.84, 0.46),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.12,
        metallic: 0.1,
        ..default()
    });
    let garlic_clove = materials.add(StandardMaterial {
        base_color: Color::srgb(0.96, 0.94, 0.74),
        emissive: LinearRgba::rgb(0.32, 0.36, 0.08),
        perceptual_roughness: 0.32,
        ..default()
    });
    let garlic_label = materials.add(StandardMaterial {
        base_color: Color::srgb(0.24, 0.74, 0.28),
        emissive: LinearRgba::rgb(0.18, 0.48, 0.12),
        ..default()
    });
    let sneaker = materials.add(StandardMaterial {
        base_color: Color::srgb(0.96, 0.96, 0.98),
        perceptual_roughness: 0.5,
        ..default()
    });

    // King-Kong Colossus Scale (~29m tall total silhouette!)
    commands
        .spawn((
            LevelEntity,
            CityWorldMarker,
            LexieBoss {
                hp: 300.0,
                max_hp: 300.0,
                on_tower: true,
                throw_cooldown: 4.8,
                chew_timer: 0.0,
                debris_timer: 0.3,
                hit_flash: 0.0,
                roar_cooldown: 4.0,
                last_saber_swing: 0,
            },
            Transform::from_translation(LEXIE_TERRACE_POS).with_scale(Vec3::splat(5.8)),
        ))
        .with_children(|lx| {
            let lm = lexie_meshes(meshes);
            // Chunky sneakers + thick tapered legs & wide hips (overweight King-Kong titan silhouette)
            for sx in [-1.0_f32, 1.0_f32] {
                lx.spawn((
                    Mesh3d(lm.shoe.clone()),
                    MeshMaterial3d(sneaker.clone()),
                    Transform::from_xyz(sx * 0.46, 0.18, 0.10),
                ));
                lx.spawn((
                    Mesh3d(lm.sole.clone()),
                    MeshMaterial3d(pink_hoodie.clone()),
                    Transform::from_xyz(sx * 0.46, 0.18, 0.10),
                ));
                lx.spawn((
                    Mesh3d(lm.leg.clone()),
                    MeshMaterial3d(skin.clone()),
                    Transform::from_xyz(sx * 0.46, 0.92, 0.0),
                ));
                lx.spawn((
                    Mesh3d(unit_sphere.clone()),
                    MeshMaterial3d(denim_shorts.clone()),
                    Transform::from_xyz(sx * 0.44, 1.56, 0.0).with_scale(Vec3::new(0.56, 0.46, 0.54)),
                ));
            }
            // Wide denim waist + one continuous pink hoodie body with a prominent rounded belly
            lx.spawn((
                Mesh3d(unit_sphere.clone()),
                MeshMaterial3d(denim_shorts),
                Transform::from_xyz(0.0, 1.82, 0.04).with_scale(Vec3::new(1.12, 0.52, 0.94)),
            ));
            lx.spawn((
                Mesh3d(lm.torso.clone()),
                MeshMaterial3d(pink_hoodie.clone()),
                Transform::default(),
            ));
            lx.spawn((
                Mesh3d(lm.hood.clone()),
                MeshMaterial3d(pink_hoodie.clone()),
                Transform::from_xyz(0.0, 3.56, -0.30).with_rotation(Quat::from_rotation_x(0.45)),
            ));
            lx.spawn((
                Mesh3d(lm.strings.clone()),
                MeshMaterial3d(sneaker.clone()),
                Transform::default(),
            ));

            // Plump Head, Double Chin, Glaring Eyes, Red Lipstick & Golden-Blonde Cascading Hair
            lx.spawn((
                Mesh3d(lm.head.clone()),
                MeshMaterial3d(skin.clone()),
                Transform::from_xyz(0.0, 3.92, 0.06),
            ));
            lx.spawn((
                Mesh3d(unit_sphere.clone()),
                MeshMaterial3d(skin.clone()),
                Transform::from_xyz(0.0, 3.57, 0.18).with_scale(Vec3::new(0.46, 0.22, 0.42)),
            ));
            lx.spawn((
                Mesh3d(lm.nose.clone()),
                MeshMaterial3d(skin.clone()),
                Transform::from_xyz(0.0, 3.89, 0.65),
            ));
            // Eyes + Angry Brows + Chewing Red Mouth (Local +Z faces down 5th Avenue toward Stan!)
            for sx in [-1.0_f32, 1.0_f32] {
                lx.spawn((
                    Mesh3d(lm.eye.clone()),
                    MeshMaterial3d(eye_dark.clone()),
                    Transform::from_xyz(sx * 0.20, 4.00, 0.60).with_rotation(Quat::from_rotation_y(sx * 0.32) * Quat::from_rotation_z(sx * 0.18)),
                ));
                lx.spawn((
                    Mesh3d(lm.brow.clone()),
                    MeshMaterial3d(blonde_hair.clone()),
                    Transform::from_xyz(sx * 0.21, 4.12, 0.60)
                        .with_rotation(Quat::from_rotation_y(sx * 0.32) * Quat::from_rotation_z(-sx * 0.28)),
                ));
            }
            lx.spawn((
                Mesh3d(lm.lips.clone()),
                MeshMaterial3d(red_lip),
                Transform::from_xyz(0.0, 3.74, 0.62),
            ));

            // Lush Golden Blonde Hair: hugging crown + cascading wavy side locks + back mane + messy bun
            lx.spawn((
                Mesh3d(lm.hair.clone()),
                MeshMaterial3d(blonde_hair.clone()),
                Transform::from_xyz(0.0, 3.92, 0.06),
            ));
            lx.spawn((
                Mesh3d(lm.mane.clone()),
                MeshMaterial3d(blonde_hair.clone()),
                Transform::from_xyz(0.0, 3.62, -0.34),
            ));
            lx.spawn((
                Mesh3d(unit_sphere.clone()),
                MeshMaterial3d(blonde_hair.clone()),
                Transform::from_xyz(0.0, 4.66, -0.18).with_scale(Vec3::new(0.42, 0.36, 0.40)),
            ));
            for sx in [-1.0_f32, 1.0_f32] {
                lx.spawn((
                    Mesh3d(if sx < 0.0 { lm.lock_l.clone() } else { lm.lock_r.clone() }),
                    MeshMaterial3d(blonde_hair.clone()),
                    Transform::from_xyz(sx * 0.56, 3.62, 0.04),
                ));
            }

            // Left Arm holding giant PICKLED GARLIC CLOVES Glass Jar
            lx.spawn((
                Mesh3d(lm.sleeve.clone()),
                MeshMaterial3d(pink_hoodie.clone()),
                Transform::from_xyz(-1.12, 2.74, 0.36)
                    .with_rotation(Quat::from_rotation_x(-0.85) * Quat::from_rotation_z(-0.25)),
            ))
            .with_children(|arm| {
                arm.spawn((
                    Mesh3d(lm.hand_l.clone()),
                    MeshMaterial3d(skin.clone()),
                    Transform::from_xyz(0.02, -0.70, 0.04).with_rotation(Quat::from_rotation_y(0.4)),
                ));
            });
            // Jar Glass + Green Label + Lid + Floating Ivory Pickled Garlic Cloves
            lx.spawn((
                Mesh3d(lm.jar.clone()),
                MeshMaterial3d(jar_glass),
                Transform::from_xyz(-1.18, 2.66, 1.02),
            ));
            lx.spawn((
                Mesh3d(unit_cyl.clone()),
                MeshMaterial3d(garlic_label.clone()),
                Transform::from_xyz(-1.18, 2.66, 1.02).with_scale(Vec3::new(0.535, 0.36, 0.535)),
            ));
            lx.spawn((
                Mesh3d(lm.lid.clone()),
                MeshMaterial3d(garlic_label),
                Transform::from_xyz(-1.18, 3.17, 1.02),
            ));
            for &(gx, gy, gz) in &[
                (-1.26_f32, 2.48_f32, 1.00_f32),
                (-1.08, 2.58, 1.08),
                (-1.20, 2.82, 0.96),
                (-1.14, 2.94, 1.04),
            ] {
                lx.spawn((
                    Mesh3d(lm.clove.clone()),
                    MeshMaterial3d(garlic_clove.clone()),
                    Transform::from_xyz(gx, gy, gz).with_rotation(Quat::from_rotation_z(gx * 3.0)),
                ));
            }

            // Right Throwing Arm gripping a glowing Pickled Garlic Clove
            lx.spawn((
                LexieThrowArm,
                Mesh3d(lm.sleeve_long.clone()),
                MeshMaterial3d(pink_hoodie),
                Transform::from_xyz(1.16, 3.16, 0.28)
                    .with_rotation(Quat::from_rotation_x(0.65) * Quat::from_rotation_z(0.28)),
            ))
            .with_children(|arm| {
                arm.spawn((
                    Mesh3d(lm.fist.clone()),
                    MeshMaterial3d(skin),
                    Transform::from_xyz(0.0, 0.74, 0.0).with_rotation(Quat::from_rotation_x(std::f32::consts::PI)),
                ));
                arm.spawn((
                    Mesh3d(lm.big_clove.clone()),
                    MeshMaterial3d(garlic_clove),
                    Transform::from_xyz(0.0, 1.06, 0.0),
                ));
            });
        });
}

/// Lexie's body-part meshes, shared by the city boss and the finale cutscene so she keeps
/// one consistent model. Local units match `spawn_lexie_boss` before its 5.8x root scale.
pub(crate) struct LexieMeshes {
    pub shoe: Handle<Mesh>,
    pub sole: Handle<Mesh>,
    pub leg: Handle<Mesh>,
    pub torso: Handle<Mesh>,
    pub hood: Handle<Mesh>,
    pub strings: Handle<Mesh>,
    pub head: Handle<Mesh>,
    pub nose: Handle<Mesh>,
    pub eye: Handle<Mesh>,
    pub brow: Handle<Mesh>,
    pub lips: Handle<Mesh>,
    pub hair: Handle<Mesh>,
    pub mane: Handle<Mesh>,
    pub lock_l: Handle<Mesh>,
    pub lock_r: Handle<Mesh>,
    pub sleeve: Handle<Mesh>,
    pub sleeve_long: Handle<Mesh>,
    pub hand_l: Handle<Mesh>,
    pub fist: Handle<Mesh>,
    pub jar: Handle<Mesh>,
    pub lid: Handle<Mesh>,
    pub clove: Handle<Mesh>,
    pub big_clove: Handle<Mesh>,
}

pub(crate) fn lexie_meshes(meshes: &mut Assets<Mesh>) -> LexieMeshes {
    use crate::shapes::{self as sh, ring};
    let torso = sh::loft(
        &[
            ring(1.70, 0.0, 0.0),
            ring(1.74, 0.92, 0.78).dz(0.08),
            ring(2.05, 1.14, 0.98).dz(0.20),
            ring(2.40, 1.18, 1.04).dz(0.26),
            ring(2.75, 1.09, 0.92).dz(0.16),
            ring(3.05, 1.06, 0.80).dz(0.03),
            ring(3.33, 0.98, 0.68),
            ring(3.54, 0.62, 0.48),
            ring(3.66, 0.28, 0.26).dz(0.04),
            ring(3.70, 0.0, 0.0).dz(0.04),
        ],
        3,
        36,
        1.0,
    );
    let string = |x: f32| sh::limb(0.62, 0.025, 0.032).at(x, 3.10, 0.0).deformed(|p| {
        // Lay the drawstrings onto the curved hoodie front
        Vec3::new(p.x, p.y, p.z + 0.84 - 0.10 * ((p.y - 3.10) / 0.31).powi(2))
    });
    let lock = |side: f32| {
        sh::limb_profile(1.0, &[0.18, 0.30, 0.27, 0.22, 0.12], 0.85).deformed(move |p| {
            Vec3::new(p.x + side * 0.05 * (p.y * 7.0).sin(), p.y, p.z + 0.04 * (p.y * 6.0).cos())
        })
    };
    let lips = sh::superellipsoid(Vec3::new(0.15, 0.042, 0.06), 0.8, 0.9, 20, 10)
        .at(0.0, 0.03, 0.0)
        .merged(sh::superellipsoid(Vec3::new(0.14, 0.05, 0.065), 0.8, 0.9, 20, 10).at(0.0, -0.035, 0.0))
        .deformed(|p| Vec3::new(p.x, p.y, p.z - 0.6 * p.x * p.x));
    let jar = sh::loft(
        &[
            ring(-0.475, 0.0, 0.0),
            ring(-0.475, 0.44, 0.44),
            ring(-0.45, 0.52, 0.52),
            ring(0.36, 0.52, 0.52),
            ring(0.44, 0.44, 0.44),
            ring(0.47, 0.40, 0.40),
            ring(0.47, 0.0, 0.0),
        ],
        2,
        32,
        1.0,
    );
    LexieMeshes {
        shoe: sh::add(meshes, sh::shoe(0.46, 0.36, 0.68)),
        sole: sh::add(meshes, sh::shoe_sole(0.46, 0.36, 0.68)),
        leg: sh::add(meshes, sh::limb_profile(1.42, &[0.30, 0.34, 0.28, 0.21], 1.05)),
        torso: sh::add(meshes, torso),
        hood: meshes.add(Torus::new(0.24, 0.58)),
        strings: sh::add(meshes, string(-0.16).merged(string(0.16))),
        head: sh::add(meshes, sh::head(0.63, 0.12)),
        nose: sh::add(meshes, sh::superellipsoid(Vec3::new(0.07, 0.10, 0.07), 1.0, 1.0, 14, 10)),
        eye: sh::add(meshes, sh::superellipsoid(Vec3::new(0.10, 0.06, 0.04), 1.0, 1.0, 16, 10)),
        brow: sh::add(meshes, sh::arc_bar(0.27, &[0.02, 0.036, 0.03, 0.016], 0.02, 0.03, 0.7)),
        lips: sh::add(meshes, lips),
        hair: sh::add(meshes, sh::hair_cap(0.63, 0.12, 0.42, -0.55, 0.10, 0.16)),
        mane: sh::add(meshes, sh::superellipsoid(Vec3::new(0.90, 0.98, 0.50), 1.0, 1.0, 28, 18).deformed(|p| {
            Vec3::new(p.x * (1.0 - 0.18 * (p.y / 0.98).min(0.0).abs()), p.y, p.z + 0.06 * (p.x * 8.0).sin() * (p.y < 0.0) as u8 as f32)
        })),
        lock_l: sh::add(meshes, lock(-1.0)),
        lock_r: sh::add(meshes, lock(1.0)),
        sleeve: sh::add(meshes, sh::limb(1.30, 0.36, 0.30)),
        sleeve_long: sh::add(meshes, sh::limb(1.38, 0.30, 0.36)),
        hand_l: sh::add(meshes, sh::hand(0.24, -1.0, false)),
        fist: sh::add(meshes, sh::hand(0.24, 1.0, true)),
        jar: sh::add(meshes, jar),
        lid: sh::add(meshes, sh::superellipsoid(Vec3::new(0.43, 0.06, 0.43), 0.4, 1.0, 28, 8)),
        clove: sh::add(meshes, sh::limb_profile(0.42, &[0.02, 0.12, 0.13, 0.10], 0.9)),
        big_clove: sh::add(meshes, sh::limb_profile(0.62, &[0.03, 0.17, 0.19, 0.14], 0.9)),
    }
}

fn spawn_50_city_tendies(
    commands: &mut Commands,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    // The real golden tendy model (as in Level 1 and the Mom fight), one shared scene handle.
    let tendy_path = "models/golden_tendy.glb#Scene0";
    let tendy_scene: Handle<Scene> = asset_server.load(tendy_path);
    // The model is ~1.1 m long with its body ~0.26-0.79 m above its origin; 1.7x makes it about
    // as big as the old 1.85 m capsule, and the offset centres it on the pickup point.
    const TENDY_MODEL_SCALE: f32 = 1.7;
    let seal_mesh = meshes.add(Sphere::new(1.25));
    let seal_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.60, 0.62, 0.68, 0.42),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.25,
        ..default()
    });
    let beacon_mesh = meshes.add(Cylinder::new(0.28, 12.0));
    let beacon_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.85, 0.25, 0.32),
        emissive: LinearRgba::rgb(1.4, 0.95, 0.18),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });

    // Generate exactly 50 well-distributed street & avenue coordinates guaranteed clear of buildings
    let mut spots = Vec::with_capacity(50);
    // 25 along the 5 N-S Avenues (-92, -46, 0, 46, 92)
    for (ai, &ax) in ROAD_COORDS.iter().enumerate() {
        for zi in 0..5 {
            let z = -104.0 + zi as f32 * 52.0 + if ai == 2 { 10.0 } else { 0.0 };
            let candidate = Vec3::new(ax + if zi % 2 == 0 { -6.5 } else { 6.5 }, 1.0, z);
            if is_clear_of_skyscrapers(candidate.x, candidate.z, 1.5) {
                spots.push(candidate);
            }
        }
    }
    // 25 along East-West cross streets & sidewalk corners
    for &sz in &ROAD_COORDS {
        for xi in 0..6 {
            if spots.len() >= 50 {
                break;
            }
            let x = -112.0 + xi as f32 * 44.5;
            let candidate = Vec3::new(x, 1.0, sz + if xi % 2 == 0 { 6.0 } else { -6.0 });
            let too_close = spots
                .iter()
                .any(|p| p.distance_squared(candidate) < 85.0);
            if !too_close && is_clear_of_skyscrapers(candidate.x, candidate.z, 1.5) {
                spots.push(candidate);
            }
        }
    }
    // Deterministically top up to 75 so players have extra Tendies for Tendy Credits (scooters) + 50/50 Kaiju goal
    let mut step = 0_usize;
    while spots.len() < 75 {
        let ax = ROAD_COORDS[step % ROAD_COORDS.len()];
        let sz = -108.0 + ((step * 29) % 216) as f32;
        let cand = Vec3::new(ax, 1.0, sz);
        let too_close = spots.iter().any(|p| p.distance_squared(cand) < 42.0);
        if !too_close && is_clear_of_skyscrapers(cand.x, cand.z, 1.5) {
            spots.push(cand);
        }
        step += 1;
    }
    crate::scooter::spawn_city_scooters(commands, meshes, materials);
    crate::stunts::spawn_stunt_course(commands, meshes, materials);
    // Never put a tendy inside Pub 217.
    spots.retain(|p| !is_in_pub217(*p, 1.8));
    let mut stunt_spots = crate::stunts::stunt_tendy_spots();
    stunt_spots.retain(|p| !is_in_pub217(*p, 1.8));
    // About 70 on the map in total (50 grow Stan to full kaiju): the stunt trails stay whole and
    // the street ones are thinned evenly.
    let street_keep = CITY_TENDY_TOTAL.saturating_sub(stunt_spots.len()).max(30);
    thin_street_tendies(&mut spots, &stunt_spots, street_keep);
    spots.extend(stunt_spots);
    commands.insert_resource(CityTendyLook { lit: None });
    let mut mike_seed_bytes = [0u8; 8];
    let _ = getrandom::getrandom(&mut mike_seed_bytes);
    let mike_seed = u64::from_le_bytes(mike_seed_bytes);
    crate::scooter::spawn_city_mike(commands, meshes, materials, mike_seed);
    // TJ, Derek, Nate and Glenn wait at fresh random spots every time the city is built (a
    // death retry or reload re-scatters whoever is still unrecruited).
    let crew_seed = if crate::audio::debug_city_start() == "crew" {
        DEBUG_CREW_SEED
    } else {
        mike_seed.rotate_left(17) ^ 0x9E37_79B9_7F4A_7C15
    };
    let crew_spots = pick_crew_spots(crew_seed);
    commands.insert_resource(CrewSpots {
        spots: crew_spots,
        revealed: [false; 4],
    });
    spawn_crew_tells(commands, meshes, materials, &crew_spots);
    crate::derek::spawn_city_derek(commands, meshes, materials, crew_spots[CREW_DEREK].0);
    crate::nate::spawn_city_nate(commands, meshes, materials, crew_spots[CREW_NATE].0);
    crate::glenn::spawn_city_glenn(commands, meshes, materials, crew_spots[CREW_GLENN].0);
    let crew_pos = crew_spots.map(|(p, _)| p);
    spawn_parked_car(commands, meshes, materials, &spots, &crew_pos);

    for (i, pos) in spots.into_iter().enumerate() {
        commands
            .spawn((
                LevelEntity,
                CityWorldMarker,
                CityTendy,
                TendyPickup {
                    base_y: pos.y,
                    phase: i as f32 * 0.55,
                    is_final_goal: false,
                },
                Transform::from_translation(pos),
                Visibility::default(),
            ))
            .with_children(|t| {
                t.spawn((
                    SceneRoot(tendy_scene.clone()),
                    crate::state::SceneRetryPath::new(tendy_path),
                    Transform::from_xyz(0.0, -0.53 * TENDY_MODEL_SCALE, 0.0)
                        .with_scale(Vec3::splat(TENDY_MODEL_SCALE)),
                ));
                // Locked look (toggled by `update_city_crew_gate`): sealed bubble on, beacon off.
                t.spawn((
                    CityTendySeal,
                    Mesh3d(seal_mesh.clone()),
                    MeshMaterial3d(seal_mat.clone()),
                    Transform::default(),
                    Visibility::Inherited,
                ));
                // Tall faint golden sky-beacon so players spot distant Manhattan tendies!
                t.spawn((
                    CityTendyBeacon,
                    Mesh3d(beacon_mesh.clone()),
                    MeshMaterial3d(beacon_mat.clone()),
                    Transform::from_xyz(0.0, 5.5, 0.0),
                    Visibility::Hidden,
                ));
            });
    }
}

fn spawn_city_traffic(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let body_colors = [
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.96, 0.78, 0.08), // NYC Yellow Cab
            perceptual_roughness: 0.25,
            metallic: 0.3,
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.95, 0.76, 0.08), // Another NYC Yellow Cab
            perceptual_roughness: 0.25,
            metallic: 0.3,
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.84, 0.16, 0.18), // Red Sedan
            perceptual_roughness: 0.28,
            metallic: 0.4,
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.16, 0.42, 0.86), // NYPD / Blue Cruiser
            perceptual_roughness: 0.28,
            metallic: 0.4,
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.90, 0.90, 0.92), // Delivery Van
            perceptual_roughness: 0.35,
            ..default()
        }),
    ];
    let glass = materials.add(StandardMaterial {
        base_color: Color::srgb(0.14, 0.22, 0.32),
        metallic: 0.6,
        perceptual_roughness: 0.15,
        ..default()
    });
    let tire = materials.add(StandardMaterial {
        base_color: Color::srgb(0.09, 0.09, 0.11),
        perceptual_roughness: 0.85,
        ..default()
    });
    let headlight = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.98, 0.80),
        emissive: LinearRgba::rgb(2.2, 2.0, 1.3),
        ..default()
    });
    let taillight = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.15, 0.15),
        emissive: LinearRgba::rgb(2.2, 0.2, 0.2),
        ..default()
    });
    let trim = materials.add(StandardMaterial {
        base_color: Color::srgb(0.12, 0.12, 0.14),
        perceptual_roughness: 0.55,
        metallic: 0.2,
        ..default()
    });
    let hub = materials.add(StandardMaterial {
        base_color: Color::srgb(0.72, 0.74, 0.78),
        perceptual_roughness: 0.3,
        metallic: 0.8,
        ..default()
    });
    let taxi_sign = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.96, 0.82),
        emissive: LinearRgba::rgb(0.9, 0.8, 0.45),
        ..default()
    });
    let cm = car_meshes(meshes);

    // Spawn 28 moving NYC vehicles distributed across both N-S Avenues and E-W Cross-Streets
    for i in 0..28_usize {
        let axis_x = i % 2 == 1;
        let dir = if (i / 2) % 2 == 0 { 1.0 } else { -1.0 };
        let road = ROAD_COORDS[(i / 3) % ROAD_COORDS.len()];
        let lane_offset = if dir > 0.0 { 3.6 } else { -3.6 };
        let lane_coord = road + lane_offset;
        let mut progress = -120.0 + ((i as f32 * 37.0) % 240.0);
        // Avenues whose north stub is Five Seasons Park: start south of the U-turn point.
        if !axis_x && crate::skatepark::bigpark_uturn_z(road).is_some_and(|z0| progress < z0) {
            progress = -progress;
        }
        let pos = if axis_x {
            Vec3::new(progress, 0.0, lane_coord)
        } else {
            Vec3::new(lane_coord, 0.0, progress)
        };
        let yaw = if axis_x {
            if dir > 0.0 {
                std::f32::consts::FRAC_PI_2
            } else {
                -std::f32::consts::FRAC_PI_2
            }
        } else if dir > 0.0 {
            0.0
        } else {
            std::f32::consts::PI
        };

        let speed = 13.5 + (i % 5) as f32 * 2.4;
        let mat = body_colors[i % body_colors.len()].clone();

        commands
            .spawn((
                LevelEntity,
                CityWorldMarker,
                CityCar {
                    axis_x,
                    dir,
                    speed,
                    horn_cooldown: 0.0,
                    occupied: false,
                    parked: false,
                    yaw,
                    velocity: Vec2::ZERO,
                    pitch_lean: 0.0,
                    roll_lean: 0.0,
                    steer_input: 0.0,
                    knock_vel: Vec2::ZERO,
                    drift_sfx_cooldown: 0.0,
                    car_hp: 100.0,
                    roof_h: if i % body_colors.len() < 2 { CAR_CAB_SIGN_TOP } else { CAR_ROOF_TOP },
                    ride: CarRide::default(),
                },
                Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(yaw)),
            ))
            .with_children(|car| {
                // Rounded chassis with sloped hood/trunk, tapered glass cabin and a body-colour roof
                car.spawn((Mesh3d(cm.body.clone()), MeshMaterial3d(mat.clone()), Transform::from_xyz(0.0, 0.72, 0.0)));
                car.spawn((Mesh3d(cm.cabin.clone()), MeshMaterial3d(glass.clone()), Transform::from_xyz(0.0, 1.38, -0.15)));
                car.spawn((Mesh3d(cm.roof.clone()), MeshMaterial3d(mat), Transform::from_xyz(0.0, 1.665, -0.26)));
                // Bumpers, grille, side skirts
                car.spawn((Mesh3d(cm.trim.clone()), MeshMaterial3d(trim.clone()), Transform::default()));
                // Headlights (+Z front) & Taillights (-Z rear)
                car.spawn((Mesh3d(cm.headlights.clone()), MeshMaterial3d(headlight.clone()), Transform::default()));
                car.spawn((Mesh3d(cm.taillights.clone()), MeshMaterial3d(taillight.clone()), Transform::default()));
                if i % body_colors.len() < 2 {
                    // Yellow cabs get the classic roof sign
                    car.spawn((Mesh3d(cm.taxi_sign.clone()), MeshMaterial3d(taxi_sign.clone()), Transform::from_xyz(0.0, 1.80, -0.30)));
                }
                // 4 Wheels
                for (wx, wz, is_front) in [
                    (-1.05_f32, 1.32_f32, true),
                    (1.05, 1.32, true),
                    (-1.05, -1.32, false),
                    (1.05, -1.32, false),
                ] {
                    car.spawn((
                        CityCarWheel { is_front },
                        Mesh3d(cm.tyre.clone()),
                        MeshMaterial3d(tire.clone()),
                        Transform::from_xyz(wx, 0.36, wz)
                            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2))
                            .with_scale(Vec3::new(0.36, 0.28, 0.36)),
                    ))
                    .with_children(|w| {
                        w.spawn((Mesh3d(cm.hub.clone()), MeshMaterial3d(hub.clone()), Transform::default()));
                    });
                }
            });
    }
}

/// Shared car meshes. Footprint matches the old 2.15 x 4.45 box so driving/collision code
/// (which works from constants, not meshes) is unaffected.
struct CarMeshes {
    body: Handle<Mesh>,
    cabin: Handle<Mesh>,
    roof: Handle<Mesh>,
    trim: Handle<Mesh>,
    headlights: Handle<Mesh>,
    taillights: Handle<Mesh>,
    taxi_sign: Handle<Mesh>,
    tyre: Handle<Mesh>,
    hub: Handle<Mesh>,
}

fn car_meshes(meshes: &mut Assets<Mesh>) -> CarMeshes {
    use crate::shapes as sh;
    let body = sh::superellipsoid(Vec3::new(1.075, 0.41, 2.225), 0.28, 0.2, 36, 18).deformed(|p| {
        let end = sh::smoothstep(0.55, 1.0, p.z.abs() / 2.225);
        let mut q = p;
        if q.y > 0.0 {
            q.y -= 0.12 * end * (q.y / 0.41);
        }
        q
    });
    let cabin = sh::superellipsoid(Vec3::new(0.94, 0.31, 1.125), 0.3, 0.3, 32, 16).deformed(|p| {
        let t = (p.y / 0.31 * 0.5 + 0.5).clamp(0.0, 1.0);
        let zk = if p.z > 0.0 { 1.0 - 0.38 * t } else { 1.0 - 0.18 * t };
        Vec3::new(p.x * (1.0 - 0.12 * t), p.y, p.z * zk)
    });
    let roof = sh::superellipsoid(Vec3::new(0.80, 0.055, 0.76), 0.5, 0.3, 28, 10);
    let bumper = |z: f32| sh::superellipsoid(Vec3::new(1.07, 0.11, 0.15), 0.45, 0.35, 28, 10).at(0.0, 0.42, z);
    let skirt = |x: f32| sh::superellipsoid(Vec3::new(0.05, 0.07, 1.6), 0.5, 0.5, 12, 8).at(x, 0.36, 0.0);
    let trim = bumper(2.18)
        .merged(bumper(-2.18))
        .merged(sh::plate(0.86, 0.17, 0.06, 0.4).at(0.0, 0.70, 2.21))
        .merged(skirt(1.06))
        .merged(skirt(-1.06));
    let lamp = |x: f32, z: f32| sh::superellipsoid(Vec3::new(0.21, 0.10, 0.05), 0.6, 0.6, 16, 10).at(x, 0.80, z);
    CarMeshes {
        body: sh::add(meshes, body),
        cabin: sh::add(meshes, cabin),
        roof: sh::add(meshes, roof),
        trim: sh::add(meshes, trim),
        headlights: sh::add(meshes, lamp(-0.70, 2.19).merged(lamp(0.70, 2.19))),
        taillights: sh::add(meshes, lamp(-0.72, -2.19).merged(lamp(0.72, -2.19))),
        taxi_sign: sh::add(meshes, sh::rounded_box(Vec3::new(0.52, 0.17, 0.22), 0.35)),
        tyre: sh::add(meshes, sh::tyre()),
        hub: meshes.add(Cylinder::new(0.58, 1.04)),
    }
}

/// Street trees along 3rd St SE (x = +-`STREET_TREE_X`, at these z).
pub(crate) const STREET_TREE_X: f32 = 8.2;
pub(crate) const STREET_TREE_Z: [f32; 6] = [-64.0, -32.0, -10.0, 12.0, 32.0, 56.0];

/// Home spots of the 38 sidewalk pedestrians (same formula as `spawn_city_npcs`, without the
/// Pub 217 skip). Each wanders +-2.2 m in x and +-4.7 m in z around its home.
pub(crate) fn npc_homes() -> Vec<Vec2> {
    (0..38_usize)
        .map(|i| {
            let bx = BLOCK_COORDS[i % BLOCK_COORDS.len()];
            let bz = BLOCK_COORDS[(i / 4) % BLOCK_COORDS.len()];
            let offset = 10.6;
            match i % 4 {
                0 => Vec2::new(bx + offset, bz),
                1 => Vec2::new(bx - offset, bz),
                2 => Vec2::new(bx, bz + offset),
                _ => Vec2::new(bx, bz - offset),
            }
        })
        .collect()
}

fn spawn_city_npcs(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let coats = [
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.18, 0.42, 0.78),
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.84, 0.24, 0.26),
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.24, 0.64, 0.36),
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.88, 0.62, 0.16),
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.62, 0.30, 0.78),
            ..default()
        }),
    ];
    let pants = materials.add(StandardMaterial {
        base_color: Color::srgb(0.18, 0.20, 0.24),
        ..default()
    });
    let skin = materials.add(StandardMaterial {
        base_color: Color::srgb(0.90, 0.74, 0.62),
        ..default()
    });
    let hair = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.12, 0.10),
        ..default()
    });
    let shoe = materials.add(StandardMaterial {
        base_color: Color::srgb(0.10, 0.10, 0.11),
        perceptual_roughness: 0.6,
        ..default()
    });
    let npc = npc_meshes(meshes);

    // Spawn 38 New Yorkers patrolling sidewalks safely outside skyscraper footprints
    for i in 0..38_usize {
        let bx = BLOCK_COORDS[i % BLOCK_COORDS.len()];
        let bz = BLOCK_COORDS[(i / 4) % BLOCK_COORDS.len()];
        let side = (i % 4) as f32;
        let offset = 10.6;
        let home = match i % 4 {
            0 => Vec3::new(bx + offset, 0.08, bz),
            1 => Vec3::new(bx - offset, 0.08, bz),
            2 => Vec3::new(bx, 0.08, bz + offset),
            _ => Vec3::new(bx, 0.08, bz - offset),
        };
        // The (-23, 23) block is Pub 217's: every patrol there would walk through its walls.
        let (pmin_x, pmax_x, pmin_z, pmax_z) = pub217_rect();
        if home.x + 2.2 + 0.5 > pmin_x
            && home.x - 2.2 - 0.5 < pmax_x
            && home.z + 4.8 + 0.5 > pmin_z
            && home.z - 4.8 - 0.5 < pmax_z
        {
            continue;
        }
        let coat = coats[i % coats.len()].clone();

        commands
            .spawn((
                LevelEntity,
                CityWorldMarker,
                CityNpc {
                    home_x: home.x,
                    home_z: home.z,
                    patrol_radius: 10.5,
                    angle: side * 1.57,
                    speed: 2.2 + (i % 4) as f32 * 0.45,
                    walk_phase: i as f32 * 0.85,
                },
                Transform::from_translation(home),
            ))
            .with_children(|n| {
                n.spawn((Mesh3d(npc.torso.clone()), MeshMaterial3d(coat.clone()), Transform::from_xyz(0.0, 1.05, 0.0)));
                n.spawn((Mesh3d(npc.neck.clone()), MeshMaterial3d(skin.clone()), Transform::from_xyz(0.0, 1.43, 0.0)));
                // Head + hair + eyes
                n.spawn((Mesh3d(npc.head.clone()), MeshMaterial3d(skin.clone()), Transform::from_xyz(0.0, 1.62, 0.0)));
                n.spawn((Mesh3d(npc.hair.clone()), MeshMaterial3d(hair.clone()), Transform::from_xyz(0.0, 1.62, 0.0)));
                n.spawn((Mesh3d(npc.eyes.clone()), MeshMaterial3d(hair.clone()), Transform::from_xyz(0.0, 1.62, 0.0)));
                // Animated legs & arms (meshes centred on the limb origin, which is the swing pivot)
                for &sx in &[-1.0_f32, 1.0_f32] {
                    n.spawn((
                        CityNpcLimb { side: sx, is_arm: false },
                        Mesh3d(npc.leg.clone()),
                        MeshMaterial3d(pants.clone()),
                        Transform::from_xyz(sx * 0.14, 0.36, 0.0),
                    ))
                    .with_children(|l| {
                        l.spawn((Mesh3d(npc.shoe.clone()), MeshMaterial3d(shoe.clone()), Transform::from_xyz(0.0, -0.32, 0.04)));
                    });
                    n.spawn((
                        CityNpcLimb { side: sx, is_arm: true },
                        Mesh3d(npc.arm.clone()),
                        MeshMaterial3d(skin.clone()),
                        Transform::from_xyz(sx * 0.34, 1.04, 0.0),
                    ))
                    .with_children(|a| {
                        let hand = if sx < 0.0 { npc.hand_l.clone() } else { npc.hand_r.clone() };
                        a.spawn((Mesh3d(hand), MeshMaterial3d(skin.clone()), Transform::from_xyz(0.0, -0.31, 0.0)));
                    });
                }
            });
    }
}

/// Shared street-NPC meshes (built once for all 38 pedestrians).
struct NpcMeshes {
    torso: Handle<Mesh>,
    neck: Handle<Mesh>,
    head: Handle<Mesh>,
    hair: Handle<Mesh>,
    eyes: Handle<Mesh>,
    leg: Handle<Mesh>,
    shoe: Handle<Mesh>,
    arm: Handle<Mesh>,
    hand_l: Handle<Mesh>,
    hand_r: Handle<Mesh>,
}

fn npc_meshes(meshes: &mut Assets<Mesh>) -> NpcMeshes {
    use crate::shapes as sh;
    let eye = |x: f32| sh::superellipsoid(Vec3::new(0.022, 0.028, 0.012), 1.0, 1.0, 10, 8).at(x, 0.02, 0.205);
    NpcMeshes {
        torso: sh::add(meshes, sh::torso(&sh::Torso {
            height: 0.72,
            hip: Vec2::new(0.23, 0.15),
            waist: Vec2::new(0.22, 0.14),
            chest: Vec2::new(0.26, 0.155),
            shoulder: Vec2::new(0.27, 0.14),
            neck: 0.06,
            sq: 0.8,
            chest_fwd: 0.01,
        })),
        neck: sh::add(meshes, sh::limb(0.16, 0.06, 0.065)),
        head: sh::add(meshes, sh::head(0.22, 0.12)),
        hair: sh::add(meshes, sh::hair_cap(0.22, 0.12, 0.52, -0.35, 0.035, 0.07)),
        eyes: sh::add(meshes, eye(-0.075).merged(eye(0.075))),
        leg: sh::add(meshes, sh::limb(0.72, 0.085, 0.068)),
        shoe: sh::add(meshes, sh::shoe(0.13, 0.08, 0.24)),
        arm: sh::add(meshes, sh::limb(0.56, 0.068, 0.054)),
        hand_l: sh::add(meshes, sh::hand(0.058, -1.0, false)),
        hand_r: sh::add(meshes, sh::hand(0.058, 1.0, false)),
    }
}

// =========================================================================
// PART 3: CINEMATIC AERIAL NEW YORK CITY INTRO CUTSCENE
// =========================================================================

pub fn enter_city_intro_cutscene(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut stats: ResMut<PlayerStats>,
    mut player_q: Query<(&mut Transform, &mut Player)>,
    mut hud_q: Query<&mut Node, With<HudOverlay>>,
    existing_city_q: Query<Entity, With<CityWorldMarker>>,
    old_level_q: Query<Entity, (Or<(With<LevelEntity>, With<Enemy>, With<FiberPuddle>)>, Without<Player>)>,
) {
    for mut node in &mut hud_q {
        node.display = Display::None;
    }
    stats.needs_city_char_select = true;
    crate::audio::mark_mom_defeated();
    crate::audio::mark_mom_defeated();
    crate::ui::save_game_checkpoint("city", &stats);

    stats.in_city = true;
    stats.boss_mode = false;
    stats.tendies = 0;
    stats.total_tendies = 50;
    stats.wave = 4;
    stats.max_hearts = 20;
    stats.hearts = stats.max_hearts;
    stats.player_scale = 1.8;
    stats.tj_recruited = false;
    stats.lexie_defeated = false;
    stats.hint_text.clear();

    let start_t = take_cutscene_start_time();
    stats.cutscene_timer = if start_t > 0.0 { start_t } else { 0.0 };
    stats.cutscene_beat = 0;

    set_touch_playing(false);
    set_touch_boss_mode(false);

    spawn_nyc_world_if_needed(
        &mut commands,
        &asset_server,
        &mut meshes,
        &mut materials,
        &existing_city_q,
        &old_level_q,
    );
    commands.insert_resource(ClearColor(Color::srgb(0.03, 0.04, 0.09)));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.75, 0.80, 0.95),
        brightness: 440.0,
    });

    if let Ok((mut p_tf, mut player)) = player_q.get_single_mut() {
        p_tf.translation = Vec3::new(0.0, 0.0, 82.0);
        p_tf.scale = Vec3::splat(1.8);
        player.velocity = Vec3::ZERO;
        player.facing_yaw = 0.0; // Facing North (-Z) straight down 5th Avenue toward Empire Citadel!
        p_tf.rotation = Quat::from_rotation_y(0.0);
    }

    play_viking_chant_music();
    play_sfx("thunder");

    commands
        .spawn((
            CityIntroOverlay,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(14.0),
                justify_content: JustifyContent::Center,
                padding: UiRect::horizontal(Val::Px(14.0)),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    padding: UiRect::axes(Val::Px(20.0), Val::Px(14.0)),
                    width: Val::Percent(94.0),
                    max_width: Val::Px(800.0),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.04, 0.07, 0.14, 0.90)),
                BorderColor(Color::srgb(1.0, 0.80, 0.18)),
                BorderRadius::all(Val::Px(14.0)),
            ))
            .with_children(|card| {
                card.spawn(Node {
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|hdr| {
                    hdr.spawn((
                        CityIntroSpeakerText,
                        Text::new("NEW YORK CITY - AERIAL OVERLOOK"),
                        TextFont {
                            font_size: 15.0,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.84, 0.25)),
                    ));
                    hdr.spawn((
                        CityIntroBeatBadge,
                        Text::new("MANHATTAN | SCENE 1/4"),
                        TextFont {
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.45, 0.88, 1.0)),
                    ));
                });
                card.spawn((
                    CityIntroSubtitleText,
                    Text::new(
                        "Welcome to New York City! Towering skyscrapers, rushing taxis, crowds, and 50 golden tendies stretch across Manhattan!",
                    ),
                    TextFont {
                        font_size: 20.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });
        });

    if stats.cutscene_timer < 1.0 {
        speak_voice(
            "Welcome to New York City! Towering skyscrapers, speeding cars, and fifty golden tendies stretch across Manhattan!",
            "narrator",
        );
    }
}

pub fn update_city_intro_cutscene(
    time: Res<Time>,
    mut stats: ResMut<PlayerStats>,
    mut next_state: crate::transition::SceneTransition,
    mut cam_q: Query<&mut Transform, With<Camera3d>>,
) {
    let dt = time.delta_secs().min(0.05);
    stats.cutscene_timer += dt;
    // Hold-to-skip: jump to the end so the normal end path below runs.
    let skipping = crate::skip::skipping();
    if skipping {
        stats.cutscene_timer = stats.cutscene_timer.max(23.0);
    }
    let t = stats.cutscene_timer;

    let (beat_idx, speech, role, sfx): (u8, &str, &str, Option<&str>) = if t < 5.6 {
        (0, "", "narrator", None)
    } else if t < 11.2 {
        (
            1,
            "High atop the Empire Citadel Tower, a King-Kong-sized menace is tearing Manhattan apart!",
            "narrator",
            Some("thunder"),
        )
    } else if t < 17.5 {
        (
            2,
            "Ha ha ha! Nobody stops Lexie and her extra-stinky pickled garlic cloves! Eat garlic fumes, tiny city!",
            "lexie",
            Some("garlic_throw"),
        )
    } else {
        (
            3,
            "Collect fifty golden tendies using your radar map, grow as tall as a skyscraper, and defeat Lexie!",
            "narrator",
            Some("tendy"),
        )
    };

    let mut active_beat = stats.cutscene_beat;
    if beat_idx > stats.cutscene_beat && (skipping || !is_voice_speaking()) {
        stats.cutscene_beat = beat_idx;
        active_beat = beat_idx;
        // A skip jumps straight to the last beat; its line and sting are not started.
        if !skipping {
            if let Some(name) = sfx {
                play_sfx(name);
            }
            if !speech.is_empty() {
                speak_voice(speech, role);
            }
        }
    } else if beat_idx > stats.cutscene_beat && is_voice_speaking() {
        stats.cutscene_timer -= dt;
    }

    // Sweeping Helicopter Aerial Camera panning massive Manhattan & Empire Citadel Tower
    if let Ok(mut cam_tf) = cam_q.get_single_mut() {
        let (eye, target) = match active_beat {
            0 => {
                // High aerial sweep orbiting the huge 270m skyline!
                let u = (t / 5.6).clamp(0.0, 1.0);
                (
                    Vec3::new(42.0 + u * 26.0, 74.0 - u * 14.0, 96.0 - u * 18.0),
                    Vec3::new(0.0, 20.0, -32.0),
                )
            }
            1 => {
                // Fast canyon race straight up 5th Avenue toward Empire Citadel Tower
                let u = ((t - 5.6) / 5.6).clamp(0.0, 1.0);
                (
                    Vec3::new(11.0 * (1.0 - u) + 18.0 * u, 22.0 + u * 36.0, 62.0 - u * 104.0),
                    Vec3::new(0.0, 26.0 + u * 22.0, -82.0),
                )
            }
            2 => {
                // Dramatic King-Kong orbit around colossal Lexie smashing tower & eating pickled garlic
                let u = ((t - 11.2) / 6.3).clamp(0.0, 1.0);
                let ang = 0.55 - u * 1.25;
                let rumble = (t * 28.0).sin() * 0.45;
                (
                    Vec3::new(
                        ang.sin() * 34.0 + rumble,
                        56.0 + u * 6.0 + rumble * 0.5,
                        -75.5 + ang.cos() * 32.0,
                    ),
                    Vec3::new(0.0, 52.0, -76.5),
                )
            }
            _ => {
                // Swoop down to Stan at South Broadway looking up the entire avenue at colossal Lexie
                let u = ((t - 17.5) / 5.0).clamp(0.0, 1.0);
                (
                    Vec3::new(6.0 * (1.0 - u), 6.0 + u * 14.0, 92.0 + u * 14.0),
                    Vec3::new(0.0, 4.0 + u * 28.0, 48.0 - u * 96.0),
                )
            }
        };
        cam_tf.translation = cam_tf.translation.lerp(eye, (dt * 4.0).min(1.0));
        cam_tf.look_at(target, Vec3::Y);
    }

    if active_beat >= 3 && t >= 23.0 && (skipping || !is_voice_speaking()) {
        next_state.set(GameState::CityPlay);
    }
}

pub fn update_city_intro_subtitles(
    stats: Res<PlayerStats>,
    mut spk_q: Query<&mut Text, With<CityIntroSpeakerText>>,
    mut badge_q: Query<&mut Text, (With<CityIntroBeatBadge>, Without<CityIntroSpeakerText>)>,
    mut sub_q: Query<
        &mut Text,
        (
            With<CityIntroSubtitleText>,
            Without<CityIntroSpeakerText>,
            Without<CityIntroBeatBadge>,
        ),
    >,
) {
    let (speaker, badge, line) = match stats.cutscene_beat {
        0 => (
            "NEW YORK CITY - AERIAL OVERLOOK",
            "MANHATTAN | SCENE 1/4",
            "\"Welcome to New York City! Towering skyscrapers, speeding cars, crowds, and 50 golden tendies span across Manhattan!\"",
        ),
        1 => (
            "EMPIRE CITADEL TOWER",
            "MANHATTAN | SCENE 2/4",
            "\"Look! High atop the Empire Citadel Tower, a King-Kong-sized colossus is tearing the skyline apart!\"",
        ),
        2 => (
            "LEXIE - PICKLED GARLIC TITAN",
            "MANHATTAN | SCENE 3/4",
            "\"HAHAHA! Nobody stops Lexie and her extra-stinky Pickled Garlic Cloves! Eat garlic fumes, tiny New York!\"",
        ),
        _ => (
            "MISSION BRIEFING - SKYSCRAPER TITAN",
            "MANHATTAN | SCENE 4/4",
            "\"Use the corner radar to collect all 50 TENDIES! Grow to SKYSCRAPER SCALE, dodge the cars & garlic fumes, and defeat Lexie!\"",
        ),
    };
    if let Ok(mut t) = spk_q.get_single_mut() {
        t.0 = speaker.to_string();
    }
    if let Ok(mut t) = badge_q.get_single_mut() {
        t.0 = badge.to_string();
    }
    if let Ok(mut t) = sub_q.get_single_mut() {
        t.0 = line.to_string();
    }
}

pub fn exit_city_intro_cutscene(
    mut commands: Commands,
    ui_q: Query<Entity, With<CityIntroOverlay>>,
) {
    for e in &ui_q {
        commands.entity(e).despawn_recursive();
    }
}

// =========================================================================
// PART 4: NEW YORK CITY GAMEPLAY, MINIMAP, TRAFFIC, NPCS & LEXIE GARLIC BOSS
// =========================================================================

pub fn enter_city_play(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut stats: ResMut<PlayerStats>,
    mut orbit: ResMut<CameraOrbit>,
    mut player_q: Query<(&mut Transform, &mut Player)>,
    mut hud_q: Query<&mut Node, With<HudOverlay>>,
    existing_city_q: Query<Entity, With<CityWorldMarker>>,
    old_level_q: Query<Entity, (Or<(With<LevelEntity>, With<Enemy>, With<FiberPuddle>)>, Without<Player>)>,
) {
    for mut node in &mut hud_q {
        node.display = Display::Flex;
    }
    stats.sync_tendy_wendies_from_save();
    stats.in_casino_level = false;
    stats.casino_open = false;
    stats.bartender_open = false;
    stats.char_select_open = false;

    spawn_nyc_world_if_needed(
        &mut commands,
        &asset_server,
        &mut meshes,
        &mut materials,
        &existing_city_q,
        &old_level_q,
    );
    commands.insert_resource(ClearColor(Color::srgb(0.03, 0.04, 0.09)));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.75, 0.80, 0.95),
        brightness: 440.0,
    });

    let resuming_from_tj = stats.tj_return_city
        || stats.mike_return_city
        || stats.derek_return_city
        || stats.nate_return_city
        || stats.glenn_return_city
        || stats.chillary_return_city
        || stats.casino_return_city;
    let from_mike = stats.mike_return_city;
    let from_derek = stats.derek_return_city;
    let from_nate = stats.nate_return_city;
    let from_glenn = stats.glenn_return_city;
    let from_chillary = stats.chillary_return_city;
    let from_casino = stats.casino_return_city;
    stats.tj_return_city = false;
    stats.mike_return_city = false;
    stats.derek_return_city = false;
    stats.nate_return_city = false;
    stats.glenn_return_city = false;
    stats.chillary_return_city = false;
    stats.casino_return_city = false;
    if from_casino {
        stats.player_scale = stats.saved_casino_city_scale.clamp(1.8, 18.0);
    }
    let is_titan_preview = !resuming_from_tj && stats.in_city && stats.tendies >= 50;
    stats.in_city = true;
    stats.boss_mode = true;
    stats.total_tendies = 50;
    stats.wave = 4;

    if !stats.free_roam || stats.game_beaten {
        crate::audio::mark_mom_defeated();
    }
    if resuming_from_tj {
        // Crew progress rides along with every recruit message; the fourth recruit is then
        // celebrated by `update_city_crew_gate`.
        let crew_line = if stats.gate_open() {
            "Gate 5/5!".to_string()
        } else {
            format!(
                "Before tendies ({}/5): {}.",
                stats.gate_steps_done(),
                stats.missing_gate_steps().join(", ")
            )
        };
        stats.hint_text = if from_mike {
            format!("SMOKE BREAK WITH MIKE DONE! +3 Hearts, Extra Woozy. {}", crew_line)
        } else if from_derek {
            format!("DEREK JOINED YOUR SQUAD! +2 Hearts, +25 Credits. {}", crew_line)
        } else if from_nate {
            format!("NATE JOINED YOUR SQUAD! +2 Hearts, +25 Credits. {}", crew_line)
        } else if from_glenn {
            format!("GLENN JOINED YOUR SQUAD! +2 Hearts, +25 Credits. {}", crew_line)
        } else if from_chillary {
            // Optional boardwalk easter egg completed; crew gate stays unchanged.
            "JUST CHILLIN' IN CEDAR RAPIDS! Chillary's Snapchat is live.".to_string()
        } else if from_casino {
            "Back in Pub 217 from Tendy Palace Casino! Your Tendy Wendies (TW) are banked.".to_string()
        } else {
            format!("TJ JOINED YOUR SQUAD! {}", crew_line)
        };
    } else {
        stats.boss_hp = 300.0;
        stats.boss_max_hp = 300.0;
        stats.max_hearts = 20;
        stats.hearts = stats.max_hearts;
        stats.estus_flasks = stats.max_estus_flasks;
        stats.tj_recruited = false;
        stats.derek_recruited = false;
        stats.nate_recruited = false;
        stats.glenn_recruited = false;
        stats.mike_smoked = false;
        stats.crew_unlock_announced = false;
        stats.lexie_defeated = false;
        // Gate progress restored from a checkpoint ([C] continue); 0 for a brand new run.
        let restore = std::mem::take(&mut stats.restore_gate_mask);
        stats.apply_gate_mask(restore);
        // A restored, already-open gate doesn't replay the "gate open" moment.
        stats.crew_unlock_announced = stats.gate_open();

        if stats.free_roam {
            // Post-game free roam (Continue after Lexie, a beaten save, `free_roam=1`, or a
            // restart/retry inside free roam): normal size, crew at their spots, no Lexie.
            stats.prepare_free_roam();
            stats.hint_text =
                "FREE ROAM! Lexie is gone for good. Scoot around Cedar Rapids, hit the stunt gates and say hi to the crew in Pub 217."
                    .to_string();
        } else if let Some(preset_tendies) = stats.test_crew_preset.take() {
            // `city_crew=1` / `city_crew_kaiju=1` test links: whole gate already done.
            stats.apply_gate_mask(31);
            stats.crew_unlock_announced = false;
            stats.tendies = preset_tendies.min(stats.total_tendies);
            stats.player_scale = stats.target_scale();
            stats.hint_text = "TEST: crew preset loaded.".to_string();
        } else if is_titan_preview {
            stats.player_scale = 18.0;
            stats.hint_text =
                "SKYSCRAPER KAIJU! Slash Lexie with your Lightsaber [CLICK / SLASH] & Dodge Roll garlic!"
                    .to_string();
        } else {
            stats.tendies = 0;
            stats.player_scale = 1.8;
            stats.hint_text =
                if stats.gate_open() {
                    "BACK IN CEDAR RAPIDS! Your crew is with you: eat 50 tendies and take down Lexie!"
                        .to_string()
                } else {
                    "CEDAR RAPIDS AT NIGHT! Before the tendies: find TJ, Derek, Nate & Glenn somewhere in the city (they shout when you get close), and smoke with Mike in Pub 217."
                        .to_string()
                };
        }
    }

    // Saved after the gate flags are settled so a recruit (or a restored gate) is persisted;
    // skipped in direct pre-victory Free Mode so an unfinished story checkpoint isn't overwritten.
    if !stats.free_roam || stats.game_beaten {
        crate::ui::save_game_checkpoint("city", &stats);
    }

    if let Ok((mut p_tf, mut player)) = player_q.get_single_mut() {
        p_tf.translation = if resuming_from_tj {
            stats.saved_city_pos
        } else if is_titan_preview {
            Vec3::new(4.2, 0.0, -18.0)
        } else {
            CITY_START_POS
        };
        p_tf.scale = Vec3::splat(stats.player_scale);
        player.velocity = Vec3::ZERO;
        // Resuming from a city cutscene keeps the facing the hero had before it (recorded by
        // `record_city_facing`); a fresh city entry faces north.
        player.facing_yaw = if resuming_from_tj { orbit.saved_city_facing } else { 0.0 };
        p_tf.rotation = Quat::from_rotation_y(player.facing_yaw);
    }

    // Only a fresh city entry resets the orbit; resumes keep the player's camera framing.
    if !resuming_from_tj {
        orbit.yaw = if is_titan_preview { -0.24 } else { 0.0 };
        orbit.pitch = if is_titan_preview { 0.42 } else { 0.36 };
        orbit.distance = if is_titan_preview { 15.5 } else { 11.2 };
        orbit.current_dist = orbit.distance;
        if !is_titan_preview {
            debug_city_start(&mut stats, &mut orbit, &mut player_q);
        }
    }

    set_touch_playing(true);
    set_touch_boss_mode(true);
    // The Mike/Derek/Nate/Glenn cutscene exits already restart the chant in this same
    // StateTransition; starting it again here would restart the track twice in one frame.
    // TJ's exit doesn't touch music, so a TJ resume (and any fresh entry) starts it here.
    if !(from_mike || from_derek || from_nate || from_glenn) {
        play_viking_chant_music();
    }

    // Spawn Lexie Top-Center Boss Bar (not in free roam) + Corner Cedar Rapids Tendy Radar Minimap
    if !stats.free_roam {
    commands
        .spawn((
            CityPlayUi,
            LexieBossHudRoot,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(8.0),
                bottom: Val::Auto,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    width: Val::Percent(44.0),
                    min_width: Val::Px(220.0),
                    max_width: Val::Px(460.0),
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(7.0)),
                    row_gap: Val::Px(4.0),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.14, 0.04, 0.12, 0.88)),
                BorderColor(Color::srgb(0.95, 0.32, 0.68)),
                BorderRadius::all(Val::Px(10.0)),
            ))
            .with_children(|bar| {
                bar.spawn((
                    LexieBossHudText,
                    Text::new("LEXIE - KING-KONG PICKLED GARLIC QUEEN"),
                    TextFont {
                        font_size: 13.5,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.84, 0.32)),
                ));
                bar.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(12.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.20, 0.14, 0.18)),
                    BorderRadius::all(Val::Px(6.0)),
                ))
                .with_children(|track| {
                    track.spawn((
                        LexieBossHpFill,
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.94, 0.22, 0.56)),
                        BorderRadius::all(Val::Px(6.0)),
                    ));
                });
            });
        });
    }

    // Top-Right Corner Cedar Rapids Radar Minimap (clears Score box and mobile Help/Sound pills)
    commands
        .spawn((
            CityPlayUi,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(132.0),
                right: Val::Px(12.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(6.0)),
                row_gap: Val::Px(4.0),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.08, 0.14, 0.88)),
            BorderColor(Color::srgb(0.45, 0.86, 1.0)),
            BorderRadius::all(Val::Px(10.0)),
        ))
        .with_children(|box_ui| {
            box_ui.spawn((
                Text::new("CEDAR RAPIDS RADAR"),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.86, 0.28)),
            ));
            box_ui
                .spawn((
                    Node {
                        position_type: PositionType::Relative,
                        width: Val::Px(112.0),
                        height: Val::Px(112.0),
                        overflow: Overflow::clip(),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.08, 0.13, 0.20, 0.95)),
                    BorderColor(Color::srgba(0.55, 0.72, 0.88, 0.45)),
                    BorderRadius::all(Val::Px(6.0)),
                ))
                .with_children(|radar| {
                    // Subtle Avenue & Cross-Street radar grid lines
                    for pct in [16.0_f32, 33.0, 50.0, 67.0, 84.0] {
                        radar.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Percent(pct),
                                top: Val::Px(0.0),
                                width: Val::Px(1.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.45, 0.58, 0.72, 0.22)),
                        ));
                        radar.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                top: Val::Percent(pct),
                                left: Val::Px(0.0),
                                width: Val::Percent(100.0),
                                height: Val::Px(1.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.45, 0.58, 0.72, 0.22)),
                        ));
                    }

                    // 50 Pooled Golden Tendy Dots
                    for _ in 0..50 {
                        radar.spawn((
                            CityMinimapTendyDot,
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Percent(50.0),
                                top: Val::Percent(50.0),
                                width: Val::Px(4.0),
                                height: Val::Px(4.0),
                                margin: UiRect::all(Val::Px(-2.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(1.0, 0.84, 0.14)),
                            BorderRadius::all(Val::Px(2.0)),
                        ));
                    }

                    // Green crew markers (TJ, Derek, Nate, Glenn once they have called out),
                    // amber Pub 217 marker (Mike), marker 5 is the driver's license card
                    // (after a car first refuses Stan).
                    for idx in 0..6_u8 {
                        radar.spawn((
                            CityMinimapCrewDot(idx),
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Percent(50.0),
                                top: Val::Percent(50.0),
                                width: Val::Px(10.0),
                                height: Val::Px(10.0),
                                margin: UiRect::all(Val::Px(-5.0)),
                                border: UiRect::all(Val::Px(2.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.20, 1.0, 0.45, 0.35)),
                            BorderColor(Color::srgb(0.35, 1.0, 0.55)),
                            BorderRadius::all(Val::Px(5.0)),
                            Visibility::Hidden,
                        ));
                    }

                    // Magenta Boss Dot (Lexie)
                    radar.spawn((
                        CityMinimapBossDot,
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Percent(50.0),
                            top: Val::Percent(20.0),
                            width: Val::Px(9.0),
                            height: Val::Px(9.0),
                            margin: UiRect::all(Val::Px(-4.5)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.96, 0.22, 0.62)),
                        BorderRadius::all(Val::Px(5.0)),
                    ));

                    // Bright Cyan Player Dot (Stan)
                    radar.spawn((
                        CityMinimapPlayerDot,
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Percent(50.0),
                            top: Val::Percent(80.0),
                            width: Val::Px(8.0),
                            height: Val::Px(8.0),
                            margin: UiRect::all(Val::Px(-4.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.18, 0.96, 1.0)),
                        BorderRadius::all(Val::Px(4.0)),
                    ));
                });
        });
}

/// Debug only (`?debug=1&city_start=...`): drops Stan somewhere useful for screenshots on a fresh
/// city entry. `pub` = outside the pub door facing it, `pubin` = inside the pub, `door` = on the doorstep, `sky` = high
/// view from the pub roof over its block, `crew` = 9 m from Derek's waiting spot (inside his call-out range).
/// `park`, `parksky`, `lot`, `pipe`, `plaza` = skate park views (`skatepark::debug_start_pose`).
fn debug_city_start(
    stats: &mut PlayerStats,
    orbit: &mut CameraOrbit,
    player_q: &mut Query<(&mut Transform, &mut Player)>,
) {
    let mode = crate::audio::debug_city_start();
    if mode.is_empty() {
        return;
    }
    let Ok((mut p_tf, mut player)) = player_q.get_single_mut() else {
        return;
    };
    // (position, facing direction)
    let (pos, face) = match mode.as_str() {
        "pub" => (PUB217_ANCHOR + Vec3::new(9.0, 0.0, 4.0), Vec3::new(-1.0, 0.0, -0.45)),
        "pubin" => (PUB217_ANCHOR + Vec3::new(-1.6, 0.0, 0.0), Vec3::NEG_X),
        "door" => (PUB217_ANCHOR + Vec3::new(1.8, 0.0, 0.0), Vec3::NEG_X),
        // On the pub roof (the camera clamps only apply inside), looking west over the block.
        "sky" => (PUB217_ANCHOR + Vec3::new(-10.0, 10.5, 0.0), Vec3::NEG_X),
        // `wheel`, `wheelst`, `wheelfar`, `wheelriver`: at / near the Ferris wheel (ferris.rs).
        m if m.starts_with("wheel") => match crate::ferris::debug_start_pose(m) {
            Some((pos, face, cam)) => {
                if let Some((pitch, dist)) = cam {
                    orbit.pitch = pitch;
                    orbit.distance = dist;
                    orbit.current_dist = dist;
                }
                (pos, face)
            }
            None => return,
        },
        "crew" => {
            // With this flag the world used `DEBUG_CREW_SEED`, so the spots are known here:
            // stand 9 m out in the street in front of Derek, looking at him.
            let (spot, yaw) = pick_crew_spots(DEBUG_CREW_SEED)[CREW_DEREK];
            let out = Vec3::new(yaw.sin(), 0.0, yaw.cos());
            (spot + out * 9.0, -out)
        }
        m => match crate::skatepark::debug_start_pose(m) {
            Some((pos, face, _, _)) => (pos, face),
            // `ramp`, `rampback`, `rampside`, `rampcar`: at a stunt wedge ramp (stunts.rs).
            None => match crate::stunts::debug_start_pose(m) {
                Some(p) => p,
                None => return,
            },
        },
    };
    p_tf.translation = pos;
    // Stan's forward is -Z at yaw 0; the orbit camera sits behind him at the same yaw.
    let yaw = (-face.x).atan2(-face.z);
    player.facing_yaw = yaw;
    p_tf.rotation = Quat::from_rotation_y(yaw);
    orbit.yaw = yaw;
    if mode == "sky" {
        orbit.pitch = 1.2;
        orbit.distance = 40.0;
        orbit.current_dist = orbit.distance;
    }
    if let Some((_, _, pitch, dist)) = crate::skatepark::debug_start_pose(&mode) {
        orbit.pitch = pitch;
        orbit.distance = dist;
        orbit.current_dist = dist;
    }
    stats.hint_text = format!("DEBUG: city_start={}", mode);
}

/// OnExit(CityPlay): remember the hero's facing so a cutscene resume can restore it
/// (cutscene stages re-pose the hero and reset `facing_yaw`).
pub fn record_city_facing(mut orbit: ResMut<CameraOrbit>, player_q: Query<&Player>) {
    if let Ok(player) = player_q.get_single() {
        orbit.saved_city_facing = player.facing_yaw;
    }
}

pub fn exit_city_play(
    mut commands: Commands,
    mut stats: ResMut<PlayerStats>,
    ui_q: Query<Entity, With<CityPlayUi>>,
    phone_q: Query<Entity, With<crate::scooter::PhoneMazeUi>>,
    scoot_ui_q: Query<Entity, With<crate::scooter::ScootPromptUi>>,
    mut player_q: Query<&mut Visibility, With<Player>>,
) {
    for e in &ui_q {
        commands.entity(e).despawn_recursive();
    }
    for e in &phone_q {
        commands.entity(e).despawn_recursive();
    }
    for e in &scoot_ui_q {
        commands.entity(e).despawn_recursive();
    }
    stats.driving_car = None;
    stats.riding_scooter = None;
    stats.scooter_gravity_scale = 1.0;
    stats.riding_wheel = false;
    stats.phone_open = false;
    stats.casino_open = false;
    stats.bartender_open = false;
    stats.char_select_open = false;
    if let Ok(mut vis) = player_q.get_single_mut() {
        *vis = Visibility::Visible;
    }
    set_touch_boss_mode(false);
}

fn wrap_angle(mut a: f32) -> f32 {
    while a > std::f32::consts::PI {
        a -= std::f32::consts::TAU;
    }
    while a < -std::f32::consts::PI {
        a += std::f32::consts::TAU;
    }
    a
}

pub fn update_city_driving(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut stats: ResMut<PlayerStats>,
    mut orbit: ResMut<CameraOrbit>,
    mut player_q: Query<(&mut Transform, &mut Player, &mut Visibility)>,
    mut car_q: Query<
        (Entity, &mut Transform, &mut CityCar, &Children),
        (
            Without<Player>,
            Without<CityCarWheel>,
            Without<PlatformBox>,
            Without<CityNpc>,
            Without<LexieBoss>,
            Without<GarlicProjectile>,
        ),
    >,
    mut wheel_q: Query<
        (&mut Transform, &CityCarWheel),
        (
            Without<Player>,
            Without<CityCar>,
            Without<PlatformBox>,
            Without<CityNpc>,
            Without<LexieBoss>,
            Without<GarlicProjectile>,
        ),
    >,
    platform_q: Query<
        (&Transform, &PlatformBox),
        (
            Without<Player>,
            Without<CityCar>,
            Without<CityCarWheel>,
            Without<CityNpc>,
            Without<LexieBoss>,
            Without<GarlicProjectile>,
        ),
    >,
    mut npc_q: Query<
        (&mut Transform, &mut CityNpc),
        (
            Without<Player>,
            Without<CityCar>,
            Without<CityCarWheel>,
            Without<PlatformBox>,
            Without<LexieBoss>,
            Without<GarlicProjectile>,
        ),
    >,
    mut lexie_q: Query<
        (&Transform, &mut LexieBoss),
        (
            Without<Player>,
            Without<CityCar>,
            Without<CityCarWheel>,
            Without<PlatformBox>,
            Without<CityNpc>,
            Without<GarlicProjectile>,
        ),
    >,
    garlic_q: Query<
        (Entity, &Transform),
        (
            With<GarlicProjectile>,
            Without<Player>,
            Without<CityCar>,
            Without<CityCarWheel>,
            Without<PlatformBox>,
            Without<CityNpc>,
            Without<LexieBoss>,
        ),
    >,
) {
    let dt = time.delta_secs().min(0.05);
    if stats.wanted_stars > 0 {
        stats.wanted_decay_timer -= dt;
        if stats.wanted_decay_timer <= 0.0 {
            stats.wanted_stars -= 1;
            stats.wanted_decay_timer = 14.0;
        }
    }
    let Ok((mut p_tf, mut player, mut p_vis)) = player_q.get_single_mut() else {
        return;
    };

    let interact = keys.just_pressed(KeyCode::KeyF)
        || keys.just_pressed(KeyCode::KeyE)
        || std::mem::take(&mut stats.car_prompt_tap);

    // 1. If currently on foot, check for nearby hijackable car
    if stats.driving_car.is_none() && stats.riding_scooter.is_none() && !stats.riding_wheel && !stats.char_select_open {
        if stats.player_scale >= 6.0 {
            return;
        }
        let mut nearest: Option<(Entity, f32)> = None;
        for (car_e, car_tf, _car, _) in &car_q {
            let d = Vec2::new(
                car_tf.translation.x - p_tf.translation.x,
                car_tf.translation.z - p_tf.translation.z,
            )
            .length();
            if d < 5.4 && nearest.map_or(true, |(_, bd)| d < bd) {
                nearest = Some((car_e, d));
            }
        }
        if let Some((car_e, _)) = nearest {
            if interact && !stats.has_license {
                // No car can be driven until the license card is found (`update_city_props`).
                refuse_no_license(&mut stats);
            } else if interact {
                if let Ok((_, mut car_tf, mut car, _)) = car_q.get_mut(car_e) {
                    car.occupied = true;
                    car.parked = false;
                    car.car_hp = car.car_hp.max(40.0);
                    let fwd = Vec2::new(car.yaw.sin(), car.yaw.cos());
                    if !car.parked && car.velocity.length_squared() < 1.0 {
                        car.velocity = fwd * (car.speed * 0.45);
                    }
                    stats.driving_car = Some(car_e);
                    *p_vis = Visibility::Hidden;
                    p_tf.translation = car_tf.translation;
                    player.velocity = Vec3::ZERO;
                    player.grounded = true;
                    orbit.yaw = wrap_angle(car.yaw + std::f32::consts::PI);
                    car_tf.rotation = Quat::from_rotation_y(car.yaw);
                    play_sfx("car_door");
                    play_sfx("car_horn");
                    stats.wanted_stars = (stats.wanted_stars + 1).min(5);
                    stats.wanted_decay_timer = 14.0;
                    stats.hint_text =
                        "GRAND THEFT TENDY! (Uber rating: 0 stars) [WASD] Drive | [SPACE] Drift | [SHIFT] Nitro | [F/E] Exit"
                            .to_string();
                }
            } else if stats.hint_text.starts_with("Collect 50")
                || stats.hint_text.starts_with("TJ JOINED")
                || stats.hint_text.starts_with("EXITED")
                || stats.hint_text.is_empty()
            {
                stats.hint_text = "[F / E] Hijack & Drive Car!".to_string();
            }
        } else if stats.hint_text == "[F / E] Hijack & Drive Car!" {
            stats.hint_text =
                "Collect 50 Tendies (use top-right Radar)! Meet TJ & Dodge Lexie's Pickled Garlic!"
                    .to_string();
        }
        return;
    }

    let Some(driven_e) = stats.driving_car else {
        return;
    };

    // 2. Handle exit / GTA IV high-speed bail-out
    if interact {
        if let Ok((_, car_tf, mut car, _)) = car_q.get_mut(driven_e) {
            car.occupied = false;
            car.parked = true;
            let right = Vec2::new(car.yaw.cos(), -car.yaw.sin());
            let speed = car.velocity.length();
            p_tf.translation = Vec3::new(
                car_tf.translation.x - right.x * 2.35,
                0.0,
                car_tf.translation.z - right.y * 2.35,
            );
            if speed > 14.0 {
                player.velocity = Vec3::new(car.velocity.x * 0.55, 5.2, car.velocity.y * 0.55);
                player.roll_timer = 0.55;
                player.invuln_timer = 0.8;
                play_sfx("dash");
                stats.hint_text = "GTA BAIL-OUT! Find another car or collect Tendies!".to_string();
            } else {
                player.velocity = Vec3::ZERO;
                play_sfx("car_door");
                stats.hint_text = "EXITED CAR! Press [F / E] near any car to drive.".to_string();
            }
        }
        stats.driving_car = None;
        *p_vis = Visibility::Visible;
        return;
    }

    // 3. Driving physics for the occupied car
    let mut driven_pos = Vec3::ZERO;
    let mut driven_vel = Vec2::ZERO;
    let mut driven_yaw = 0.0_f32;
    let honking = keys.just_pressed(KeyCode::KeyG) || mouse_buttons.just_pressed(MouseButton::Left);

    if let Ok((_, mut car_tf, mut car, children)) = car_q.get_mut(driven_e) {
        if car.horn_cooldown > 0.0 {
            car.horn_cooldown = (car.horn_cooldown - dt).max(0.0);
        }
        if car.drift_sfx_cooldown > 0.0 {
            car.drift_sfx_cooldown = (car.drift_sfx_cooldown - dt).max(0.0);
        }

        let mut throttle = 0.0_f32;
        if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
            throttle += 1.0;
        }
        if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
            throttle -= 1.0;
        }
        throttle += crate::audio::touch_move_y();
        throttle = throttle.clamp(-1.0, 1.0);

        let mut steer_cmd = 0.0_f32;
        if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
            steer_cmd += 1.0;
        }
        if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
            steer_cmd -= 1.0;
        }
        steer_cmd -= crate::audio::touch_move_x();
        steer_cmd = steer_cmd.clamp(-1.0, 1.0);

        let handbrake = keys.pressed(KeyCode::Space) || crate::audio::take_touch_jump();
        let nitro_held = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        let nitro_active = nitro_held && throttle > 0.1 && stats.dash_energy > 2.0;
        if nitro_active {
            stats.dash_energy = (stats.dash_energy - 38.0 * dt).max(0.0);
        }

        // Smooth steering input
        car.steer_input += (steer_cmd - car.steer_input) * (dt * 9.0).min(1.0);

        let fwd = Vec2::new(car.yaw.sin(), car.yaw.cos());
        let right = Vec2::new(car.yaw.cos(), -car.yaw.sin());
        let mut fwd_speed = car.velocity.dot(fwd);
        let mut lat_speed = car.velocity.dot(right);

        // Longitudinal acceleration / braking / reverse
        let max_fwd = if nitro_active { 52.0 } else { 36.0 };
        let accel_force = if nitro_active { 46.0 } else { 29.0 };
        let mut pitch_target = 0.0_f32;

        if throttle > 0.05 {
            if fwd_speed < -1.0 {
                // Braking from reverse
                fwd_speed += 44.0 * dt;
                pitch_target = 0.085;
            } else {
                fwd_speed = (fwd_speed + accel_force * throttle * dt).min(max_fwd);
                pitch_target = if nitro_active { -0.095 } else { -0.055 };
            }
        } else if throttle < -0.05 {
            if fwd_speed > 1.5 {
                // Heavy GTA IV brake nose-dive
                fwd_speed = (fwd_speed - 48.0 * (-throttle) * dt).max(0.0);
                pitch_target = 0.105;
            } else {
                fwd_speed = (fwd_speed - 17.5 * (-throttle) * dt).max(-15.5);
                pitch_target = 0.04;
            }
        } else {
            // Rolling coast resistance
            fwd_speed *= (1.0 - 1.35 * dt).max(0.0);
        }

        if handbrake {
            fwd_speed *= (1.0 - 2.6 * dt).max(0.0);
            pitch_target += 0.05;
        }

        // Speed-sensitive steering (reverse flips steering direction)
        let speed_factor = (fwd_speed.abs() / 11.0).clamp(0.0, 1.0);
        let high_speed_damp = 1.0 - (fwd_speed.abs() / 65.0).clamp(0.0, 0.35);
        let dir_sign = if fwd_speed >= -0.4 { 1.0 } else { -1.0 };
        let drift_turn_mul = if handbrake { 1.48 } else { 1.0 };
        let turn_rate = 2.45 * speed_factor * high_speed_damp * drift_turn_mul * dir_sign;
        car.yaw = wrap_angle(car.yaw + car.steer_input * turn_rate * dt);

        // Recompute basis after yaw update, then damp lateral slip (handbrake = drift!)
        let new_fwd = Vec2::new(car.yaw.sin(), car.yaw.cos());
        let new_right = Vec2::new(car.yaw.cos(), -car.yaw.sin());
        let lat_grip = if handbrake { 1.65 } else { 7.8 };
        lat_speed *= (1.0 - lat_grip * dt).max(0.0);

        if (handbrake || lat_speed.abs() > 7.5) && fwd_speed.abs() > 8.0 && car.drift_sfx_cooldown <= 0.0
        {
            play_sfx("car_drift");
            car.drift_sfx_cooldown = 0.28;
        }

        car.velocity = new_fwd * fwd_speed + new_right * lat_speed;

        // GTA IV heavy body roll & pitch squat/dive
        let roll_target = (-lat_speed * 0.017).clamp(-0.18, 0.18);
        car.roll_lean += (roll_target - car.roll_lean) * (dt * 8.5).min(1.0);
        car.pitch_lean += (pitch_target - car.pitch_lean) * (dt * 9.0).min(1.0);

        car_tf.translation.x = (car_tf.translation.x + car.velocity.x * dt).clamp(-128.0, 128.0);
        car_tf.translation.z = (car_tf.translation.z + car.velocity.y * dt).clamp(-128.0, 128.0);
        car_tf.translation.y = 0.0;

        // Building AABB collisions (bounce + scrape cleanly along skyscrapers)
        let wall_r = 1.95;
        for _ in 0..2 {
            for (b_tf, b_box) in &platform_q {
                if b_tf.translation.y - b_box.half_extents.y > 2.2 {
                    continue;
                }
                // Only boxes that rise above the road are walls. Without this the city ground
                // slab (top at y = 0, covering the whole map) counted as a building the car was
                // inside, and the push-out threw every driven car to the edge of the map.
                if b_tf.translation.y + b_box.half_extents.y <= car_tf.translation.y + 0.3 {
                    continue;
                }
                let min_x = b_tf.translation.x - b_box.half_extents.x - wall_r;
                let max_x = b_tf.translation.x + b_box.half_extents.x + wall_r;
                let min_z = b_tf.translation.z - b_box.half_extents.z - wall_r;
                let max_z = b_tf.translation.z + b_box.half_extents.z + wall_r;
                if car_tf.translation.x > min_x
                    && car_tf.translation.x < max_x
                    && car_tf.translation.z > min_z
                    && car_tf.translation.z < max_z
                {
                    let dx_l = car_tf.translation.x - min_x;
                    let dx_r = max_x - car_tf.translation.x;
                    let dz_b = car_tf.translation.z - min_z;
                    let dz_f = max_z - car_tf.translation.z;
                    let m = dx_l.min(dx_r).min(dz_b).min(dz_f);
                    let impact;
                    if (m - dx_l).abs() < 0.001 {
                        car_tf.translation.x = min_x;
                        impact = car.velocity.x.abs();
                        car.velocity.x = -car.velocity.x.max(0.0) * 0.28;
                    } else if (m - dx_r).abs() < 0.001 {
                        car_tf.translation.x = max_x;
                        impact = car.velocity.x.abs();
                        car.velocity.x = -car.velocity.x.min(0.0) * 0.28;
                    } else if (m - dz_b).abs() < 0.001 {
                        car_tf.translation.z = min_z;
                        impact = car.velocity.y.abs();
                        car.velocity.y = -car.velocity.y.max(0.0) * 0.28;
                    } else {
                        car_tf.translation.z = max_z;
                        impact = car.velocity.y.abs();
                        car.velocity.y = -car.velocity.y.min(0.0) * 0.28;
                    }
                    if impact > 9.5 && car.horn_cooldown <= 0.0 {
                        play_sfx("car_crash");
                        car.horn_cooldown = 0.35;
                        car.pitch_lean = 0.14;
                    }
                }
            }
        }
        // Pub 217 is walk-in only: keep driven cars outdoors.
        if let Some(n) = push_car_out_of_pub217(&mut car_tf, car.yaw) {
            let into = car.velocity.dot(n).min(0.0);
            car.velocity -= n * into * 1.28;
        }

        car_tf.rotation = Quat::from_rotation_y(car.yaw)
            * Quat::from_rotation_x(car.pitch_lean)
            * Quat::from_rotation_z(car.roll_lean);

        // Steer front wheels visually
        for &child in children.iter() {
            if let Ok((mut w_tf, w)) = wheel_q.get_mut(child) {
                let steer_yaw = if w.is_front { car.steer_input * 0.42 } else { 0.0 };
                w_tf.rotation = Quat::from_rotation_y(steer_yaw)
                    * Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
            }
        }

        if honking && car.horn_cooldown <= 0.0 {
            play_sfx("car_horn");
            car.horn_cooldown = 0.32;
        }

        // Sync Stan's position to car for Tendy pickups, radar, and TJ trigger
        p_tf.translation = car_tf.translation;
        p_tf.rotation = Quat::from_rotation_y(car.yaw);

        // Smooth GTA IV chase camera auto-align while steering/driving
        if car.velocity.length() > 2.5 && !mouse_buttons.pressed(MouseButton::Right) {
            let target_cam_yaw = wrap_angle(car.yaw + std::f32::consts::PI);
            let diff = wrap_angle(target_cam_yaw - orbit.yaw);
            // Paused for ~1.5 s after manual look input, then eases back in.
            let align = (1.0 - (-2.8 * dt).exp()) * orbit.auto_align_weight();
            orbit.yaw = wrap_angle(orbit.yaw + diff * align);
        }

        driven_pos = car_tf.translation;
        driven_vel = car.velocity;
        driven_yaw = car.yaw;
    }

    // 4. Car-to-Car GTA collisions
    let my_speed = driven_vel.length();
    let mut car_hit_impulse = Vec2::ZERO;
    for (other_e, mut other_tf, mut other_car, _) in &mut car_q {
        if other_e == driven_e {
            continue;
        }
        let delta = Vec2::new(
            other_tf.translation.x - driven_pos.x,
            other_tf.translation.z - driven_pos.z,
        );
        let dist = delta.length();
        if dist < 3.4 && dist > 0.001 {
            let n = delta / dist;
            other_tf.translation.x += n.x * (3.4 - dist);
            other_tf.translation.z += n.y * (3.4 - dist);
            if my_speed > 5.0 {
                other_car.knock_vel += n * (my_speed * 0.85) + driven_vel * 0.35;
                other_car.yaw = wrap_angle(other_car.yaw + (n.x * driven_vel.y - n.y * driven_vel.x) * 0.03);
                if other_car.horn_cooldown <= 0.0 {
                    play_sfx("car_crash");
                    play_sfx("car_horn");
                    other_car.horn_cooldown = 0.9;
                }
                car_hit_impulse -= n * (my_speed * 0.32);
            }
        }
    }
    if car_hit_impulse.length_squared() > 0.01 {
        if let Ok((_, _, mut car, _)) = car_q.get_mut(driven_e) {
            car.velocity += car_hit_impulse;
            car.pitch_lean = 0.12;
        }
    }

    // 5. Pedestrians scatter away from speeding/honking driven car
    if my_speed > 6.0 || honking {
        for (mut npc_tf, mut npc) in &mut npc_q {
            let diff = Vec2::new(
                npc_tf.translation.x - driven_pos.x,
                npc_tf.translation.z - driven_pos.z,
            );
            let dist = diff.length();
            if dist < 9.5 && dist > 0.01 {
                let away = diff / dist;
                npc_tf.translation.x += away.x * 14.0 * dt;
                npc_tf.translation.z += away.y * 14.0 * dt;
                npc.home_x = npc_tf.translation.x;
                npc.home_z = npc_tf.translation.z;
            }
        }
    }

    // 6. Smash Lexie's Garlic projectiles & ram Lexie at high speed!
    for (g_e, g_tf) in &garlic_q {
        let gd = Vec2::new(g_tf.translation.x - driven_pos.x, g_tf.translation.z - driven_pos.z).length();
        if gd < 2.9 && (g_tf.translation.y - driven_pos.y).abs() < 2.6 {
            commands.entity(g_e).despawn_recursive();
            play_sfx("car_crash");
        }
    }
    if my_speed > 11.0 {
        for (l_tf, mut lexie) in &mut lexie_q {
            if stats.lexie_defeated || lexie.on_tower {
                continue;
            }
            let ld = Vec2::new(l_tf.translation.x - driven_pos.x, l_tf.translation.z - driven_pos.z).length();
            if ld < 4.5 && lexie.hit_flash <= 0.0 {
                let dmg = 14.0 + my_speed * 0.35;
                // Applied to Lexie's own HP exactly like a saber hit: chip damage can't take her
                // below 75 until all 50 tendies are eaten, and `update_city_lexie` handles defeat.
                lexie.hp = if stats.tendies < 50 {
                    (lexie.hp - dmg).max(75.0)
                } else {
                    lexie.hp - dmg
                };
                stats.boss_hp = lexie.hp.max(0.0);
                stats.score += 350;
                lexie.hit_flash = 0.45;
                play_sfx("car_crash");
                play_sfx("car_horn");
                stats.hint_text = format!("CAR RAMMED LEXIE! ({:.0} DMG!)", dmg);
                if let Ok((_, _, mut car, _)) = car_q.get_mut(driven_e) {
                    let bounce = Vec2::new(driven_pos.x - l_tf.translation.x, driven_pos.z - l_tf.translation.z).normalize_or_zero();
                    car.velocity = bounce * 10.0;
                    car.yaw = wrap_angle(driven_yaw + 0.35);
                }
            }
        }
    }
}

pub fn update_city_traffic(
    time: Res<Time>,
    state: Res<State<GameState>>,
    mut stats: ResMut<PlayerStats>,
    mut next_state: crate::transition::SceneTransition,
    mut player_q: Query<(&mut Transform, &mut Player)>,
    mut car_q: Query<(&mut Transform, &mut CityCar), Without<Player>>,
) {
    let dt = time.delta_secs().min(0.05);
    let Ok((p_tf, mut player)) = player_q.get_single_mut() else {
        return;
    };
    let in_active_play = *state.get() == GameState::CityPlay;
    let is_driving = stats.driving_car.is_some() || stats.phone_open || stats.bartender_open || stats.char_select_open;
    let hit_radius = 1.9 + stats.player_scale * 0.22;

    for (mut car_tf, mut car) in &mut car_q {
        if car.occupied {
            continue;
        }
        if car.horn_cooldown > 0.0 {
            car.horn_cooldown = (car.horn_cooldown - dt).max(0.0);
        }

        if car.knock_vel.length_squared() > 0.05 {
            car_tf.translation.x = (car_tf.translation.x + car.knock_vel.x * dt).clamp(-132.0, 132.0);
            car_tf.translation.z = (car_tf.translation.z + car.knock_vel.y * dt).clamp(-132.0, 132.0);
            car.knock_vel *= (1.0 - 3.6 * dt).max(0.0);
        }

        if car.parked {
            car_tf.translation.x = (car_tf.translation.x + car.velocity.x * dt).clamp(-132.0, 132.0);
            car_tf.translation.z = (car_tf.translation.z + car.velocity.y * dt).clamp(-132.0, 132.0);
            if push_car_out_of_pub217(&mut car_tf, car.yaw).is_some() {
                car.velocity = Vec2::ZERO;
                car.knock_vel = Vec2::ZERO;
            }
            car.velocity *= (1.0 - 2.2 * dt).max(0.0);
            car.pitch_lean *= (1.0 - 6.0 * dt).max(0.0);
            car.roll_lean *= (1.0 - 6.0 * dt).max(0.0);
            car_tf.rotation = Quat::from_rotation_y(car.yaw)
                * Quat::from_rotation_x(car.pitch_lean)
                * Quat::from_rotation_z(car.roll_lean);
            continue;
        }

        if car.axis_x {
            car_tf.translation.x += car.dir * car.speed * dt;
            if car_tf.translation.x > CITY_HALF_EXTENT {
                car_tf.translation.x = -CITY_HALF_EXTENT;
            } else if car_tf.translation.x < -CITY_HALF_EXTENT {
                car_tf.translation.x = CITY_HALF_EXTENT;
            }
        } else if let Some(z0) = crate::skatepark::bigpark_uturn_z((car_tf.translation.x / 46.0).round() * 46.0) {
            // Avenue stub absorbed by Five Seasons Park: northbound cars U-turn on a 3.6 m arc
            // north of `z0` into the southbound lane; at the south edge they swap lanes and come
            // back north instead of wrapping into the park.
            let road = (car_tf.translation.x / 46.0).round() * 46.0;
            let r = 3.6;
            let (x, z) = (car_tf.translation.x, car_tf.translation.z);
            if car.dir < 0.0 && z <= z0 {
                let phi = (z0 - z).atan2(x - road) - car.speed * dt / r;
                if phi <= 0.0 {
                    car_tf.translation.x = road + r;
                    car_tf.translation.z = z0;
                    car.dir = 1.0;
                    car.yaw = 0.0;
                } else {
                    car_tf.translation.x = road + r * phi.cos();
                    car_tf.translation.z = z0 - r * phi.sin();
                    car.yaw = phi;
                }
            } else {
                car_tf.translation.z += car.dir * car.speed * dt;
                if car.dir > 0.0 && car_tf.translation.z > CITY_HALF_EXTENT {
                    car_tf.translation.z = CITY_HALF_EXTENT;
                    car_tf.translation.x = road - r;
                    car.dir = -1.0;
                    car.yaw = std::f32::consts::PI;
                }
            }
        } else {
            car_tf.translation.z += car.dir * car.speed * dt;
            if car_tf.translation.z > CITY_HALF_EXTENT {
                car_tf.translation.z = -CITY_HALF_EXTENT;
            } else if car_tf.translation.z < -CITY_HALF_EXTENT {
                car_tf.translation.z = CITY_HALF_EXTENT;
            }
        }
        // Backstop for knockback shoving a lane car sideways into Pub 217.
        if push_car_out_of_pub217(&mut car_tf, car.yaw).is_some() {
            car.knock_vel = Vec2::ZERO;
        }
        car_tf.rotation = Quat::from_rotation_y(car.yaw);

        if is_driving {
            continue;
        }

        // Check car impact with Stan (flying above street height clears cars cleanly!)
        let touching = if stats.riding_scooter.is_some() {
            // On the scooter the car's real body counts: wheels above the hood/roof (a hop, or
            // riding on top) pass cleanly; only running into the body hits. The rider sits
            // 0.25 m above the deck origin (`update_scooter_stunts`).
            let wheels = p_tf.translation - Vec3::Y * 0.25;
            city_car_contact(&car_tf, &car, wheels, 0.45) == CarContact::Inside
        } else {
            let vertical_gap = (p_tf.translation.y - car_tf.translation.y).abs();
            let flat_dist = Vec2::new(
                p_tf.translation.x - car_tf.translation.x,
                p_tf.translation.z - car_tf.translation.z,
            )
            .length();
            flat_dist < hit_radius && vertical_gap < (1.6 + stats.player_scale * 0.35)
        };
        if in_active_play
            && touching
            && player.roll_timer <= 0.0
            && player.invuln_timer <= 0.0
        {
            if car.horn_cooldown <= 0.0 {
                play_sfx("car_horn");
                car.horn_cooldown = 1.4;
            }
            let knock = (p_tf.translation - car_tf.translation)
                .with_y(0.0)
                .normalize_or_zero();
            // Saber guard stops the car at the blade; a perfect block bats it away.
            let outcome =
                crate::player::try_block(&mut player, &mut stats, p_tf.translation, car_tf.translation);
            if outcome.blocked() {
                let push = 6.0 + if outcome == crate::player::BlockOutcome::Perfect { 14.0 } else { 0.0 };
                car.knock_vel -= Vec2::new(knock.x, knock.z) * push;
                continue;
            }
            player.velocity.y = 4.5;
            player.invuln_timer = 1.1;
            // Same 2.2-unit shove as before, delivered as a decaying knockback impulse.
            let hurt = !stats.has_ranch_shield;
            crate::player::hit_reaction(&mut player, &mut stats, knock, 2.2, hurt);

            if stats.has_ranch_shield {
                stats.has_ranch_shield = false;
                stats.hint_text = "Ranch Shield blocked a speeding NYC Taxi!".to_string();
            } else {
                stats.hearts -= 1;
                play_sfx("hit");
                stats.hint_text =
                    "HIT BY NYC TRAFFIC! Jump or [SHIFT / ROLL] through cars!".to_string();
                if stats.hearts <= 0 {
                    play_sfx("gameover");
                    next_state.set(GameState::GameOver);
                    return;
                }
            }
        }
    }
}

pub fn update_city_npcs(
    time: Res<Time>,
    mut npc_q: Query<(&mut Transform, &mut CityNpc, &Children)>,
    mut limb_q: Query<(&mut Transform, &CityNpcLimb), Without<CityNpc>>,
) {
    let dt = time.delta_secs().min(0.05);
    for (mut tf, mut npc, children) in &mut npc_q {
        npc.angle += npc.speed * 0.16 * dt;
        npc.walk_phase += npc.speed * dt * 3.8;
        let nx = npc.home_x + npc.angle.cos() * 2.2;
        let nz = npc.home_z + npc.angle.sin() * npc.patrol_radius * 0.45;
        let dx = nx - tf.translation.x;
        let dz = nz - tf.translation.z;
        tf.translation.x = nx;
        tf.translation.z = nz;
        if dx * dx + dz * dz > 1e-5 {
            let yaw = dx.atan2(dz);
            tf.rotation = Quat::from_rotation_y(yaw);
        }
        let swing = npc.walk_phase.sin() * 0.55;
        for &ch in children {
            if let Ok((mut l_tf, limb)) = limb_q.get_mut(ch) {
                let s = if limb.is_arm { -swing * limb.side } else { swing * limb.side };
                l_tf.rotation = Quat::from_rotation_x(s);
            }
        }
    }
}

pub fn update_city_pickups(
    mut commands: Commands,
    time: Res<Time>,
    mut stats: ResMut<PlayerStats>,
    player_q: Query<&Transform, With<Player>>,
    mut tendy_q: Query<(Entity, &mut Transform, &TendyPickup), (With<CityTendy>, Without<Player>)>,
    mut lock_hint_cd: Local<f32>,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    let Ok(p_tf) = player_q.get_single() else {
        return;
    };
    let pickup_r = 2.1 + stats.player_scale * 0.45;
    let mid_y = 0.55 * stats.player_scale;

    if stats.free_roam {
        // Post-game free roam has no tendies at all: clear any the city spawned with (a fresh
        // free-roam entry, or the world carried over from the run that beat Lexie).
        for (entity, _, _) in &tendy_q {
            commands.entity(entity).despawn_recursive();
        }
        return;
    }

    let unlocked = stats.city_tendies_unlocked();
    *lock_hint_cd = (*lock_hint_cd - dt).max(0.0);
    for (entity, mut tf, tendy) in &mut tendy_q {
        // Locked tendies hang still and dim; lit ones spin and bob.
        if unlocked {
            tf.rotate_y(2.4 * dt);
            tf.translation.y = tendy.base_y + (t * 3.2 + tendy.phase).sin() * 0.22;
        }

        let d = (p_tf.translation + Vec3::Y * mid_y).distance(tf.translation);
        if d < pickup_r && !unlocked {
            if *lock_hint_cd <= 0.0 {
                *lock_hint_cd = 3.5;
                stats.hint_text = format!(
                    "TENDY LOCKED ({}/5)! First: {}.",
                    stats.gate_steps_done(),
                    stats.missing_gate_steps().join(", ")
                );
                stats.hint_bump = stats.hint_bump.wrapping_add(1);
                play_sfx("hack_deny");
            }
            continue;
        }
        if d < pickup_r {
            commands.entity(entity).remove::<(TendyPickup, CityTendy)>();
            crate::player::start_pickup_pop(&mut commands, entity, tf.scale);
            stats.tendies = (stats.tendies + 2).min(stats.total_tendies);
            stats.tendy_credits += 2;
            stats.add_tendy_wendies(5);
            stats.score += 400;
            crate::audio::mark_mom_defeated();
            crate::ui::save_game_checkpoint("city", &stats);
            play_sfx("tendy");
            if stats.tendies >= stats.total_tendies {
                stats.score += 5000;
                stats.hint_text =
                    "50/50 TENDIES - FULL SKYSCRAPER KAIJU! Slash Lexie [CLICK / SLASH]!"
                        .to_string();
                speak_voice(
                    "Maximum skyscraper power reached! Finish Lexie now, Stan!",
                    "narrator",
                );
            } else {
                stats.hint_text = format!(
                    "Tendy {}/{}! Size: {} ({}cm)",
                    stats.tendies,
                    stats.total_tendies,
                    stats.size_tier(),
                    stats.height_cm()
                );
            }
        }
    }
}

/// Hearts Lexie's garlic can't take Stan below while the gate is still closed (half of the
/// city's 20).
const LEXIE_PREGATE_HEART_FLOOR: i32 = 10;

/// One Lexie hit (garlic clove or stench tick). Full damage (1 heart) once the gate is open;
/// before that the fight hasn't begun, so a hit still costs 1 heart but never takes Stan below
/// `LEXIE_PREGATE_HEART_FLOOR` (or below what he already has), so exploring can't kill him.
fn lexie_damage(stats: &mut PlayerStats) {
    // `city_tendies_unlocked` also covers the `city_titan=1` preview (50 tendies, no crew).
    if stats.city_tendies_unlocked() {
        stats.hearts -= 1;
    } else if stats.hearts > LEXIE_PREGATE_HEART_FLOOR {
        stats.hearts -= 1;
    }
}

pub fn update_city_lexie(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
    mut stats: ResMut<PlayerStats>,
    mut next_state: crate::transition::SceneTransition,
    mut player_q: Query<(&mut Transform, &mut Player)>,
    mut lexie_q: Query<
        (Entity, &mut Transform, &mut LexieBoss, &Children),
        (Without<Player>, Without<LexieThrowArm>),
    >,
    mut arm_q: Query<
        &mut Transform,
        (With<LexieThrowArm>, Without<Player>, Without<LexieBoss>),
    >,
    mut garlic_q: Query<
        (Entity, &mut Transform, &mut GarlicProjectile),
        (Without<Player>, Without<LexieBoss>, Without<LexieThrowArm>),
    >,
    mut stench_q: Query<
        (Entity, &mut Transform, &mut GarlicStenchCloud),
        (
            Without<Player>,
            Without<LexieBoss>,
            Without<LexieThrowArm>,
            Without<GarlicProjectile>,
        ),
    >,
    mut debris_q: Query<
        (Entity, &mut Transform, &mut CityTowerDebris),
        (
            Without<Player>,
            Without<LexieBoss>,
            Without<LexieThrowArm>,
            Without<GarlicProjectile>,
            Without<GarlicStenchCloud>,
        ),
    >,
    mut hp_fill_q: Query<&mut Node, With<LexieBossHpFill>>,
    mut label_q: Query<&mut Text, With<LexieBossHudText>>,
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();
    let Ok((p_tf, mut player)) = player_q.get_single_mut() else {
        return;
    };

    // Post-game free roam: Lexie is gone for good. Clear her (spawned with the city world) and
    // anything she threw; nothing below (garlic hits, GameOver, the victory cutscene) can run.
    if stats.free_roam {
        for (e, ..) in &lexie_q {
            commands.entity(e).despawn_recursive();
        }
        for (e, ..) in &garlic_q {
            commands.entity(e).despawn_recursive();
        }
        for (e, ..) in &stench_q {
            commands.entity(e).despawn_recursive();
        }
        for (e, ..) in &debris_q {
            commands.entity(e).despawn_recursive();
        }
        return;
    }

    if let Ok((lexie_ent, mut lx_tf, mut lexie, lx_children)) = lexie_q.get_single_mut() {
        // Tower demolition chunks falling off Empire Citadel while Lexie rampages
        lexie.debris_timer -= dt;
        if lexie.debris_timer <= 0.0 && lexie.on_tower {
            lexie.debris_timer = 0.55;
            let chunk_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(0.62, 0.58, 0.54),
                perceptual_roughness: 0.85,
                ..default()
            });
            commands.spawn((
                LevelEntity,
                CityWorldMarker,
                CityTowerDebris {
                    velocity: Vec3::new(
                        (t * 7.3).sin() * 9.0,
                        4.0 + (t * 4.1).cos().abs() * 5.0,
                        4.0 + (t * 5.7).cos() * 6.0,
                    ),
                    spin: Vec3::new(3.2, 2.4, 4.1),
                    age: 0.0,
                },
                Mesh3d(meshes.add(Cuboid::new(1.5, 1.3, 1.4))),
                MeshMaterial3d(chunk_mat),
                Transform::from_xyz(
                    (t * 3.7).sin() * 5.0,
                    44.0 + (t * 2.1).cos() * 4.0,
                    CITADEL_POS.z + 5.5,
                ),
            ));
        }

        // Transition Lexie down to 3rd St SE Brawl once Stan grows (>= 20 tendies)
        if lexie.on_tower && (stats.tendies >= 20 || p_tf.translation.z < -42.0 && stats.tendies >= 14) {
            lexie.on_tower = false;
            lx_tf.translation = Vec3::new(0.0, 0.0, -44.0);
            play_sfx("thunder");
            speak_voice(
                "Hey oversized tendy boy! Eat fresh pickled garlic cloves up close!",
                "lexie",
            );
            stats.hint_text =
                "LEXIE LEAPT OFF THE TOWER! Collect all 50 Tendies & SLASH her!".to_string();
        }

        // Face toward Stan
        let to_p = p_tf.translation - lx_tf.translation;
        let flat = Vec3::new(to_p.x, 0.0, to_p.z);
        let dist = flat.length();
        if flat.length_squared() > 0.1 {
            let yaw = flat.x.atan2(flat.z);
            lx_tf.rotation = Quat::from_rotation_y(yaw);
        }

        // She waits while the hero rides the Ferris wheel (he is out of reach up there).
        if !lexie.on_tower && dist > 16.0 && !stats.riding_wheel {
            let dir = flat.normalize_or_zero();
            let next_pos = lx_tf.translation + dir * 7.5 * dt;
            if is_clear_of_skyscrapers(next_pos.x, next_pos.z, 3.5) {
                lx_tf.translation = next_pos;
            }
            lx_tf.translation.y = (t * 5.5).sin().abs() * 0.45;
        }

        // Pickled garlic chew & throw animation
        lexie.throw_cooldown -= dt;
        lexie.roar_cooldown -= dt;
        if lexie.hit_flash > 0.0 {
            lexie.hit_flash = (lexie.hit_flash - dt).max(0.0);
        }

        if lexie.chew_timer > 0.0 {
            lexie.chew_timer = (lexie.chew_timer - dt).max(0.0);
        }
        for &ch in lx_children {
            if let Ok(mut arm_tf) = arm_q.get_mut(ch) {
                let swing = if lexie.chew_timer > 0.0 {
                    -1.75 + (t * 12.0).sin() * 0.25
                } else if lexie.throw_cooldown < 0.45 {
                    -1.25
                } else {
                    0.55 + (t * 4.5).sin() * 0.35
                };
                arm_tf.rotation = Quat::from_rotation_x(swing) * Quat::from_rotation_z(0.28);
            }
        }

        // Throw Pickled Garlic Clove volley toward Stan!
        if lexie.throw_cooldown <= 0.0 && !stats.riding_wheel {
            lexie.throw_cooldown = if lexie.on_tower { 3.6 } else { 2.8 };
            lexie.chew_timer = 0.85;
            play_sfx("garlic_throw");
            let hand_world = lx_tf.translation + Vec3::new(0.0, 20.0, 4.0);
            let target_pos = p_tf.translation + Vec3::Y * (0.5 * stats.player_scale);
            let count = if lexie.on_tower { 2 } else { 3 };
            let clove_mesh = meshes.add(Sphere::new(1.35));
            let clove_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(0.95, 0.94, 0.68),
                emissive: LinearRgba::rgb(0.45, 0.65, 0.10),
                perceptual_roughness: 0.3,
                ..default()
            });
            for k in 0..count {
                let spread = (k as f32 - (count as f32 - 1.0) * 0.5) * 9.0;
                let aimed = target_pos + Vec3::new(spread, 0.0, spread * 0.35);
                let flight_t = 1.45_f32;
                let g = 24.0_f32;
                let vx = (aimed.x - hand_world.x) / flight_t;
                let vz = (aimed.z - hand_world.z) / flight_t;
                let vy = (aimed.y - hand_world.y + 0.5 * g * flight_t * flight_t) / flight_t;
                commands.spawn((
                    LevelEntity,
                    CityWorldMarker,
                    GarlicProjectile {
                        velocity: Vec3::new(vx, vy, vz),
                        spin: 5.5 + k as f32,
                        age: 0.0,
                        deflected: false,
                    },
                    Mesh3d(clove_mesh.clone()),
                    MeshMaterial3d(clove_mat.clone()),
                    Transform::from_translation(hand_world).with_scale(Vec3::new(0.85, 1.35, 0.85)),
                ));
            }
        }

        // Take Stan's Skyscraper Lightsaber Slash — or enter the Turn-Based 5-Hero RPG Showdown
        // once all 50 tendies are collected!
        let lexie_contact = crate::player::saber_contact(
            &player,
            &stats,
            p_tf.translation,
            &crate::player::SaberTarget::lexie(&lx_tf),
        );
        if !stats.free_roam
            && stats.tendies >= 50
            && !stats.lexie_defeated
            && (!lexie.on_tower && dist < 26.0 || lexie_contact.is_some())
        {
            let players = crate::audio::net_squad_info()[0].clamp(1, 5);
            crate::audio::net_event(crate::net::EVENT_LEXIE_BATTLE, players);
            commands.entity(lexie_ent).despawn_recursive();
            next_state.set(GameState::LexieBattle);
            return;
        }
        if let (Some(contact), true) = (lexie_contact, lexie.last_saber_swing != player.swing_id) {
            lexie.last_saber_swing = player.swing_id;
            lexie.hit_flash = 0.32;
            crate::player::register_saber_hit(&mut player, &mut stats, contact);
            play_sfx("saber_hit");
            let heavy_mul = if stats.heavy_attack { 1.85 } else { 1.0 };
            if stats.tendies < 50 {
                // Chip damage capped until all 50 tendies give Skyscraper Titan Power
                lexie.hp = (lexie.hp - 18.0 * heavy_mul).max(75.0);
                stats.hint_text = format!(
                    "Collect all 50 TENDIES ({}/50) for Full Skyscraper Titan Power to finish Lexie!",
                    stats.tendies
                );
            } else {
                lexie.hp -= 85.0;
                stats.score += 1200;
            }
            stats.boss_hp = lexie.hp.max(0.0);
        }

        // Defeat check runs every frame so a finishing car ram (update_city_driving) counts too.
        if lexie.hp <= 0.0 && !stats.lexie_defeated {
            stats.score += 15000;
            stats.add_tendy_wendies(100);
            stats.lexie_defeated = true;
            // Unlock and persist post-game Free Mode immediately on defeat so reloading during
            // the victory cutscene never loses the unlock.
            stats.game_beaten = true;
            crate::ui::save_game_checkpoint("city", &stats);
            stats.hint_text = "LEXIE DEFEATED! NEW YORK CITY SAVED!".to_string();
            play_sfx("win");
            speak_voice(
                "Oh no! My pickled garlic empire! You win, giant Viking Stan!",
                "lexie",
            );
            commands.entity(lexie_ent).despawn_recursive();
            next_state.set(GameState::LexieVictoryCutscene);
            return;
        }

        // Update Top-Center Boss HP UI
        if let Ok(mut fill) = hp_fill_q.get_single_mut() {
            fill.width = Val::Percent((lexie.hp / lexie.max_hp * 100.0).clamp(0.0, 100.0));
        }
        if let Ok(mut lbl) = label_q.get_single_mut() {
            lbl.0 = if stats.tendies < 50 {
                format!(
                    "LEXIE | PICKLED GARLIC QUEEN ({}/50 TENDIES FOR MAX TITAN)",
                    stats.tendies
                )
            } else {
                format!("LEXIE | SKYSCRAPER TITAN SHOWDOWN! HP {:.0}%", lexie.hp / lexie.max_hp * 100.0)
            };
        }
    }

    let in_pub217 = is_in_pub217(p_tf.translation, stats.player_scale);
    let lexie_pos = lexie_q.get_single().ok().map(|(_, tf, _, _)| tf.translation);

    // Update airborne Pickled Garlic Clove Projectiles & burst into lingering Stench Clouds
    for (g_ent, mut g_tf, mut proj) in &mut garlic_q {
        proj.age += dt;
        proj.velocity.y -= 24.0 * dt;
        g_tf.translation += proj.velocity * dt;
        g_tf.rotate_y(proj.spin * dt);
        g_tf.rotate_x(proj.spin * 0.7 * dt);

        if is_in_pub217(g_tf.translation, stats.player_scale) {
            commands.entity(g_ent).despawn_recursive();
            continue;
        }
        if proj.deflected {
            if g_tf.translation.y <= 0.6 || proj.age > 5.5 {
                commands.entity(g_ent).despawn_recursive();
            }
            continue;
        }

        let hit_stan = g_tf.translation.distance(p_tf.translation + Vec3::Y * (0.5 * stats.player_scale))
            < (2.6 + stats.player_scale * 0.25);

        // Saber guard: a blocked clove is sliced apart (no stench cloud on Stan); a perfect block
        // bats it back toward Lexie. Cloves arriving inside the post-block grace are cut for free.
        if !stats.phone_open && !stats.bartender_open && !in_pub217 && hit_stan && player.roll_timer <= 0.0 {
            let outcome = if player.invuln_timer <= 0.0 {
                crate::player::try_block(&mut player, &mut stats, p_tf.translation, g_tf.translation)
            } else if crate::player::guard_covers(&player, &stats, p_tf.translation, g_tf.translation) {
                player.block_spark = player.block_spark.max(1);
                crate::player::BlockOutcome::Blocked
            } else {
                crate::player::BlockOutcome::None
            };
            if outcome == crate::player::BlockOutcome::Perfect {
                let back = lexie_pos
                    .map(|l| Vec3::new(l.x - g_tf.translation.x, 0.0, l.z - g_tf.translation.z))
                    .unwrap_or(-proj.velocity.with_y(0.0))
                    .normalize_or_zero();
                let speed = proj.velocity.with_y(0.0).length().max(12.0) * 1.3;
                proj.velocity = back * speed + Vec3::Y * 14.0;
                proj.deflected = true;
                proj.age = 0.0;
                continue;
            } else if outcome == crate::player::BlockOutcome::Blocked {
                commands.entity(g_ent).despawn_recursive();
                continue;
            }
        }

        if !stats.phone_open
            && !stats.bartender_open
            && !in_pub217
            && hit_stan
            && player.roll_timer <= 0.0
            && player.invuln_timer <= 0.0
        {
            player.invuln_timer = 1.0;
            let hurt = !stats.has_ranch_shield;
            crate::player::hit_reaction(&mut player, &mut stats, Vec3::ZERO, 0.0, hurt);
            if stats.has_ranch_shield {
                stats.has_ranch_shield = false;
            } else {
                lexie_damage(&mut stats);
                play_sfx("hit");
                stats.hint_text = if stats.city_tendies_unlocked() {
                    "DIRECT PICKLED GARLIC HIT! Dodge Roll [SHIFT / ROLL]!".to_string()
                } else {
                    "Garlic splash! Lexie is only warming up until your crew is ready.".to_string()
                };
                if stats.hearts <= 0 {
                    play_sfx("gameover");
                    next_state.set(GameState::GameOver);
                    return;
                }
            }
        }

        if g_tf.translation.y <= 0.6 || hit_stan || proj.age > 5.5 {
            let impact_pos = Vec3::new(g_tf.translation.x, 0.35, g_tf.translation.z);
            commands.entity(g_ent).despawn_recursive();
            let fume_mat = materials.add(StandardMaterial {
                base_color: Color::srgba(0.68, 0.88, 0.16, 0.26),
                emissive: LinearRgba::rgb(0.42, 0.64, 0.06),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            });
            commands.spawn((
                LevelEntity,
                CityWorldMarker,
                GarlicStenchCloud {
                    lifetime: 4.8,
                    max_lifetime: 4.8,
                    radius: 5.8,
                    damage_tick: 0.65,
                },
                Mesh3d(meshes.add(Sphere::new(1.0))),
                MeshMaterial3d(fume_mat),
                Transform::from_translation(impact_pos).with_scale(Vec3::new(3.5, 2.2, 3.5)),
            ));
        }
    }

    // Update lingering Pickled Garlic Stench Clouds & intense smell damage
    for (s_ent, mut s_tf, mut cloud) in &mut stench_q {
        cloud.lifetime -= dt;
        if cloud.lifetime <= 0.0 {
            commands.entity(s_ent).despawn_recursive();
            continue;
        }
        let life_frac = (cloud.lifetime / cloud.max_lifetime).clamp(0.0, 1.0);
        let spread = 1.0 + (1.0 - life_frac) * 1.15;
        s_tf.scale = Vec3::new(
            cloud.radius * spread,
            4.2 * (0.65 + (1.0 - life_frac) * 0.5),
            cloud.radius * spread,
        );
        s_tf.rotate_y(1.2 * dt);

        if cloud.damage_tick > 0.0 {
            cloud.damage_tick = (cloud.damage_tick - dt).max(0.0);
        }
        let flat_dist = Vec2::new(
            p_tf.translation.x - s_tf.translation.x,
            p_tf.translation.z - s_tf.translation.z,
        )
        .length();
        if !stats.phone_open
            && !stats.bartender_open
            && !in_pub217
            && flat_dist < cloud.radius * spread
            && p_tf.translation.y < 6.5
            && player.roll_timer <= 0.0
            && player.invuln_timer <= 0.0
            && cloud.damage_tick <= 0.0
        {
            cloud.damage_tick = 1.45;
            player.invuln_timer = 0.85;
            let hurt = !stats.has_ranch_shield;
            crate::player::hit_reaction(&mut player, &mut stats, Vec3::ZERO, 0.0, hurt);
            if stats.has_ranch_shield {
                stats.has_ranch_shield = false;
            } else {
                lexie_damage(&mut stats);
                play_sfx("hit");
                stats.hint_text =
                    "INTENSE PICKLED GARLIC SMELL! Dodge Roll out of green fumes!".to_string();
                if stats.hearts <= 0 {
                    play_sfx("gameover");
                    next_state.set(GameState::GameOver);
                    return;
                }
            }
        }
    }

    // Animate & clean up Empire Citadel demolition debris chunks
    for (d_ent, mut d_tf, mut deb) in &mut debris_q {
        deb.age += dt;
        deb.velocity.y -= 24.0 * dt;
        d_tf.translation += deb.velocity * dt;
        d_tf.rotate_x(deb.spin.x * dt);
        d_tf.rotate_y(deb.spin.y * dt);
        if d_tf.translation.y <= 0.4 || deb.age > 5.0 {
            commands.entity(d_ent).despawn_recursive();
        }
    }
}

pub fn update_city_minimap(
    time: Res<Time>,
    stats: Res<PlayerStats>,
    crew_spots: Option<Res<CrewSpots>>,
    player_q: Query<&Transform, With<Player>>,
    lexie_q: Query<&Transform, (With<LexieBoss>, Without<Player>)>,
    tendy_q: Query<&Transform, (With<CityTendy>, Without<Player>, Without<LexieBoss>)>,
    license_q: Query<&Transform, (With<LicenseCard>, Without<Player>, Without<LexieBoss>, Without<CityTendy>)>,
    mut p_dot_q: Query<&mut Node, With<CityMinimapPlayerDot>>,
    mut b_dot_q: Query<(&mut Node, &mut Visibility), (With<CityMinimapBossDot>, Without<CityMinimapPlayerDot>)>,
    mut t_dot_q: Query<
        (&mut Node, &mut Visibility, &mut BackgroundColor),
        (
            With<CityMinimapTendyDot>,
            Without<CityMinimapPlayerDot>,
            Without<CityMinimapBossDot>,
        ),
    >,
    mut c_dot_q: Query<
        (&mut Node, &mut Visibility, &mut BorderColor, &CityMinimapCrewDot),
        (
            Without<CityMinimapTendyDot>,
            Without<CityMinimapPlayerDot>,
            Without<CityMinimapBossDot>,
        ),
    >,
) {
    let to_pct = |v: f32| -> f32 { ((v + CITY_HALF_EXTENT) / (CITY_HALF_EXTENT * 2.0) * 100.0).clamp(3.0, 97.0) };

    if let (Ok(p_tf), Ok(mut p_node)) = (player_q.get_single(), p_dot_q.get_single_mut()) {
        p_node.left = Val::Percent(to_pct(p_tf.translation.x));
        p_node.top = Val::Percent(to_pct(p_tf.translation.z));
    }

    if let Ok((mut b_node, mut b_vis)) = b_dot_q.get_single_mut() {
        if let Ok(lx_tf) = lexie_q.get_single() {
            *b_vis = Visibility::Visible;
            b_node.left = Val::Percent(to_pct(lx_tf.translation.x));
            b_node.top = Val::Percent(to_pct(lx_tf.translation.z));
        } else {
            *b_vis = Visibility::Hidden;
        }
    }

    // Locked tendies show as faint grey dots so the radar points at the crew first.
    let unlocked = stats.city_tendies_unlocked();
    let tendy_color = if unlocked {
        Color::srgb(1.0, 0.84, 0.14)
    } else {
        Color::srgba(0.62, 0.62, 0.66, 0.45)
    };
    let mut tendies_iter = tendy_q.iter();
    for (mut t_node, mut t_vis, mut t_bg) in &mut t_dot_q {
        if let Some(t_tf) = tendies_iter.next() {
            *t_vis = Visibility::Visible;
            t_node.left = Val::Percent(to_pct(t_tf.translation.x));
            t_node.top = Val::Percent(to_pct(t_tf.translation.z));
            if t_bg.0 != tendy_color {
                t_bg.0 = tendy_color;
            }
        } else {
            *t_vis = Visibility::Hidden;
        }
    }

    // Missing-crew markers pulse so they read as the current goal.
    let pulse = 0.55 + 0.45 * (time.elapsed_secs() * 4.0).sin().abs();
    for (mut c_node, mut c_vis, mut c_border, dot) in &mut c_dot_q {
        if dot.0 == 5 {
            let card = license_q.iter().next().filter(|_| stats.license_refused && !stats.has_license);
            let want = if card.is_some() { Visibility::Visible } else { Visibility::Hidden };
            if *c_vis != want {
                *c_vis = want;
            }
            if let Some(t) = card {
                c_node.left = Val::Percent(to_pct(t.translation.x));
                c_node.top = Val::Percent(to_pct(t.translation.z));
            }
            c_border.0 = Color::srgba(0.35, 0.85, 1.0, pulse);
            continue;
        }
        // Crew only show once they have called out (the search is the point); the pub marker
        // stays up for Mike until the smoke break.
        let i = usize::from(dot.0);
        let (pos, missing, color) = if i < 4 {
            match crew_spots.as_deref() {
                Some(c) => (
                    c.spots[i].0,
                    c.revealed[i] && !crew_recruited(&stats, i),
                    Color::srgba(0.35, 1.0, 0.55, pulse),
                ),
                None => (Vec3::ZERO, false, Color::NONE),
            }
        } else {
            (
                PUB217_ANCHOR + Vec3::new(-6.5, 0.0, 0.0) * PUB217_SCALE,
                !stats.mike_smoked,
                Color::srgba(1.0, 0.78, 0.25, pulse),
            )
        };
        let want = if missing && !unlocked && !stats.free_roam { Visibility::Visible } else { Visibility::Hidden };
        if *c_vis != want {
            *c_vis = want;
        }
        c_node.left = Val::Percent(to_pct(pos.x));
        c_node.top = Val::Percent(to_pct(pos.z));
        c_border.0 = color;
    }
}

/// Crew gate: city tendies stay dimmed (no glow, no sky beacon) until TJ, Derek, Nate and
/// Glenn have all joined and Stan has smoked with Mike; the last step triggers a one-time
/// "gate open" moment.
pub fn update_city_crew_gate(
    mut stats: ResMut<PlayerStats>,
    look: Option<ResMut<CityTendyLook>>,
    mut seal_q: Query<&mut Visibility, (With<CityTendySeal>, Without<CityTendyBeacon>)>,
    mut beacon_q: Query<&mut Visibility, (With<CityTendyBeacon>, Without<CityTendySeal>)>,
) {
    if stats.gate_open() && !stats.crew_unlock_announced {
        stats.crew_unlock_announced = true;
        if stats.tendies < stats.total_tendies {
            stats.hint_text =
                "CREW ASSEMBLED & MIKE'S SMOKE BREAK DONE! The tendies are lit: eat 50, grow into a kaiju, take down Lexie!"
                    .to_string();
            stats.hint_bump = stats.hint_bump.wrapping_add(1);
            play_sfx("powerup");
            speak_voice("The crew is assembled! Go eat those tendies, Stan!", "narrator");
        }
    }
    let Some(mut look) = look else {
        return;
    };
    let lit = stats.city_tendies_unlocked();
    if look.lit == Some(lit) {
        return;
    }
    look.lit = Some(lit);
    // Locked: each tendy sits in a sealed grey bubble with its beacon off (and the spin stops,
    // see `update_city_pickups`). Unlocked: bubbles vanish and the beacons come on.
    let (seal, beacon) = if lit {
        (Visibility::Hidden, Visibility::Inherited)
    } else {
        (Visibility::Inherited, Visibility::Hidden)
    };
    for mut v in &mut seal_q {
        *v = seal;
    }
    for mut v in &mut beacon_q {
        *v = beacon;
    }
}

/// Pub 217 has a fixed size, so once Stan outgrows its door the doorway is plugged like the
/// rest of the building's walls (he can still stand on the roof). Stan never grows inside the
/// pub (no tendies spawn there), so the plug can't trap him.
pub fn update_pub217_door(
    mut stats: ResMut<PlayerStats>,
    player_q: Query<&Transform, (With<Player>, Without<Pub217DoorPlug>)>,
    mut plug_q: Query<(&mut Transform, &mut PlatformBox), With<Pub217DoorPlug>>,
    mut warned: Local<bool>,
) {
    let too_big = stats.player_scale > PUB217_DOOR_MAX_SCALE;
    for (mut tf, mut pbox) in &mut plug_q {
        if too_big {
            tf.translation = PUB217_ANCHOR + Vec3::new(0.0, 3.2, 0.0) * PUB217_SCALE;
            pbox.half_extents = Vec3::new(0.25, 3.2, 2.3) * PUB217_SCALE;
        } else {
            tf.translation = PUB217_ANCHOR + Vec3::new(0.0, -50.0, 0.0);
            pbox.half_extents = Vec3::new(0.25, 0.1, 0.1);
        }
    }
    // One short hint per approach when a grown Stan walks up to the door.
    let Ok(p_tf) = player_q.get_single() else {
        return;
    };
    let near_door = Vec2::new(p_tf.translation.x - PUB217_ANCHOR.x, p_tf.translation.z - PUB217_ANCHOR.z)
        .length()
        < 4.0 + stats.player_scale * 0.6;
    if too_big && near_door && p_tf.translation.y < 3.0 {
        if !*warned {
            *warned = true;
            stats.hint_text = "You've outgrown Pub 217! The crew is with you: go eat tendies.".to_string();
            stats.hint_bump = stats.hint_bump.wrapping_add(1);
        }
    } else if !near_door {
        *warned = false;
    }
}

pub fn animate_city_intro_lexie(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
    mut lexie_q: Query<(&mut LexieBoss, &Children)>,
    mut arm_q: Query<&mut Transform, (With<LexieThrowArm>, Without<CityTowerDebris>)>,
    mut debris_q: Query<(Entity, &mut Transform, &mut CityTowerDebris), Without<LexieThrowArm>>,
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();

    if let Ok((mut lexie, children)) = lexie_q.get_single_mut() {
        for &ch in children {
            if let Ok(mut arm_tf) = arm_q.get_mut(ch) {
                let swing = 0.35 + (t * 6.0).sin() * 0.85;
                arm_tf.rotation = Quat::from_rotation_x(swing) * Quat::from_rotation_z(0.28);
            }
        }
        lexie.debris_timer -= dt;
        if lexie.debris_timer <= 0.0 {
            lexie.debris_timer = 0.42;
            let chunk_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(0.64, 0.60, 0.56),
                perceptual_roughness: 0.85,
                ..default()
            });
            commands.spawn((
                LevelEntity,
                CityWorldMarker,
                CityTowerDebris {
                    velocity: Vec3::new(
                        (t * 7.3).sin() * 10.0,
                        4.5 + (t * 4.1).cos().abs() * 6.0,
                        5.0 + (t * 5.7).cos() * 6.0,
                    ),
                    spin: Vec3::new(3.2, 2.4, 4.1),
                    age: 0.0,
                },
                Mesh3d(meshes.add(Cuboid::new(1.8, 1.5, 1.6))),
                MeshMaterial3d(chunk_mat),
                Transform::from_xyz(
                    (t * 3.7).sin() * 5.0,
                    46.0 + (t * 2.1).cos() * 4.0,
                    CITADEL_POS.z + 5.5,
                ),
            ));
        }
    }

    for (d_ent, mut d_tf, mut deb) in &mut debris_q {
        deb.age += dt;
        deb.velocity.y -= 24.0 * dt;
        d_tf.translation += deb.velocity * dt;
        d_tf.rotate_x(deb.spin.x * dt);
        d_tf.rotate_y(deb.spin.y * dt);
        if d_tf.translation.y <= 0.4 || deb.age > 5.0 {
            commands.entity(d_ent).despawn_recursive();
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Street tendy count, the Pub 217 beer tower and the parked "no license" car
// ---------------------------------------------------------------------------------------------

/// Tendies on the city map in total, stunt trails included (50 of them grow Stan to full size).
const CITY_TENDY_TOTAL: usize = 70;

/// Drops street tendies until `keep` remain, each time removing the one whose nearest neighbour
/// (street or `fixed`) is closest, so crowded stretches thin out first and every district keeps
/// some. Deterministic, so the map is the same every run.
fn thin_street_tendies(spots: &mut Vec<Vec3>, fixed: &[Vec3], keep: usize) {
    let flat_d2 = |a: Vec3, b: Vec3| (a.x - b.x).powi(2) + (a.z - b.z).powi(2);
    while spots.len() > keep {
        let mut worst = (0_usize, f32::MAX);
        for (i, &p) in spots.iter().enumerate() {
            let nearest = spots
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, &q)| flat_d2(p, q))
                .chain(fixed.iter().map(|&q| flat_d2(p, q)))
                .fold(f32::MAX, f32::min);
            if nearest < worst.1 {
                worst = (i, nearest);
            }
        }
        spots.remove(worst.0);
    }
}

/// Floor ring around the beer-tower table, shown while Stan can drink there.
#[derive(Component)]
pub struct BeerTowerGlow;

/// The one parked car: a normal `CityCar` spawned with `parked: true`, so traffic leaves it
/// where it is (see the parked branch of `update_city_traffic`) and `update_city_driving` can
/// take it over once Stan has the license. Left anywhere, it parks again like any taken car.
#[derive(Component)]
pub struct ParkedCar;

/// Solid box that follows the parked car while nobody is driving it (`sync_parked_car_collider`).
/// `armed` is false right after Stan leaves it, so the box only switches back on once he has
/// stepped clear and never pops him out.
#[derive(Component)]
pub struct ParkedCarCollider {
    car: Entity,
    armed: bool,
}

const PARKED_COLLIDER_OFF: Vec3 = Vec3::new(0.0, -60.0, 0.0);

/// Shared on-screen button for the beer tower and the parked car (tappable on touch).
#[derive(Component)]
pub struct PropPromptUi;

#[derive(Component)]
pub struct PropPromptButton;

#[derive(Component)]
pub struct PropPromptText;

/// The beer-tower table, in the same pre-scale pub coordinates the pub builder uses.
const BEER_TOWER_PUB_POS: Vec3 = Vec3::new(-22.6, 0.0, 38.6);
const BEER_HEARTS: i32 = 3;
const BEER_DRUNK_SECS: f32 = 20.0;
const BEER_DRUNK_CAP_SECS: f32 = 45.0;
const BEER_COOLDOWN_SECS: f32 = 4.0;
/// Reach from the table centre (the table is about 1.6 m half-width at pub scale).
const BEER_REACH: f32 = 3.4;
/// Stan only fits at the bar around normal size (180 cm is scale 1.8).
const BEER_MAX_SCALE: f32 = 2.6;
const PARKED_CAR_REACH: f32 = 4.0;

fn beer_tower_world_pos() -> Vec3 {
    PUB217_ANCHOR + (BEER_TOWER_PUB_POS - PUB217_LAYOUT_ORIGIN) * PUB217_SCALE
}

/// Spawns one parked car and the driver's license card at two different random kerbside spots:
/// mid-block, outside the traffic lanes (lanes are 3.6 m off the road centre, these sit at 7 m),
/// clear of buildings, Pub 217 and its front, Greene Square, the stunt trails, the tendies and
/// the scattered crew.
/// The car is solid via a `PlatformBox`; the card is a street-level walk-in pickup.
fn spawn_parked_car(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    tendy_spots: &[Vec3],
    crew_spots: &[Vec3],
) {
    let stunt = crate::stunts::stunt_tendy_spots();
    let flat_d2 = |a: Vec3, b: Vec3| (a.x - b.x).powi(2) + (a.z - b.z).powi(2);
    let pub_front = PUB217_ANCHOR + Vec3::new(4.0, 0.0, 0.0);
    let mut candidates: Vec<(Vec3, bool)> = Vec::new();
    for &r in &ROAD_COORDS {
        // 3rd St SE (x = 0) has the bike lanes, trees and the north stunt circuit.
        if r == 0.0 {
            continue;
        }
        for &b in &BLOCK_COORDS {
            for side in [-7.0_f32, 7.0] {
                // (position, runs along z)
                for (p, along_z) in [(Vec3::new(r + side, 0.0, b), true), (Vec3::new(b, 0.0, r + side), false)] {
                    let ok = is_clear_of_skyscrapers(p.x, p.z, 2.5)
                        && !is_in_pub217(p, 1.8)
                        && flat_d2(p, pub_front) > 26.0 * 26.0
                        && flat_d2(p, Vec3::new(69.0, 0.0, 69.0)) > 32.0 * 32.0
                        && stunt.iter().all(|&q| flat_d2(p, q) > 12.0 * 12.0)
                        && tendy_spots.iter().all(|&q| flat_d2(p, q) > 5.0 * 5.0)
                        && crew_spots.iter().all(|&q| flat_d2(p, q) > 9.0 * 9.0);
                    if ok {
                        candidates.push((p, along_z));
                    }
                }
            }
        }
    }
    let mut seed = [0u8; 8];
    let _ = getrandom::getrandom(&mut seed);
    let seed = u64::from_le_bytes(seed);
    let n = candidates.len();
    if n < 2 {
        return;
    }
    let car_i = (seed % n as u64) as usize;
    let card_i = (car_i + 1 + ((seed >> 32) % (n as u64 - 1)) as usize) % n;
    spawn_license_card(commands, meshes, materials, candidates[card_i].0);
    let (pos, along_z) = candidates[car_i];
    let yaw = if along_z { 0.0 } else { std::f32::consts::FRAC_PI_2 };

    let paint = materials.add(StandardMaterial {
        base_color: Color::srgb(0.30, 0.10, 0.42),
        perceptual_roughness: 0.22,
        metallic: 0.55,
        ..default()
    });
    let glass = materials.add(StandardMaterial {
        base_color: Color::srgb(0.14, 0.22, 0.32),
        metallic: 0.6,
        perceptual_roughness: 0.15,
        ..default()
    });
    let tire = materials.add(StandardMaterial {
        base_color: Color::srgb(0.09, 0.09, 0.11),
        perceptual_roughness: 0.85,
        ..default()
    });
    let trim = materials.add(StandardMaterial {
        base_color: Color::srgb(0.12, 0.12, 0.14),
        perceptual_roughness: 0.55,
        metallic: 0.2,
        ..default()
    });
    let hub = materials.add(StandardMaterial {
        base_color: Color::srgb(0.72, 0.74, 0.78),
        perceptual_roughness: 0.3,
        metallic: 0.8,
        ..default()
    });
    // Lights off: it is parked.
    let lamp_off = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.52, 0.45),
        perceptual_roughness: 0.3,
        ..default()
    });
    let cm = car_meshes(meshes);
    let car_e = commands
        .spawn((
            LevelEntity,
            CityWorldMarker,
            ParkedCar,
            CityCar {
                axis_x: !along_z,
                dir: 1.0,
                speed: 15.0,
                horn_cooldown: 0.0,
                occupied: false,
                parked: true,
                yaw,
                velocity: Vec2::ZERO,
                pitch_lean: 0.0,
                roll_lean: 0.0,
                steer_input: 0.0,
                knock_vel: Vec2::ZERO,
                drift_sfx_cooldown: 0.0,
                car_hp: 100.0,
                roof_h: CAR_ROOF_TOP,
                ride: CarRide::default(),
            },
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
        ))
        .with_children(|car| {
            car.spawn((Mesh3d(cm.body.clone()), MeshMaterial3d(paint.clone()), Transform::from_xyz(0.0, 0.72, 0.0)));
            car.spawn((Mesh3d(cm.cabin.clone()), MeshMaterial3d(glass), Transform::from_xyz(0.0, 1.38, -0.15)));
            car.spawn((Mesh3d(cm.roof.clone()), MeshMaterial3d(paint), Transform::from_xyz(0.0, 1.665, -0.26)));
            car.spawn((Mesh3d(cm.trim.clone()), MeshMaterial3d(trim), Transform::default()));
            car.spawn((Mesh3d(cm.headlights.clone()), MeshMaterial3d(lamp_off.clone()), Transform::default()));
            car.spawn((Mesh3d(cm.taillights.clone()), MeshMaterial3d(lamp_off), Transform::default()));
            for (wx, wz) in [(-1.05_f32, 1.32_f32), (1.05, 1.32), (-1.05, -1.32), (1.05, -1.32)] {
                car.spawn((
                    CityCarWheel { is_front: wz > 0.0 },
                    Mesh3d(cm.tyre.clone()),
                    MeshMaterial3d(tire.clone()),
                    Transform::from_xyz(wx, 0.36, wz)
                        .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2))
                        .with_scale(Vec3::new(0.36, 0.28, 0.36)),
                ))
                .with_children(|w| {
                    w.spawn((Mesh3d(cm.hub.clone()), MeshMaterial3d(hub.clone()), Transform::default()));
                });
            }
        })
        .id();
    // Solid like a building for Stan and the scooters while parked (placed each frame).
    commands.spawn((
        LevelEntity,
        CityWorldMarker,
        ParkedCarCollider { car: car_e, armed: true },
        Transform::from_translation(PARKED_COLLIDER_OFF),
        PlatformBox { half_extents: Vec3::splat(0.1) },
    ));
}

/// Keeps the parked car's `PlatformBox` on it while nobody drives it (an AABB around its yaw,
/// so it also follows wherever Stan leaves it or anything pushes it), and parks the box far
/// below the street while it is driven so the car never hits its own collider. Runs before
/// `update_city_driving`.
pub fn sync_parked_car_collider(
    mut commands: Commands,
    stats: Res<PlayerStats>,
    car_q: Query<(&Transform, &CityCar), With<ParkedCar>>,
    player_q: Query<&Transform, (With<Player>, Without<ParkedCarCollider>)>,
    mut box_q: Query<(Entity, &mut Transform, &mut PlatformBox, &mut ParkedCarCollider), Without<ParkedCar>>,
) {
    let p = player_q.get_single().ok().map(|t| t.translation);
    for (box_e, mut b_tf, mut pbox, mut col) in &mut box_q {
        let Ok((c_tf, car)) = car_q.get(col.car) else {
            // The car is gone (world rebuilt): drop its collider too, never leave a ghost wall.
            commands.entity(box_e).despawn();
            continue;
        };
        let driven = car.occupied || stats.driving_car == Some(col.car);
        if driven {
            col.armed = false;
        } else if !col.armed {
            let clear = p.is_none_or(|p| {
                Vec2::new(p.x - c_tf.translation.x, p.z - c_tf.translation.z).length() > 4.0
            });
            col.armed = clear;
        }
        if driven || !col.armed {
            if b_tf.translation != PARKED_COLLIDER_OFF {
                b_tf.translation = PARKED_COLLIDER_OFF;
                pbox.half_extents = Vec3::splat(0.1);
            }
            continue;
        }
        let (s, c) = car.yaw.sin_cos();
        let (hx, hz) = (1.12_f32, 2.3_f32);
        let half = Vec3::new(c.abs() * hx + s.abs() * hz, 0.95, s.abs() * hx + c.abs() * hz);
        b_tf.translation = Vec3::new(c_tf.translation.x, c_tf.translation.y + 0.95, c_tf.translation.z);
        pbox.half_extents = half;
    }
}

/// The driver's license pickup (`update_license_card`); never respawns once taken (it is only
/// spawned with the city and despawned as soon as `has_license` is set).
#[derive(Component)]
pub struct LicenseCard;

fn spawn_license_card(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
) {
    let card_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.93, 1.0),
        emissive: LinearRgba::rgb(0.35, 0.55, 0.9),
        perceptual_roughness: 0.3,
        ..default()
    });
    let stripe_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.12, 0.30, 0.75),
        emissive: LinearRgba::rgb(0.1, 0.25, 0.8),
        ..default()
    });
    let beacon_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.35, 0.80, 1.0, 0.28),
        emissive: LinearRgba::rgb(0.3, 0.9, 1.5),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let card = meshes.add(Cuboid::new(0.95, 0.62, 0.05));
    let stripe = meshes.add(Cuboid::new(0.97, 0.12, 0.06));
    let photo = meshes.add(Cuboid::new(0.26, 0.32, 0.06));
    let beacon = meshes.add(Cylinder::new(0.22, 10.0));
    commands
        .spawn((
            LevelEntity,
            CityWorldMarker,
            LicenseCard,
            Transform::from_translation(pos + Vec3::Y * 1.1),
            Visibility::default(),
        ))
        .with_children(|c| {
            c.spawn((Mesh3d(card), MeshMaterial3d(card_mat), Transform::default()));
            c.spawn((Mesh3d(stripe), MeshMaterial3d(stripe_mat.clone()), Transform::from_xyz(0.0, 0.2, 0.0)));
            c.spawn((Mesh3d(photo), MeshMaterial3d(stripe_mat), Transform::from_xyz(-0.28, -0.08, 0.0)));
            c.spawn((Mesh3d(beacon), MeshMaterial3d(beacon_mat), Transform::from_xyz(0.0, 4.6, 0.0)));
        });
}

/// Spins and bobs the license card and picks it up when Stan walks into it.
pub fn update_license_card(
    mut commands: Commands,
    time: Res<Time>,
    mut stats: ResMut<PlayerStats>,
    player_q: Query<&Transform, With<Player>>,
    mut card_q: Query<(Entity, &mut Transform), (With<LicenseCard>, Without<Player>)>,
) {
    if stats.has_license {
        for (e, _) in &card_q {
            commands.entity(e).despawn_recursive();
        }
        return;
    }
    let Ok(p_tf) = player_q.get_single() else {
        return;
    };
    let t = time.elapsed_secs();
    for (e, mut c_tf) in &mut card_q {
        c_tf.rotation = Quat::from_rotation_y(t * 2.2);
        c_tf.translation.y = 1.1 + (t * 2.6).sin() * 0.15;
        let d = Vec2::new(c_tf.translation.x - p_tf.translation.x, c_tf.translation.z - p_tf.translation.z).length();
        // Walk-in pickup at street level, at any size up to a big Stan stepping on it.
        if d < 1.6 + stats.player_scale * 0.3 && p_tf.translation.y < 3.0 + stats.player_scale {
            commands.entity(e).despawn_recursive();
            stats.has_license = true;
            play_sfx("powerup");
            stats.hint_text = "Driver's license found! You can drive cars now.".to_string();
            stats.hint_bump = stats.hint_bump.wrapping_add(1);
            speak_voice("Driver's license found! You can drive cars now.", "narrator");
            crate::ui::save_game_checkpoint("city", &stats);
        }
    }
}

/// Refuses a car while Stan has no license (traffic hijack in `update_city_driving` and the
/// parked car). Rate limited by `license_deny_cd`; the first refusal turns on the radar marker.
pub fn refuse_no_license(stats: &mut PlayerStats) {
    if stats.license_deny_cd > 0.0 {
        return;
    }
    stats.license_deny_cd = 2.5;
    stats.license_refused = true;
    play_sfx("hack_deny");
    stats.hint_text =
        "Sorry, you don't have a license. One is hidden somewhere in Cedar Rapids (blue marker on the radar)."
            .to_string();
    stats.hint_bump = stats.hint_bump.wrapping_add(1);
    speak_voice("Sorry, you don't have a license.", "narrator");
}

/// What the shared prompt is offering this frame.
#[derive(Clone, Copy, PartialEq)]
pub enum PropAction {
    Beer,
    Car,
    /// The Ferris wheel (board / hop off); label and meaning come from `ferris::WheelPrompt`.
    Wheel,
    /// Chillary Clinton NPC beside the Ferris wheel plaza (`chillary.rs`).
    Chillary,
    /// Maria the Pub 217 bartender (`pub_portal.rs`).
    Bartender,
}

/// Beer tower and parked car interaction (CityPlay, normal play and free roam). Both use the city
/// interact input (F / E, gamepad X via index.html, or the on-screen button). Runs after the crew
/// systems so its hint wins a same-frame press, and before `update_city_driving` so a DRIVE tap
/// reaches it the same frame. The beer is open from the start of the city; it only shuts within
/// Mike's interact reach so it can never get in front of his smoke-break cutscene.
#[allow(clippy::too_many_arguments)]
pub fn update_city_props(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut stats: ResMut<PlayerStats>,
    player_q: Query<&Transform, With<Player>>,
    parked_q: Query<(&Transform, &CityCar), (With<ParkedCar>, Without<Player>)>,
    car_q: Query<&Transform, (With<CityCar>, Without<Player>, Without<ParkedCar>)>,
    scooter_q: Query<&Transform, (With<crate::scooter::CityScooter>, Without<Player>)>,
    mut glow_q: Query<
        (&mut Transform, &mut Visibility),
        (
            With<BeerTowerGlow>,
            Without<Player>,
            Without<ParkedCar>,
            Without<CityCar>,
            Without<crate::scooter::CityScooter>,
        ),
    >,
    prompt_q: Query<Entity, With<PropPromptUi>>,
    btn_q: Query<&Interaction, (Changed<Interaction>, With<PropPromptButton>)>,
    mut text_q: Query<&mut Text, With<PropPromptText>>,
    mut wheel: ResMut<crate::ferris::WheelPrompt>,
    mut chillary: ResMut<crate::chillary::ChillaryPrompt>,
    mut bartender: ResMut<crate::pub_portal::BartenderPrompt>,
    mut last: Local<Option<PropAction>>,
) {
    let dt = time.delta_secs().min(0.05);
    stats.license_deny_cd = (stats.license_deny_cd - dt).max(0.0);
    let Ok(p_tf) = player_q.get_single() else {
        return;
    };
    let p = p_tf.translation;
    let flat = |a: Vec3| Vec2::new(a.x - p.x, a.z - p.z).length();
    let on_foot = stats.driving_car.is_none()
        && stats.riding_scooter.is_none()
        && !stats.phone_open
        && !stats.bartender_open
        && !stats.char_select_open
        && p.y < 4.0;

    // Open from the start; only shut while Stan stands where Mike's own [F / E] could fire
    // (Mike is about 9 m from the tower, so in practice never).
    let near_mike = !stats.free_roam && !stats.mike_smoked && flat(mike_pub_spot()) < 4.8;
    let beer_open = !near_mike;
    let beer_size_ok = stats.player_scale <= BEER_MAX_SCALE;
    let tower = beer_tower_world_pos();
    let near_beer = on_foot && beer_open && beer_size_ok && flat(tower) < BEER_REACH;
    // Don't fight the scooter / hijack prompts for the same key.
    let other_vehicle_near = scooter_q.iter().any(|t| flat(t.translation) < 5.2)
        || car_q.iter().any(|t| flat(t.translation) < 5.4);
    let near_car = on_foot
        && stats.player_scale < 6.0
        && !other_vehicle_near
        && parked_q
            .iter()
            .any(|(t, c)| !c.occupied && flat(t.translation) < PARKED_CAR_REACH);
    let action = if wheel.label.is_some() {
        Some(PropAction::Wheel)
    } else if chillary.label.is_some() {
        // Yield to Ferris wheel boarding (`PropAction::Wheel`) so players riding or hopping onto
        // the wheel are never hijacked into the optional boardwalk easter-egg cutscene.
        Some(PropAction::Chillary)
    } else if bartender.label.is_some() {
        Some(PropAction::Bartender)
    } else if near_beer {
        Some(PropAction::Beer)
    } else if near_car {
        Some(PropAction::Car)
    } else {
        None
    };

    let t = time.elapsed_secs();
    for (mut g_tf, mut vis) in &mut glow_q {
        let want = if beer_size_ok { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
        let k = 1.0 + (t * 3.0).sin() * if near_beer { 0.07 } else { 0.03 };
        g_tf.scale = Vec3::new(k, 0.15, k);
    }

    let Some(action) = action else {
        for e in &prompt_q {
            commands.entity(e).despawn_recursive();
        }
        *last = None;
        return;
    };

    let label = match action {
        PropAction::Beer if stats.beer_cooldown > 0.0 => {
            format!("POURING... {:.0}s", stats.beer_cooldown.ceil())
        }
        PropAction::Beer => "DRINK A BEER [F/E]".to_string(),
        PropAction::Car => "DRIVE [F/E]".to_string(),
        PropAction::Wheel => wheel.label.clone().unwrap_or_default(),
        PropAction::Chillary => chillary.label.clone().unwrap_or_default(),
        PropAction::Bartender => bartender.label.clone().unwrap_or_default(),
    };
    if prompt_q.is_empty() {
        commands
            .spawn((
                PropPromptUi,
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(74.0),
                    width: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
            ))
            .with_children(|root| {
                root.spawn((
                    PropPromptButton,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(20.0), Val::Px(10.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.62, 0.38, 0.06)),
                    BorderColor(Color::srgb(1.0, 0.82, 0.35)),
                    BorderRadius::all(Val::Px(10.0)),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        PropPromptText,
                        Text::new(label.clone()),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });
            });
    } else if let Ok(mut txt) = text_q.get_single_mut() {
        if txt.0 != label {
            txt.0 = label;
        }
    }

    // Approach hint once per arrival.
    if *last != Some(action) {
        *last = Some(action);
        if action == PropAction::Beer {
            stats.hint_text = format!(
                "BEER TOWER! [F / E] Drink a beer: +{} hearts, but you'll be drunk for a bit.",
                BEER_HEARTS
            );
        }
    }

    let tapped = btn_q.iter().any(|i| *i == Interaction::Pressed);
    let pressed = keys.just_pressed(KeyCode::KeyF) || keys.just_pressed(KeyCode::KeyE) || tapped;
    if !pressed {
        return;
    }
    match action {
        PropAction::Beer => {
            if stats.beer_cooldown > 0.0 || stats.estus_drink_timer > 0.0 {
                return;
            }
            stats.beer_cooldown = BEER_COOLDOWN_SECS;
            // Same sip pause as the estus flask (slows Stan while it runs).
            stats.estus_drink_timer = 0.9;
            stats.hearts = (stats.hearts + BEER_HEARTS).min(stats.max_hearts);
            stats.drunk_timer = (stats.drunk_timer + BEER_DRUNK_SECS).min(BEER_DRUNK_CAP_SECS);
            play_sfx("estus_drink");
            stats.hint_text = format!(
                "CHEERS! +{} Hearts. The room is starting to sway... (DRUNK {:.0}s)",
                BEER_HEARTS,
                stats.drunk_timer.ceil()
            );
            stats.hint_bump = stats.hint_bump.wrapping_add(1);
        }
        // The parked car is a real `CityCar`: `update_city_driving` mounts it (or refuses it
        // without a license) on F / E itself; a tap on the DRIVE button is forwarded there.
        PropAction::Car => {
            if tapped {
                stats.car_prompt_tap = true;
            }
        }
        // `ferris::ferris_ride_update` decides what the press means this frame.
        PropAction::Wheel => wheel.pressed = true,
        PropAction::Chillary => chillary.pressed = true,
        PropAction::Bartender => bartender.pressed = true,
    }
}

/// OnExit(CityPlay): the prop prompt must not linger over cutscenes.
pub fn despawn_prop_prompt(mut commands: Commands, prompt_q: Query<Entity, With<PropPromptUi>>) {
    for e in &prompt_q {
        commands.entity(e).despawn_recursive();
    }
}
