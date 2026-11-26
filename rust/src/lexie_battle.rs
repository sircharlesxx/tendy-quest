//! Turn-Based 5-Hero RPG Showdown against Giant Lexie (`GameState::LexieBattle`).
//!
//! Triggered in Story Mode (`CityPlay`) once all 50 tendies are collected and Stan engages Lexie
//! (or directly via `?lexie_battle=1`). All 5 squad heroes (`Stan`, `Glenn`, `Nate`, `Derek`, `TJ`)
//! face Lexie with bespoke animated Light `[1]` and Heavy `[2]` moves, rhythmic idle dance/sway,
//! damage recoil reactions, status effects (`STUNNED`, Stan's 4-hour `ASLEEP`), and 156 BPM
//! symphonic JRPG boss music (`playLexieRpgBossMusic` in `rust/index.html`). Defeating Lexie
//! awards +15,000 pts / +100 TW, saves the checkpoint, and transitions to `LexieVictoryCutscene`.

use bevy::prelude::*;
use std::f32::consts::PI;

use crate::audio::{is_voice_speaking, play_lexie_rpg_music, play_sfx, speak_voice, stop_voice};
use crate::derek::DerekArmPivot;
use crate::nate::{NateArmPivot, NateLegPivot};
use crate::net::{CrewStrideArm, CrewStrideLeg};
use crate::state::{GameState, PlayerStats};
use crate::tj::TjLegPivot;

const BATTLE_STAGE: Vec3 = Vec3::new(450.0, 0.0, 450.0);
const LEXIE_HOME: Vec3 = Vec3::new(0.0, 0.0, -14.0);

#[derive(Component)]
pub struct LexieBattleEntity;

#[derive(Component)]
pub struct LexieBattleOverlay;

#[derive(Component)]
pub(crate) struct RpgHero {
    idx: usize,
    home_pos: Vec3,
    home_yaw: f32,
    dance_phase: f32,
}

#[derive(Component)]
pub(crate) struct RpgTurnRing;

#[derive(Component)]
pub(crate) struct RpgLexieRoot;

#[derive(Component)]
pub(crate) struct RpgLexieArm {
    is_left: bool,
}

#[derive(Component)]
pub(crate) struct RpgLexieHead;

#[derive(Component)]
pub(crate) struct RpgStunStar {
    index: usize,
}

#[derive(Component)]
pub(crate) struct RpgVfx {
    age: f32,
    lifetime: f32,
    vel: Vec3,
    spin: f32,
    grow_rate: f32,
}

#[derive(Component)]
pub(crate) struct RpgBossHpFill;

#[derive(Component)]
pub(crate) struct RpgBossStatusText;

#[derive(Component)]
pub(crate) struct RpgBannerText;

#[derive(Component)]
pub(crate) struct RpgPartyCard(usize);

#[derive(Component)]
pub(crate) struct RpgPartyHpFill(usize);

#[derive(Component)]
pub(crate) struct RpgPartyHpText(usize);

#[derive(Component)]
pub(crate) struct RpgPartyStateText(usize);

#[derive(Component)]
pub(crate) struct RpgCmdPanel;

#[derive(Component)]
pub(crate) struct RpgCmdHeader;

#[derive(Component)]
pub(crate) struct RpgCmdBtn(usize);

#[derive(Component)]
pub(crate) struct RpgCmdBtnText(usize);

#[derive(Component)]
pub(crate) struct RpgFloatingText {
    age: f32,
    lifetime: f32,
    vy: f32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RpgPhase {
    Intro,
    PlayerSelect,
    HeroAsleepSkip,
    HeroAction,
    LexieWindup,
    LexieAction,
    VictoryDelay,
    DefeatDelay,
}

#[derive(Clone)]
struct PartyMember {
    name: &'static str,
    hp: i32,
    max_hp: i32,
    light_name: &'static str,
    light_tag: &'static str,
    heavy_name: &'static str,
    heavy_tag: &'static str,
    asleep_turns: u8,
    hit_flash: f32,
    hit_shake: f32,
}

#[derive(Resource)]
pub struct RpgBattleState {
    phase: RpgPhase,
    phase_timer: f32,
    phase_duration: f32,
    actor: usize,
    selected_btn: usize,
    is_heavy: bool,
    hit_applied: bool,
    vfx_stage: u8,
    lexie_hp: f32,
    lexie_max_hp: f32,
    lexie_stunned: u8,
    lexie_stun_immune: u8,
    lexie_hit_flash: f32,
    lexie_attack_kind: u8,
    lexie_target: usize,
    turn_round: u32,
    coop_players: u32,
    coop_online: bool,
    turn_owner_claimed: bool,
    turn_local_owns: bool,
    local_char_id: u8,
    local_vote: Option<bool>,
    remote_votes: Vec<(u8, bool)>,
    vote_coop_active: bool,
    vote_total_players: usize,
    vote_time_left: f32,
    banner_msg: String,
    party: [PartyMember; 5],
}

fn actor_to_char_id(actor: usize) -> u8 {
    match actor {
        0 => crate::net::CHAR_STAN,  // 0
        1 => crate::net::CHAR_GLENN, // 2
        2 => crate::net::CHAR_NATE,  // 1
        3 => crate::net::CHAR_DEREK, // 4
        _ => crate::net::CHAR_TJ,    // 3
    }
}

fn lexie_scaled_max_hp(players: u32) -> f32 {
    1550.0 + (players.clamp(1, 5).saturating_sub(1) as f32) * 450.0
}

fn lexie_coop_damage_mul(players: u32) -> f32 {
    1.0 + (players.clamp(1, 5).saturating_sub(1) as f32) * 0.08
}

/// Refreshes per-turn character ownership and voter counts for the current active hero.
fn sync_turn_ownership(battle: &mut RpgBattleState, net: Option<&crate::net::NetState>) {
    let coop_active = net.is_some_and(|n| n.is_coop_active());
    let sq = crate::audio::net_squad_info();
    let total_voters = if coop_active {
        net.map(|n| n.online_player_count() as usize)
            .unwrap_or(1)
            .max(sq[0] as usize)
            .max(1)
    } else {
        1
    };
    let claimed_mask = sq[7] as u8;
    let local_char = net.map(|n| n.local_char()).unwrap_or(0);
    let actor_char = actor_to_char_id(battle.actor);
    let actor_is_claimed = coop_active && ((claimed_mask & (1 << actor_char)) != 0);
    let local_owns_actor = !coop_active || (actor_is_claimed && local_char == actor_char);

    battle.coop_online = coop_active;
    battle.local_char_id = local_char;
    battle.turn_owner_claimed = actor_is_claimed;
    battle.turn_local_owns = local_owns_actor;
    battle.vote_coop_active = coop_active && !actor_is_claimed;
    battle.vote_total_players = total_voters;
}

fn sync_lexie_coop_scale(
    battle: &mut RpgBattleState,
    stats: &mut PlayerStats,
    coop: Option<&crate::net::NetCoopEvents>,
) {
    let squad_players = crate::audio::net_squad_info()[0].clamp(1, 5);
    let event_players = coop.map(|c| c.lexie_battle_players).unwrap_or(0).clamp(0, 5);
    let target_players = squad_players.max(event_players).max(battle.coop_players).clamp(1, 5);
    if target_players != battle.coop_players {
        battle.coop_players = target_players;
        battle.lexie_max_hp = lexie_scaled_max_hp(target_players);
        battle.lexie_hp = battle.lexie_max_hp;
        stats.boss_max_hp = battle.lexie_max_hp;
        stats.boss_hp = battle.lexie_hp;
        if target_players > 1 && battle.phase == RpgPhase::Intro {
            battle.banner_msg = format!(
                "{}P CO-OP VS LEXIE ({:.0} HP)! EACH PLAYER CONTROLS THEIR HERO!",
                target_players, battle.lexie_max_hp
            );
        }
    }
}

impl Default for RpgBattleState {
    fn default() -> Self {
        Self {
            phase: RpgPhase::Intro,
            phase_timer: 0.0,
            phase_duration: 1.6,
            actor: 0,
            selected_btn: 0,
            is_heavy: false,
            hit_applied: false,
            vfx_stage: 0,
            lexie_hp: 1550.0,
            lexie_max_hp: 1550.0,
            lexie_stunned: 0,
            lexie_stun_immune: 0,
            lexie_hit_flash: 0.0,
            lexie_attack_kind: 0,
            lexie_target: 0,
            turn_round: 1,
            coop_players: 1,
            coop_online: false,
            turn_owner_claimed: false,
            turn_local_owns: true,
            local_char_id: 0,
            local_vote: None,
            remote_votes: Vec::new(),
            vote_coop_active: false,
            vote_total_players: 1,
            vote_time_left: 0.0,
            banner_msg: "ALL 5 HEROES VS LEXIE! PICK YOUR MOVE!".to_string(),
            party: [
                PartyMember {
                    name: "STAN",
                    hp: 145,
                    max_hp: 145,
                    light_name: "ESXi Restore",
                    light_tag: "42 DMG + Party Heal",
                    heavy_name: "The Cycle",
                    heavy_tag: "118 DMG (4h Nap!)",
                    asleep_turns: 0,
                    hit_flash: 0.0,
                    hit_shake: 0.0,
                },
                PartyMember {
                    name: "GLENN",
                    hp: 170,
                    max_hp: 170,
                    light_name: "Power Punch",
                    heavy_tag: "108 DMG Slam",
                    heavy_name: "Powerbomb",
                    light_tag: "58 DMG Combo",
                    asleep_turns: 0,
                    hit_flash: 0.0,
                    hit_shake: 0.0,
                },
                PartyMember {
                    name: "NATE",
                    hp: 140,
                    max_hp: 140,
                    light_name: "Lazy Eye Stun",
                    light_tag: "32 DMG + STUN",
                    heavy_name: "Gum Bubble Pop",
                    heavy_tag: "90 DMG Pop",
                    asleep_turns: 0,
                    hit_flash: 0.0,
                    hit_shake: 0.0,
                },
                PartyMember {
                    name: "DEREK",
                    hp: 150,
                    max_hp: 150,
                    light_name: "Elbow Sweat",
                    light_tag: "52 DMG Splash",
                    heavy_name: "Kamehameha",
                    heavy_tag: "114 DMG Beam",
                    asleep_turns: 0,
                    hit_flash: 0.0,
                    hit_shake: 0.0,
                },
                PartyMember {
                    name: "TJ",
                    hp: 135,
                    max_hp: 135,
                    light_name: "Broken Ankle Complain",
                    light_tag: "28 DMG + STUN",
                    heavy_name: "Spirit Bomb",
                    heavy_tag: "165 DMG Tendy Bomb",
                    asleep_turns: 0,
                    hit_flash: 0.0,
                    hit_shake: 0.0,
                },
            ],
        }
    }
}

const HERO_SLOTS: [(f32, f32, f32); 5] = [
    (-6.8, 8.0, -0.18), // Stan
    (-3.4, 9.2, -0.09), // Glenn
    (0.0, 9.8, 0.0),    // Nate
    (3.4, 9.2, 0.09),   // Derek
    (6.8, 8.0, 0.18),   // TJ
];

fn hero_accent_color(idx: usize) -> Color {
    match idx {
        0 => Color::srgb(0.20, 0.90, 1.0),
        1 => Color::srgb(1.0, 0.42, 0.22),
        2 => Color::srgb(0.45, 0.65, 1.0),
        3 => Color::srgb(0.95, 0.78, 0.25),
        _ => Color::srgb(0.35, 0.95, 0.45),
    }
}

fn find_next_alive_actor(party: &[PartyMember; 5], start_after: Option<usize>) -> Option<usize> {
    let start = match start_after {
        Some(i) => i + 1,
        None => 0,
    };
    for i in start..5 {
        if party[i].hp > 0 {
            return Some(i);
        }
    }
    None
}

fn spawn_floating_popup(
    commands: &mut Commands,
    text: impl Into<String>,
    color: Color,
    left_pct: f32,
    top_pct: f32,
) {
    commands
        .spawn((
            LexieBattleOverlay,
            RpgFloatingText {
                age: 0.0,
                lifetime: 1.5,
                vy: -42.0,
            },
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(left_pct),
                top: Val::Percent(top_pct),
                ..default()
            },
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(text.into()),
                TextFont {
                    font_size: 28.0,
                    ..default()
                },
                TextColor(color),
            ));
        });
}

fn hero_screen_pos(idx: usize) -> (f32, f32) {
    match idx {
        0 => (18.0, 56.0),
        1 => (32.0, 59.0),
        2 => (48.0, 61.0),
        3 => (64.0, 59.0),
        _ => (78.0, 56.0),
    }
}

pub fn enter_lexie_battle(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    mut stats: ResMut<PlayerStats>,
    mut battle: ResMut<RpgBattleState>,
    coop: Option<Res<crate::net::NetCoopEvents>>,
    mut cam_q: Query<(&mut Transform, &mut Projection), With<Camera3d>>,
) {
    crate::audio::net_set_scene(6, stats.tendies >= 12);
    *battle = RpgBattleState::default();
    let squad_players = crate::audio::net_squad_info()[0].clamp(1, 5);
    let event_players = coop.as_ref().map(|c| c.lexie_battle_players).unwrap_or(0).clamp(0, 5);
    let players = squad_players.max(event_players).clamp(1, 5);
    battle.coop_players = players;
    battle.lexie_max_hp = lexie_scaled_max_hp(players);
    battle.lexie_hp = battle.lexie_max_hp;
    battle.phase_duration = 1.6;
    if players > 1 {
        battle.banner_msg = format!(
            "{}P CO-OP VS LEXIE ({:.0} HP)! EACH PLAYER CONTROLS THEIR HERO!",
            players, battle.lexie_max_hp
        );
    }
    stats.boss_mode = true;
    stats.boss_hp = battle.lexie_hp;
    stats.boss_max_hp = battle.lexie_max_hp;
    stats.in_city = true;
    stats.char_select_open = false;

    crate::audio::set_touch_playing(false);
    play_lexie_rpg_music();
    speak_voice(
        "All five of us together! Let's finish Lexie once and for all!",
        "stan",
    );

    // 1. Cedar Rapids twilight arena street + glowing RPG battle ring + skyline silhouettes
    let asphalt_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.11, 0.12, 0.15),
        perceptual_roughness: 0.78,
        ..default()
    });
    let ring_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.18, 0.65, 0.95),
        emissive: LinearRgba::rgb(0.25, 0.85, 1.4),
        perceptual_roughness: 0.25,
        ..default()
    });
    let bldg_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.10, 0.11, 0.16),
        perceptual_roughness: 0.75,
        ..default()
    });
    let win_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.86, 0.45),
        emissive: LinearRgba::rgb(1.6, 1.25, 0.45),
        ..default()
    });
    let lamp_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.90, 0.62),
        emissive: LinearRgba::rgb(2.4, 1.9, 0.9),
        ..default()
    });
    let quaker_fire_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.42, 0.08),
        emissive: LinearRgba::rgb(3.2, 1.1, 0.15),
        ..default()
    });

    commands.spawn((
        LexieBattleEntity,
        Mesh3d(meshes.add(Cuboid::new(140.0, 0.2, 160.0))),
        MeshMaterial3d(asphalt_mat),
        Transform::from_translation(BATTLE_STAGE + Vec3::new(0.0, -0.1, 0.0)),
    ));

    // Glowing arena circle under fighters
    commands.spawn((
        LexieBattleEntity,
        Mesh3d(meshes.add(Torus::new(14.35, 14.55))),
        MeshMaterial3d(ring_mat),
        Transform::from_translation(BATTLE_STAGE + Vec3::new(0.0, 0.06, -2.0)),
    ));

    // Flanking downtown towers & streetlights
    let unit_box = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let lamp_sphere = meshes.add(Sphere::new(0.32));
    for (bx, bz, bw, bh, bd) in [
        (-28.0_f32, -8.0_f32, 14.0_f32, 46.0_f32, 16.0_f32),
        (-32.0, 16.0, 14.0, 34.0, 14.0),
        (-29.0, -34.0, 16.0, 56.0, 18.0),
        (28.0, -8.0, 14.0, 48.0, 16.0),
        (32.0, 16.0, 14.0, 36.0, 14.0),
        (29.0, -34.0, 16.0, 52.0, 18.0),
        (0.0, -68.0, 26.0, 38.0, 18.0), // Distant Quaker Oats mill
    ] {
        commands
            .spawn((
                LexieBattleEntity,
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(bldg_mat.clone()),
                Transform::from_translation(BATTLE_STAGE + Vec3::new(bx, bh * 0.5, bz))
                    .with_scale(Vec3::new(bw, bh, bd)),
            ))
            .with_children(|b| {
                b.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(win_mat.clone()),
                    Transform::from_xyz(0.0, 0.15, 0.0).with_scale(Vec3::new(1.01, 0.04, 1.01)),
                ));
            });
    }
    // Distant Quaker Oats flames
    commands.spawn((
        LexieBattleEntity,
        Mesh3d(meshes.add(Sphere::new(6.0))),
        MeshMaterial3d(quaker_fire_mat),
        Transform::from_translation(BATTLE_STAGE + Vec3::new(0.0, 39.0, -68.0)),
    ));

    for sx in [-12.5_f32, 12.5] {
        for lz in [-8.0_f32, 6.0, 18.0] {
            commands.spawn((
                LexieBattleEntity,
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(bldg_mat.clone()),
                Transform::from_translation(BATTLE_STAGE + Vec3::new(sx, 3.2, lz))
                    .with_scale(Vec3::new(0.18, 6.4, 0.18)),
            ));
            commands.spawn((
                LexieBattleEntity,
                Mesh3d(lamp_sphere.clone()),
                MeshMaterial3d(lamp_mat.clone()),
                Transform::from_translation(BATTLE_STAGE + Vec3::new(sx - sx.signum() * 0.6, 6.4, lz)),
            ));
        }
    }

    // Active-turn glowing ring
    let turn_ring_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.88, 0.25),
        emissive: LinearRgba::rgb(2.6, 2.1, 0.35),
        ..default()
    });
    commands.spawn((
        LexieBattleEntity,
        RpgTurnRing,
        Mesh3d(meshes.add(Torus::new(1.38, 1.56))),
        MeshMaterial3d(turn_ring_mat),
        Transform::from_translation(BATTLE_STAGE + Vec3::new(HERO_SLOTS[0].0, 0.08, HERO_SLOTS[0].1)),
    ));

    // 2. Spawn All 5 Heroes
    // 0: Stan
    let hero_scene: Handle<Scene> = asset_server.load("models/plaid_hero.glb#Scene0");
    commands.spawn((
        LexieBattleEntity,
        RpgHero {
            idx: 0,
            home_pos: BATTLE_STAGE + Vec3::new(HERO_SLOTS[0].0, 0.0, HERO_SLOTS[0].1),
            home_yaw: HERO_SLOTS[0].2,
            dance_phase: 0.0,
        },
        SceneRoot(hero_scene),
        Transform::from_translation(BATTLE_STAGE + Vec3::new(HERO_SLOTS[0].0, 0.0, HERO_SLOTS[0].1))
            .with_rotation(Quat::from_rotation_y(HERO_SLOTS[0].2))
            .with_scale(Vec3::splat(2.75)),
    ));

    // Cozy blanket + pillow + floating Zzzs for Stan's Cycle nap (#11)
    let pillow_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.97, 0.96, 0.92),
        perceptual_roughness: 0.85,
        ..default()
    });
    let blankie_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.32, 0.14, 0.54),
        perceptual_roughness: 0.78,
        ..default()
    });
    let blankie_trim = materials.add(StandardMaterial {
        base_color: Color::srgb(0.98, 0.78, 0.14),
        perceptual_roughness: 0.65,
        ..default()
    });
    let zzz_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.90, 1.0),
        emissive: LinearRgba::rgb(0.7, 1.5, 2.6),
        unlit: true,
        ..default()
    });
    commands
        .spawn((
            LexieBattleEntity,
            StanNapBlankie,
            Transform::from_translation(BATTLE_STAGE + Vec3::new(HERO_SLOTS[0].0, 0.04, HERO_SLOTS[0].1))
                .with_rotation(Quat::from_rotation_y(HERO_SLOTS[0].2)),
            Visibility::Hidden,
        ))
        .with_children(|nb| {
            // Fluffy white pillow under Stan's head (Stan tips +X in local frame when rolling PI*0.48)
            nb.spawn((
                Mesh3d(meshes.add(Cuboid::new(1.35, 0.42, 1.85))),
                MeshMaterial3d(pillow_mat),
                Transform::from_xyz(-3.55, 0.34, 0.0).with_rotation(Quat::from_rotation_y(0.14)),
            ));
            // Pillow piping strip
            nb.spawn((
                Mesh3d(meshes.add(Cuboid::new(1.42, 0.10, 1.90))),
                MeshMaterial3d(blankie_trim.clone()),
                Transform::from_xyz(-3.55, 0.34, 0.0).with_rotation(Quat::from_rotation_y(0.14)),
            ));
            // Vikings purple plaid blankie draped cozily over Stan's napping body
            nb.spawn((
                Mesh3d(meshes.add(Cylinder::new(1.22, 3.25))),
                MeshMaterial3d(blankie_mat),
                Transform::from_xyz(-1.35, 0.62, 0.0)
                    .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2))
                    .with_scale(Vec3::new(0.62, 1.0, 1.38)),
            ));
            // Gold quilt stripe along the top fold of the blankie
            nb.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.45, 0.16, 2.95))),
                MeshMaterial3d(blankie_trim),
                Transform::from_xyz(-2.65, 1.28, 0.0),
            ));
            // Floating glowing "Zzz" sleep bubbles
            for zi in 0..3_usize {
                nb.spawn((
                    StanNapZzz(zi as f32 * 2.1),
                    Mesh3d(meshes.add(Sphere::new(0.22 + zi as f32 * 0.06))),
                    MeshMaterial3d(zzz_mat.clone()),
                    Transform::from_xyz(-3.7, 1.35 + zi as f32 * 0.65, 0.35),
                ));
            }
        });

    // 1: Glenn
    let glenn_mats = crate::glenn::make_glenn_materials(&mut materials);
    commands
        .spawn((
            LexieBattleEntity,
            RpgHero {
                idx: 1,
                home_pos: BATTLE_STAGE + Vec3::new(HERO_SLOTS[1].0, 0.0, HERO_SLOTS[1].1),
                home_yaw: PI + HERO_SLOTS[1].2,
                dance_phase: 1.2,
            },
            Transform::from_translation(BATTLE_STAGE + Vec3::new(HERO_SLOTS[1].0, 0.0, HERO_SLOTS[1].1))
                .with_rotation(Quat::from_rotation_y(PI + HERO_SLOTS[1].2))
                .with_scale(Vec3::splat(2.45)),
            Visibility::default(),
        ))
        .with_children(|gl| {
            crate::glenn::spawn_glenn_rig(gl, &mut meshes, &glenn_mats);
        });

    // 2: Nate
    let nate_mats = crate::nate::make_nate_materials(&mut materials);
    commands
        .spawn((
            LexieBattleEntity,
            RpgHero {
                idx: 2,
                home_pos: BATTLE_STAGE + Vec3::new(HERO_SLOTS[2].0, 0.0, HERO_SLOTS[2].1),
                home_yaw: PI + HERO_SLOTS[2].2,
                dance_phase: 2.4,
            },
            Transform::from_translation(BATTLE_STAGE + Vec3::new(HERO_SLOTS[2].0, 0.0, HERO_SLOTS[2].1))
                .with_rotation(Quat::from_rotation_y(PI + HERO_SLOTS[2].2))
                .with_scale(Vec3::splat(2.45)),
            Visibility::default(),
        ))
        .with_children(|nt| {
            crate::nate::spawn_nate_rig(nt, &mut meshes, &nate_mats, false);
        });

    // 3: Derek
    let derek_mats = crate::derek::make_derek_materials(&mut materials);
    commands
        .spawn((
            LexieBattleEntity,
            RpgHero {
                idx: 3,
                home_pos: BATTLE_STAGE + Vec3::new(HERO_SLOTS[3].0, 0.0, HERO_SLOTS[3].1),
                home_yaw: PI + HERO_SLOTS[3].2,
                dance_phase: 3.6,
            },
            Transform::from_translation(BATTLE_STAGE + Vec3::new(HERO_SLOTS[3].0, 0.0, HERO_SLOTS[3].1))
                .with_rotation(Quat::from_rotation_y(PI + HERO_SLOTS[3].2))
                .with_scale(Vec3::splat(2.45)),
            Visibility::default(),
        ))
        .with_children(|dk| {
            crate::derek::spawn_derek_rig(dk, &mut meshes, &derek_mats, false);
        });

    // 4: TJ
    let tj_ent = crate::tj::spawn_tj_character(
        &mut commands,
        &mut meshes,
        &mut materials,
        BATTLE_STAGE + Vec3::new(HERO_SLOTS[4].0, 0.0, HERO_SLOTS[4].1),
        PI + HERO_SLOTS[4].2,
        2.45,
        false,
    );
    commands.entity(tj_ent).insert((
        LexieBattleEntity,
        RpgHero {
            idx: 4,
            home_pos: BATTLE_STAGE + Vec3::new(HERO_SLOTS[4].0, 0.0, HERO_SLOTS[4].1),
            home_yaw: PI + HERO_SLOTS[4].2,
            dance_phase: 4.8,
        },
    ));

    // 3. Spawn Giant Lexie Boss (articulated arms + head + stun stars)
    spawn_rpg_lexie(&mut commands, &mut meshes, &mut materials);

    // 4. Camera setup
    if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
        c_tf.translation = BATTLE_STAGE + Vec3::new(14.5, 11.5, 29.0);
        c_tf.look_at(BATTLE_STAGE + Vec3::new(0.0, 9.5, -5.0), Vec3::Y);
        if let Projection::Perspective(ref mut p) = *proj {
            p.fov = 0.72;
        }
    }

    // 5. Spawn RPG HUD & Turn Menu
    spawn_rpg_ui(&mut commands, &battle);
}

fn spawn_rpg_lexie(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let lm = crate::city::lexie_meshes(meshes);
    let unit_sphere = meshes.add(Sphere::new(1.0));
    let skin = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.78, 0.69),
        perceptual_roughness: 0.52,
        ..default()
    });
    let pink_hoodie = materials.add(StandardMaterial {
        base_color: Color::srgb(0.96, 0.25, 0.56),
        perceptual_roughness: 0.62,
        ..default()
    });
    let denim_shorts = materials.add(StandardMaterial {
        base_color: Color::srgb(0.18, 0.34, 0.62),
        perceptual_roughness: 0.78,
        ..default()
    });
    let blonde_hair = materials.add(StandardMaterial {
        base_color: Color::srgb(0.96, 0.80, 0.28),
        perceptual_roughness: 0.42,
        ..default()
    });
    let red_lip = materials.add(StandardMaterial {
        base_color: Color::srgb(0.86, 0.10, 0.22),
        perceptual_roughness: 0.28,
        ..default()
    });
    let eye_dark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.10, 0.08, 0.14),
        perceptual_roughness: 0.2,
        ..default()
    });
    let sneaker = materials.add(StandardMaterial {
        base_color: Color::srgb(0.96, 0.96, 0.98),
        perceptual_roughness: 0.5,
        ..default()
    });
    let star_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.92, 0.20),
        emissive: LinearRgba::rgb(2.6, 2.2, 0.25),
        ..default()
    });

    commands
        .spawn((
            LexieBattleEntity,
            RpgLexieRoot,
            Transform::from_translation(BATTLE_STAGE + LEXIE_HOME).with_scale(Vec3::splat(5.4)),
            Visibility::default(),
        ))
        .with_children(|lx| {
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
                    Transform::from_xyz(sx * 0.44, 1.56, 0.0)
                        .with_scale(Vec3::new(0.56, 0.46, 0.54)),
                ));
            }
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
                Mesh3d(lm.strings),
                MeshMaterial3d(sneaker),
                Transform::default(),
            ));

            // Head pivot
            lx.spawn((
                RpgLexieHead,
                Transform::from_xyz(0.0, 3.92, 0.06),
                Visibility::default(),
            ))
            .with_children(|hd| {
                hd.spawn((Mesh3d(lm.head), MeshMaterial3d(skin.clone()), Transform::default()));
                hd.spawn((
                    Mesh3d(unit_sphere.clone()),
                    MeshMaterial3d(skin.clone()),
                    Transform::from_xyz(0.0, -0.35, 0.12)
                        .with_scale(Vec3::new(0.46, 0.22, 0.42)),
                ));
                hd.spawn((
                    Mesh3d(lm.nose),
                    MeshMaterial3d(skin.clone()),
                    Transform::from_xyz(0.0, -0.03, 0.59),
                ));
                for sx in [-1.0_f32, 1.0_f32] {
                    hd.spawn((
                        Mesh3d(lm.eye.clone()),
                        MeshMaterial3d(eye_dark.clone()),
                        Transform::from_xyz(sx * 0.20, 0.08, 0.54).with_rotation(
                            Quat::from_rotation_y(sx * 0.32) * Quat::from_rotation_z(sx * 0.18),
                        ),
                    ));
                    hd.spawn((
                        Mesh3d(lm.brow.clone()),
                        MeshMaterial3d(blonde_hair.clone()),
                        Transform::from_xyz(sx * 0.21, 0.20, 0.54).with_rotation(
                            Quat::from_rotation_y(sx * 0.32) * Quat::from_rotation_z(-sx * 0.28),
                        ),
                    ));
                }
                hd.spawn((
                    Mesh3d(lm.lips),
                    MeshMaterial3d(red_lip),
                    Transform::from_xyz(0.0, -0.18, 0.56),
                ));
                hd.spawn((
                    Mesh3d(lm.hair),
                    MeshMaterial3d(blonde_hair.clone()),
                    Transform::default(),
                ));
                hd.spawn((
                    Mesh3d(lm.mane),
                    MeshMaterial3d(blonde_hair.clone()),
                    Transform::from_xyz(0.0, -0.30, -0.40),
                ));
                hd.spawn((
                    Mesh3d(lm.lock_l),
                    MeshMaterial3d(blonde_hair.clone()),
                    Transform::default(),
                ));
                hd.spawn((
                    Mesh3d(lm.lock_r),
                    MeshMaterial3d(blonde_hair),
                    Transform::default(),
                ));
            });

            // Articulated Left & Right Arms
            for (idx, sx) in [-1.0_f32, 1.0_f32].iter().enumerate() {
                lx.spawn((
                    RpgLexieArm { is_left: idx == 0 },
                    Transform::from_xyz(sx * 1.15, 3.05, 0.05),
                    Visibility::default(),
                ))
                .with_children(|arm| {
                    arm.spawn((
                        Mesh3d(if idx == 0 { lm.sleeve.clone() } else { lm.sleeve_long.clone() }),
                        MeshMaterial3d(pink_hoodie.clone()),
                        Transform::from_xyz(0.0, -0.46, 0.0),
                    ));
                    arm.spawn((
                        Mesh3d(if idx == 0 { lm.hand_l.clone() } else { lm.fist.clone() }),
                        MeshMaterial3d(skin.clone()),
                        Transform::from_xyz(0.0, -0.96, 0.14),
                    ));
                });
            }

            // 3 Dizzy Stun Stars above head (hidden until stunned)
            let star_mesh = meshes.add(Tetrahedron::default());
            for i in 0..3 {
                lx.spawn((
                    RpgStunStar { index: i },
                    Mesh3d(star_mesh.clone()),
                    MeshMaterial3d(star_mat.clone()),
                    Transform::from_xyz(0.0, 4.9, 0.0),
                    Visibility::Hidden,
                ));
            }
        });
}

fn spawn_rpg_ui(commands: &mut Commands, battle: &RpgBattleState) {
    commands
        .spawn((
            LexieBattleOverlay,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::all(Val::Px(14.0)),
                ..default()
            },
        ))
        .with_children(|root| {
            // Top Boss Bar + Banner
            root.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(8.0),
                ..default()
            })
            .with_children(|top| {
                top.spawn((
                    Node {
                        width: Val::Px(520.0),
                        max_width: Val::Percent(92.0),
                        padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(5.0),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.06, 0.04, 0.12, 0.88)),
                    BorderColor(Color::srgb(0.95, 0.25, 0.60)),
                ))
                .with_children(|box_node| {
                    box_node.spawn((
                        RpgBossStatusText,
                        Text::new(format!(
                            "LEXIE - PICKLED GARLIC QUEEN   HP {:.0} / {:.0}",
                            battle.lexie_hp, battle.lexie_max_hp
                        )),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.88, 0.45)),
                    ));
                    box_node
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(14.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.18, 0.08, 0.16)),
                        ))
                        .with_children(|track| {
                            track.spawn((
                                RpgBossHpFill,
                                Node {
                                    width: Val::Percent(100.0),
                                    height: Val::Percent(100.0),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.95, 0.20, 0.55)),
                            ));
                        });
                });

                top.spawn((
                    Node {
                        padding: UiRect::axes(Val::Px(16.0), Val::Px(6.0)),
                        border: UiRect::all(Val::Px(1.5)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.04, 0.07, 0.14, 0.86)),
                    BorderColor(Color::srgb(0.32, 0.78, 1.0)),
                ))
                .with_children(|bn| {
                    bn.spawn((
                        RpgBannerText,
                        Text::new(battle.banner_msg.clone()),
                        TextFont {
                            font_size: 18.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });
            });

            // Bottom Row: Party Strip (Left) + Turn Command Menu (Right)
            root.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexEnd,
                column_gap: Val::Px(8.0),
                ..default()
            })
            .with_children(|bot| {
                // Party Cards
                bot.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(5.0),
                    ..default()
                })
                .with_children(|strip| {
                    for (i, m) in battle.party.iter().enumerate() {
                        strip
                            .spawn((
                                RpgPartyCard(i),
                                Node {
                                    width: Val::Px(106.0),
                                    padding: UiRect::all(Val::Px(6.0)),
                                    flex_direction: FlexDirection::Column,
                                    row_gap: Val::Px(2.0),
                                    border: UiRect::all(Val::Px(2.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.05, 0.08, 0.15, 0.90)),
                                BorderColor(hero_accent_color(i)),
                            ))
                            .with_children(|card| {
                                card.spawn((
                                    Text::new(m.name),
                                    TextFont {
                                        font_size: 14.0,
                                        ..default()
                                    },
                                    TextColor(hero_accent_color(i)),
                                ));
                                card.spawn((
                                    RpgPartyHpText(i),
                                    Text::new(format!("{}/{} HP", m.hp, m.max_hp)),
                                    TextFont {
                                        font_size: 12.0,
                                        ..default()
                                    },
                                    TextColor(Color::WHITE),
                                ));
                                card.spawn((
                                    Node {
                                        width: Val::Percent(100.0),
                                        height: Val::Px(7.0),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgb(0.14, 0.18, 0.25)),
                                ))
                                .with_children(|bar| {
                                    bar.spawn((
                                        RpgPartyHpFill(i),
                                        Node {
                                            width: Val::Percent(100.0),
                                            height: Val::Percent(100.0),
                                            ..default()
                                        },
                                        BackgroundColor(Color::srgb(0.25, 0.90, 0.45)),
                                    ));
                                });
                                card.spawn((
                                    RpgPartyStateText(i),
                                    Text::new("READY"),
                                    TextFont {
                                        font_size: 11.0,
                                        ..default()
                                    },
                                    TextColor(Color::srgb(0.75, 0.88, 1.0)),
                                ));
                            });
                    }
                });

                // Command Menu
                bot.spawn((
                    RpgCmdPanel,
                    Node {
                        padding: UiRect::all(Val::Px(10.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(6.0),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.04, 0.08, 0.18, 0.94)),
                    BorderColor(Color::srgb(1.0, 0.85, 0.25)),
                    Visibility::Hidden,
                ))
                .with_children(|cmd| {
                    cmd.spawn((
                        RpgCmdHeader,
                        Text::new("STAN'S TURN - [1 / 2] OR CLICK"),
                        TextFont {
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.88, 0.35)),
                    ));
                    cmd.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(8.0),
                        ..default()
                    })
                    .with_children(|row| {
                        for b in 0..2 {
                            row.spawn((
                                RpgCmdBtn(b),
                                Button,
                                Node {
                                    padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                                    border: UiRect::all(Val::Px(2.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.10, 0.18, 0.32)),
                                BorderColor(Color::srgb(0.35, 0.78, 1.0)),
                            ))
                            .with_children(|btn| {
                                btn.spawn((
                                    RpgCmdBtnText(b),
                                    Text::new(""),
                                    TextFont {
                                        font_size: 13.0,
                                        ..default()
                                    },
                                    TextColor(Color::WHITE),
                                ));
                            });
                        }
                    });
                });
            });
        });
}

pub fn exit_lexie_battle(
    mut commands: Commands,
    mut stats: ResMut<PlayerStats>,
    mut coop: Option<ResMut<crate::net::NetCoopEvents>>,
    ents: Query<Entity, With<LexieBattleEntity>>,
    overlays: Query<Entity, With<LexieBattleOverlay>>,
) {
    stop_voice();
    crate::audio::net_set_scene(0, stats.tendies >= 12);
    stats.boss_mode = false;
    if let Some(ref mut ce) = coop {
        ce.lexie_moves.clear();
        ce.lexie_commits.clear();
        ce.lexie_battle_players = 0;
    }
    for e in &ents {
        commands.entity(e).despawn_recursive();
    }
    for o in &overlays {
        commands.entity(o).despawn_recursive();
    }
}

pub fn update_lexie_battle(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut stats: ResMut<PlayerStats>,
    mut battle: ResMut<RpgBattleState>,
    mut next_state: crate::transition::SceneTransition,
    net: Option<Res<crate::net::NetState>>,
    mut coop: Option<ResMut<crate::net::NetCoopEvents>>,
    mut btn_q: Query<(&RpgCmdBtn, &Interaction, &mut BackgroundColor, &mut BorderColor)>,
) {
    let dt = time.delta_secs().min(0.15);
    battle.phase_timer += dt;
    if battle.lexie_hit_flash > 0.0 {
        battle.lexie_hit_flash = (battle.lexie_hit_flash - dt).max(0.0);
    }
    for m in &mut battle.party {
        if m.hit_flash > 0.0 {
            m.hit_flash = (m.hit_flash - dt).max(0.0);
        }
        if m.hit_shake > 0.0 {
            m.hit_shake = (m.hit_shake - dt).max(0.0);
        }
    }

    match battle.phase {
        RpgPhase::Intro => {
            if (battle.lexie_hp - battle.lexie_max_hp).abs() < 0.01 {
                sync_lexie_coop_scale(&mut battle, &mut stats, coop.as_deref());
            }
            if phase_finished_with_voice(&battle, 6.0) {
                start_actor_turn(&mut battle, 0);
            }
        }
        RpgPhase::HeroAsleepSkip => {
            if battle.phase_timer >= battle.phase_duration {
                let a = battle.actor;
                battle.party[a].asleep_turns = battle.party[a].asleep_turns.saturating_sub(1);
                advance_after_hero(&mut battle);
            }
        }
        RpgPhase::PlayerSelect => {
            if battle.turn_round == 1
                && battle.actor == 0
                && (battle.lexie_hp - battle.lexie_max_hp).abs() < 0.01
                && battle.local_vote.is_none()
            {
                sync_lexie_coop_scale(&mut battle, &mut stats, coop.as_deref());
            }
            sync_turn_ownership(&mut battle, net.as_deref());
            let coop_active = battle.coop_online;
            let total_voters = battle.vote_total_players;
            let is_anchor = net.as_ref().is_some_and(|n| n.is_anchor());
            let actor_is_claimed = battle.turn_owner_claimed;
            let local_owns_actor = battle.turn_local_owns;
            let can_input = !coop_active || !actor_is_claimed || local_owns_actor;

            if coop_active {
                // Step A: Authoritative commit drain (applies matching (round, actor) move immediately).
                let mut matched_commit: Option<bool> = None;
                if let Some(ref mut ce) = coop {
                    let cur_round = battle.turn_round;
                    let cur_actor = battle.actor;
                    let mut kept = Vec::new();
                    for (round, actor, is_heavy) in ce.lexie_commits.drain(..) {
                        if round == cur_round && actor == cur_actor {
                            if matched_commit.is_none() {
                                matched_commit = Some(is_heavy);
                            }
                        } else if round > cur_round || (round == cur_round && actor > cur_actor) {
                            kept.push((round, actor, is_heavy));
                        }
                    }
                    ce.lexie_commits = kept;
                }
                if let Some(committed_heavy) = matched_commit {
                    battle.local_vote = None;
                    battle.remote_votes.clear();
                    begin_hero_action(&mut battle, committed_heavy);
                    return;
                }

                // Step B: Vote drain for unclaimed turns (prunes stale rounds/turns).
                if let Some(ref mut ce) = coop {
                    let cur_round = battle.turn_round;
                    let cur_actor = battle.actor;
                    let mut kept = Vec::new();
                    for (slot, round, actor, is_heavy) in ce.lexie_moves.drain(..) {
                        if round == cur_round && actor == cur_actor {
                            if let Some(existing) = battle.remote_votes.iter_mut().find(|(s, _)| *s == slot) {
                                existing.1 = is_heavy;
                            } else {
                                battle.remote_votes.push((slot, is_heavy));
                            }
                        } else if round > cur_round || (round == cur_round && actor > cur_actor) {
                            kept.push((slot, round, actor, is_heavy));
                        }
                    }
                    ce.lexie_moves = kept;
                }
            } else if let Some(ref mut ce) = coop {
                ce.lexie_moves.clear();
                ce.lexie_commits.clear();
            }

            // Step C: Input gating & button visuals
            let mut chosen: Option<bool> = None;
            if can_input {
                if keys.just_pressed(KeyCode::Digit1) {
                    chosen = Some(false);
                } else if keys.just_pressed(KeyCode::Digit2) {
                    chosen = Some(true);
                } else if keys.just_pressed(KeyCode::ArrowLeft) || keys.just_pressed(KeyCode::KeyA) {
                    battle.selected_btn = 0;
                } else if keys.just_pressed(KeyCode::ArrowRight) || keys.just_pressed(KeyCode::KeyD) {
                    battle.selected_btn = 1;
                } else if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) {
                    chosen = Some(battle.selected_btn == 1);
                }

                for gp in &gamepads {
                    if gp.just_pressed(GamepadButton::DPadLeft) {
                        battle.selected_btn = 0;
                    } else if gp.just_pressed(GamepadButton::DPadRight) {
                        battle.selected_btn = 1;
                    } else if gp.just_pressed(GamepadButton::South) {
                        chosen = Some(battle.selected_btn == 1);
                    }
                }
            }

            for (btn, inter, mut bg, mut border) in &mut btn_q {
                let hovered = can_input && *inter == Interaction::Hovered;
                let pressed = can_input && *inter == Interaction::Pressed;
                if hovered {
                    battle.selected_btn = btn.0;
                }
                if pressed {
                    chosen = Some(btn.0 == 1);
                }
                let voted_this = can_input && battle.local_vote == Some(btn.0 == 1);
                let sel = can_input && (battle.selected_btn == btn.0 || voted_this);
                *bg = BackgroundColor(if !can_input {
                    Color::srgb(0.07, 0.09, 0.15)
                } else if voted_this {
                    Color::srgb(0.14, 0.42, 0.28)
                } else if sel {
                    Color::srgb(0.16, 0.30, 0.56)
                } else {
                    Color::srgb(0.08, 0.14, 0.26)
                });
                *border = BorderColor(if !can_input {
                    Color::srgb(0.26, 0.32, 0.46)
                } else if voted_this {
                    Color::srgb(0.45, 1.0, 0.55)
                } else if sel {
                    Color::srgb(1.0, 0.88, 0.25)
                } else {
                    Color::srgb(0.32, 0.65, 0.90)
                });
            }

            // Step D: Action resolution
            if !coop_active {
                battle.vote_total_players = 1;
                battle.vote_time_left = 0.0;
                if let Some(is_heavy) = chosen {
                    begin_hero_action(&mut battle, is_heavy);
                }
            } else if actor_is_claimed {
                let deadline = 20.0;
                battle.vote_total_players = total_voters;
                battle.vote_time_left = (deadline - battle.phase_timer).max(0.0);
                if local_owns_actor {
                    if let Some(is_heavy) = chosen {
                        crate::audio::net_event(
                            crate::net::EVENT_LEXIE_COMMIT,
                            crate::net::pack_lexie_move(battle.turn_round, battle.actor, is_heavy),
                        );
                        begin_hero_action(&mut battle, is_heavy);
                        return;
                    }
                }
                let fallback_heavy = (battle.turn_round + battle.actor as u32) % 2 == 1;
                if is_anchor && battle.phase_timer >= deadline {
                    crate::audio::net_event(
                        crate::net::EVENT_LEXIE_COMMIT,
                        crate::net::pack_lexie_move(battle.turn_round, battle.actor, fallback_heavy),
                    );
                    begin_hero_action(&mut battle, fallback_heavy);
                } else if !is_anchor && battle.phase_timer >= deadline + 4.0 {
                    begin_hero_action(&mut battle, fallback_heavy);
                }
            } else {
                if let Some(is_heavy) = chosen {
                    if battle.local_vote != Some(is_heavy) {
                        battle.local_vote = Some(is_heavy);
                        crate::audio::net_event(
                            crate::net::EVENT_LEXIE_MOVE,
                            crate::net::pack_lexie_move(battle.turn_round, battle.actor, is_heavy),
                        );
                    }
                }
                let total_cast = usize::from(battle.local_vote.is_some()) + battle.remote_votes.len();
                let deadline = if total_cast > 0 { 8.0 } else { 15.0 };
                battle.vote_total_players = total_voters;
                battle.vote_time_left = (deadline - battle.phase_timer).max(0.0);

                let winner_is_heavy = {
                    let mut light_votes = usize::from(battle.local_vote == Some(false));
                    let mut heavy_votes = usize::from(battle.local_vote == Some(true));
                    for &(_, h) in &battle.remote_votes {
                        if h {
                            heavy_votes += 1;
                        } else {
                            light_votes += 1;
                        }
                    }
                    if heavy_votes > light_votes {
                        true
                    } else if light_votes > heavy_votes {
                        false
                    } else {
                        (battle.turn_round + battle.actor as u32) % 2 == 1
                    }
                };
                if is_anchor && (total_cast >= total_voters || battle.phase_timer >= deadline) {
                    crate::audio::net_event(
                        crate::net::EVENT_LEXIE_COMMIT,
                        crate::net::pack_lexie_move(battle.turn_round, battle.actor, winner_is_heavy),
                    );
                    battle.local_vote = None;
                    battle.remote_votes.clear();
                    begin_hero_action(&mut battle, winner_is_heavy);
                } else if !is_anchor && battle.phase_timer >= deadline + 4.0 {
                    battle.local_vote = None;
                    battle.remote_votes.clear();
                    begin_hero_action(&mut battle, winner_is_heavy);
                }
            }
        }
        RpgPhase::HeroAction => {
            run_hero_action_vfx(
                &mut commands,
                &mut meshes,
                &mut materials,
                &mut battle,
                &mut stats,
            );
            if phase_finished_with_voice(&battle, 6.0) {
                if battle.lexie_hp <= 0.0 {
                    battle.lexie_hp = 0.0;
                    battle.phase = RpgPhase::VictoryDelay;
                    battle.phase_timer = 0.0;
                    battle.phase_duration = 2.1;
                    battle.banner_msg = "LEXIE DEFEATED!! CEDAR RAPIDS IS SAVED!!".to_string();
                    play_sfx("win");
                    speak_voice(
                        "Noooo! My pickled garlic empire! You guys actually teamed up!",
                        "lexie",
                    );
                } else {
                    advance_after_hero(&mut battle);
                }
            }
        }
        RpgPhase::LexieWindup => {
            if battle.lexie_stunned > 0 {
                if phase_finished_with_voice(&battle, 5.0) {
                    battle.lexie_stunned -= 1;
                    battle.turn_round += 1;
                    if let Some(first) = find_next_alive_actor(&battle.party, None) {
                        start_actor_turn(&mut battle, first);
                    }
                }
            } else if battle.phase_timer >= battle.phase_duration {
                battle.phase = RpgPhase::LexieAction;
                battle.phase_timer = 0.0;
                battle.phase_duration = 2.0;
                battle.hit_applied = false;
                battle.vfx_stage = 0;
                spawn_lexie_attack_vfx(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    battle.lexie_attack_kind,
                    battle.lexie_target,
                );
            }
        }
        RpgPhase::LexieAction => {
            if !battle.hit_applied && battle.phase_timer >= 0.95 {
                battle.hit_applied = true;
                apply_lexie_damage(&mut commands, &mut battle);
            }
            if battle.phase_timer >= battle.phase_duration {
                battle.lexie_stun_immune = battle.lexie_stun_immune.saturating_sub(1);
                if battle.party.iter().all(|m| m.hp <= 0) {
                    battle.phase = RpgPhase::DefeatDelay;
                    battle.phase_timer = 0.0;
                    battle.phase_duration = 1.8;
                    battle.banner_msg = "THE SQUAD FELL TO LEXIE'S GARLIC...".to_string();
                    play_sfx("lose");
                } else {
                    battle.turn_round += 1;
                    if let Some(first) = find_next_alive_actor(&battle.party, None) {
                        start_actor_turn(&mut battle, first);
                    }
                }
            }
        }
        RpgPhase::VictoryDelay => {
            if phase_finished_with_voice(&battle, 6.0) {
                stats.score += 15000;
                stats.add_tendy_wendies(100);
                stats.lexie_defeated = true;
                stats.game_beaten = true;
                stats.boss_hp = 0.0;
                crate::ui::save_game_checkpoint("city", &stats);
                next_state.set(GameState::LexieVictoryCutscene);
            }
        }
        RpgPhase::DefeatDelay => {
            if battle.phase_timer >= battle.phase_duration {
                stats.hearts = 0;
                next_state.set(GameState::GameOver);
            }
        }
    }
    if battle.phase == RpgPhase::PlayerSelect {
        sync_turn_ownership(&mut battle, net.as_deref());
    }
}

/// Returns `true` once the phase's animation duration has elapsed and any active TTS voice
/// line has finished speaking (bounded by `max_extra_voice_wait` so a stalled speech engine
/// can never soft-lock the battle).
fn phase_finished_with_voice(battle: &RpgBattleState, max_extra_voice_wait: f32) -> bool {
    if battle.phase_timer < battle.phase_duration {
        return false;
    }
    if !is_voice_speaking() {
        return true;
    }
    battle.phase_timer >= battle.phase_duration + max_extra_voice_wait
}

fn start_actor_turn(battle: &mut RpgBattleState, actor: usize) {
    battle.actor = actor;
    battle.selected_btn = 0;
    battle.phase_timer = 0.0;
    battle.local_vote = None;
    battle.remote_votes.clear();
    if battle.party[actor].asleep_turns > 0 {
        battle.phase = RpgPhase::HeroAsleepSkip;
        battle.phase_duration = 1.65;
        battle.banner_msg = format!(
            "{} IS FAST ASLEEP! (4-HOUR NAP... Zzz)",
            battle.party[actor].name
        );
        play_sfx("sleep_snore");
    } else {
        battle.phase = RpgPhase::PlayerSelect;
        battle.phase_duration = 0.0;
        battle.banner_msg = format!(
            "ROUND {} - {}'S TURN! CHOOSE LIGHT OR HEAVY!",
            battle.turn_round, battle.party[actor].name
        );
    }
}

fn advance_after_hero(battle: &mut RpgBattleState) {
    if let Some(next_idx) = find_next_alive_actor(&battle.party, Some(battle.actor)) {
        start_actor_turn(battle, next_idx);
    } else {
        // Lexie's turn!
        battle.phase = RpgPhase::LexieWindup;
        battle.phase_timer = 0.0;
        if battle.lexie_stunned > 0 {
            battle.phase_duration = 1.9;
            battle.banner_msg = "LEXIE IS STUNNED! SHE CAN'T MOVE THIS TURN!".to_string();
            play_sfx("stun_bell");
            speak_voice("Ugh! My head! Everything is spinning!", "lexie");
        } else {
            battle.phase_duration = 1.1;
            let alive: Vec<usize> = battle
                .party
                .iter()
                .enumerate()
                .filter_map(|(i, m)| if m.hp > 0 { Some(i) } else { None })
                .collect();
            let pick_i = ((battle.turn_round as usize) * 3 + battle.actor) % alive.len().max(1);
            let target = alive.get(pick_i).copied().unwrap_or(0);
            let kind = (battle.turn_round as u8) % 3;
            battle.lexie_attack_kind = kind;
            battle.lexie_target = target;
            battle.banner_msg = match kind {
                0 => format!("LEXIE HURLS PICKLED GARLIC CLOVES AT {}!", battle.party[target].name),
                1 => "LEXIE UNLEASHES PICKLED STENCH BREATH ON EVERYONE!".to_string(),
                _ => format!("LEXIE WINDS UP A KAIJU STOMP ON {}!", battle.party[target].name),
            };
            play_sfx("garlic_throw");
        }
    }
}

fn begin_hero_action(battle: &mut RpgBattleState, is_heavy: bool) {
    battle.phase = RpgPhase::HeroAction;
    battle.is_heavy = is_heavy;
    battle.phase_timer = 0.0;
    battle.hit_applied = false;
    battle.vfx_stage = 0;

    let (dur, msg, voice_line, voice_role) = match (battle.actor, is_heavy) {
        (0, false) => (
            2.5,
            "STAN: CLICKING RESTORE SNAPSHOT IN ESXi!",
            Some("Restoring clean snapshot in ESXi!"),
            "stan",
        ),
        (0, true) => (
            3.7,
            "STAN: THE CYCLE! SONIC SPIN-DASH -> 4-HOUR NAP!",
            Some("Initiating the cycle! See you in four hours!"),
            "stan",
        ),
        (1, false) => (
            2.2,
            "GLENN: TRIPLE POWER PUNCH COMBO!",
            Some("Eat these knuckles, Lexie!"),
            "glenn",
        ),
        (1, true) => (
            2.9,
            "GLENN: TITAN POWERBOMB SLAM!!",
            Some("Going up! Powerbomb!"),
            "glenn",
        ),
        (2, false) => (
            2.5,
            "NATE: LAZY EYE HYPNOTIC STUN!!",
            Some("Look into my crooked eye!"),
            "nate",
        ),
        (2, true) => (
            2.6,
            "NATE: GIANT GUM BUBBLE POP!!",
            Some("Watch this bubble pop!"),
            "nate",
        ),
        (3, false) => (
            2.3,
            "DEREK: ELBOW SWEAT SPLASH ATTACK!!",
            Some("Have some elbow sweat!"),
            "derek",
        ),
        (3, true) => (
            3.2,
            "DEREK: KA... ME... HA... ME... HAAA!!",
            Some("Kamehameha!"),
            "derek",
        ),
        (_, false) => (
            2.5,
            "TJ: MY ANKLE IS BROKEN!! (RELENTLESS COMPLAINING STUN!)",
            Some("Seriously guys my ankle is broken!"),
            "tj",
        ),
        (_, true) => (
            5.2,
            "TJ: LEND ME YOUR ENERGY! GIVE ME ALL THE WORLD'S GOLDEN TENDIES!!",
            Some("Cedar Rapids! Lend me your energy! Give me all the world's golden tendies!"),
            "tj",
        ),
    };
    battle.phase_duration = dur;
    battle.banner_msg = msg.to_string();
    if let Some(line) = voice_line {
        speak_voice(line, voice_role);
    }
}

fn spawn_vfx_orb(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    vel: Vec3,
    radius: f32,
    color: Color,
    emissive: LinearRgba,
    lifetime: f32,
    grow_rate: f32,
) {
    commands.spawn((
        LexieBattleEntity,
        RpgVfx {
            age: 0.0,
            lifetime,
            vel,
            spin: 6.0,
            grow_rate,
        },
        Mesh3d(meshes.add(Sphere::new(radius))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: color,
            emissive,
            ..default()
        })),
        Transform::from_translation(pos),
    ));
}

fn spawn_vfx_ring(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    radius: f32,
    color: Color,
    emissive: LinearRgba,
    lifetime: f32,
    grow_rate: f32,
    tilt_x: f32,
) {
    commands.spawn((
        LexieBattleEntity,
        RpgVfx {
            age: 0.0,
            lifetime,
            vel: Vec3::ZERO,
            spin: 4.0,
            grow_rate,
        },
        Mesh3d(meshes.add(Torus::new((radius - 0.14).max(0.1), radius + 0.14))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: color,
            emissive,
            ..default()
        })),
        Transform::from_translation(pos).with_rotation(Quat::from_rotation_x(tilt_x)),
    ));
}

fn run_hero_action_vfx(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    battle: &mut RpgBattleState,
    stats: &mut PlayerStats,
) {
    let t = battle.phase_timer;
    let slot = HERO_SLOTS[battle.actor];
    let hero_pos = BATTLE_STAGE + Vec3::new(slot.0, 2.8, slot.1);
    let lexie_hit_pos = BATTLE_STAGE + LEXIE_HOME + Vec3::new(0.0, 9.5, 2.5);

    match (battle.actor, battle.is_heavy) {
        // Stan Light: ESXi Restore
        (0, false) => {
            if battle.vfx_stage == 0 {
                battle.vfx_stage = 1;
                // Holographic ESXi window panel in front of Stan
                commands.spawn((
                    LexieBattleEntity,
                    RpgVfx {
                        age: 0.0,
                        lifetime: 1.5,
                        vel: Vec3::ZERO,
                        spin: 0.0,
                        grow_rate: 0.2,
                    },
                    Mesh3d(meshes.add(Cuboid::new(3.8, 2.4, 0.14))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: Color::srgba(0.08, 0.35, 0.65, 0.9),
                        emissive: LinearRgba::rgb(0.2, 0.9, 1.8),
                        ..default()
                    })),
                    Transform::from_translation(hero_pos + Vec3::new(0.0, 1.2, -2.5)),
                ));
            }
            if battle.vfx_stage == 1 && t >= 0.85 {
                battle.vfx_stage = 2;
                play_sfx("esxi_click");
                let dir = (lexie_hit_pos - hero_pos).normalize_or_zero();
                spawn_vfx_orb(
                    commands,
                    meshes,
                    materials,
                    hero_pos + Vec3::new(0.0, 1.0, -2.0),
                    dir * 42.0,
                    0.9,
                    Color::srgb(0.2, 1.0, 0.65),
                    LinearRgba::rgb(0.4, 2.6, 1.4),
                    0.75,
                    0.5,
                );
            }
            if !battle.hit_applied && t >= 1.45 {
                battle.hit_applied = true;
                damage_lexie(commands, battle, stats, 42.0, false);
                for i in 0..5 {
                    if battle.party[i].hp > 0 {
                        battle.party[i].hp = (battle.party[i].hp + 12).min(battle.party[i].max_hp);
                    }
                }
                spawn_floating_popup(commands, "+12 PARTY HP!", Color::srgb(0.35, 1.0, 0.55), 35.0, 52.0);
            }
        }
        // Stan Heavy: The Cycle (Sonic spin -> Rocket dash -> 4h nap)
        (0, true) => {
            if battle.vfx_stage == 0 {
                battle.vfx_stage = 1;
                play_sfx("sonic_spin");
                spawn_vfx_ring(
                    commands,
                    meshes,
                    materials,
                    hero_pos - Vec3::Y * 1.4,
                    2.0,
                    Color::srgb(0.2, 0.9, 1.0),
                    LinearRgba::rgb(0.4, 2.4, 3.0),
                    1.0,
                    1.6,
                    PI * 0.5,
                );
            }
            if !battle.hit_applied && t >= 1.65 {
                battle.hit_applied = true;
                play_sfx("powerbomb");
                spawn_vfx_ring(
                    commands,
                    meshes,
                    materials,
                    lexie_hit_pos,
                    2.2,
                    Color::srgb(0.25, 0.95, 1.0),
                    LinearRgba::rgb(0.5, 2.5, 3.2),
                    0.8,
                    7.0,
                    0.0,
                );
                damage_lexie(commands, battle, stats, 118.0, false);
            }
            if battle.vfx_stage == 1 && t >= 2.4 {
                battle.vfx_stage = 2;
                battle.party[0].asleep_turns = 1;
                play_sfx("sleep_snore");
                spawn_floating_popup(commands, "Zzz... (4 HOURS)", Color::srgb(0.75, 0.85, 1.0), 14.0, 50.0);
            }
        }
        // Glenn Light: Power Punch (3-hit)
        (1, false) => {
            if battle.vfx_stage == 0 && t >= 0.55 {
                battle.vfx_stage = 1;
                play_sfx("hit");
                damage_lexie(commands, battle, stats, 18.0, false);
            } else if battle.vfx_stage == 1 && t >= 0.95 {
                battle.vfx_stage = 2;
                play_sfx("hit");
                damage_lexie(commands, battle, stats, 18.0, false);
            } else if !battle.hit_applied && t >= 1.35 {
                battle.hit_applied = true;
                play_sfx("lunge");
                spawn_vfx_ring(
                    commands,
                    meshes,
                    materials,
                    lexie_hit_pos,
                    1.8,
                    Color::srgb(1.0, 0.5, 0.15),
                    LinearRgba::rgb(2.6, 1.0, 0.2),
                    0.6,
                    6.0,
                    0.0,
                );
                damage_lexie(commands, battle, stats, 22.0, false);
            }
        }
        // Glenn Heavy: Powerbomb
        (1, true) => {
            if !battle.hit_applied && t >= 1.72 {
                battle.hit_applied = true;
                play_sfx("powerbomb");
                spawn_vfx_ring(
                    commands,
                    meshes,
                    materials,
                    BATTLE_STAGE + LEXIE_HOME + Vec3::Y * 0.4,
                    3.0,
                    Color::srgb(1.0, 0.65, 0.15),
                    LinearRgba::rgb(2.8, 1.4, 0.25),
                    0.9,
                    14.0,
                    PI * 0.5,
                );
                damage_lexie(commands, battle, stats, 108.0, false);
            }
        }
        // Nate Light: Lazy Eye Stun
        (2, false) => {
            if battle.vfx_stage == 0 && t >= 0.8 {
                battle.vfx_stage = 1;
                play_sfx("stun_bell");
                let dir = (lexie_hit_pos - hero_pos).normalize_or_zero();
                for sx in [-0.35_f32, 0.35] {
                    spawn_vfx_orb(
                        commands,
                        meshes,
                        materials,
                        hero_pos + Vec3::new(sx, 1.4, -0.5),
                        dir * 38.0 + Vec3::new(sx * 3.0, 0.0, 0.0),
                        0.55,
                        Color::srgb(0.85, 0.35, 1.0),
                        LinearRgba::rgb(2.2, 0.6, 3.0),
                        0.9,
                        0.8,
                    );
                }
            }
            if !battle.hit_applied && t >= 1.55 {
                battle.hit_applied = true;
                damage_lexie(commands, battle, stats, 32.0, true);
            }
        }
        // Nate Heavy: Gum Bubble Pop
        (2, true) => {
            if battle.vfx_stage == 0 && t >= 0.25 {
                battle.vfx_stage = 1;
                spawn_vfx_orb(
                    commands,
                    meshes,
                    materials,
                    hero_pos + Vec3::new(0.0, 1.4, -1.6),
                    Vec3::ZERO,
                    0.45,
                    Color::srgb(1.0, 0.45, 0.78),
                    LinearRgba::rgb(1.8, 0.5, 1.2),
                    1.25,
                    2.6,
                );
            }
            if !battle.hit_applied && t >= 1.55 {
                battle.hit_applied = true;
                play_sfx("bubble_pop");
                for k in 0..6 {
                    let ang = (k as f32) * PI / 3.0;
                    spawn_vfx_orb(
                        commands,
                        meshes,
                        materials,
                        lexie_hit_pos,
                        Vec3::new(ang.cos() * 14.0, 6.0, ang.sin() * 14.0),
                        0.9,
                        Color::srgb(1.0, 0.45, 0.80),
                        LinearRgba::rgb(2.2, 0.6, 1.5),
                        0.7,
                        0.0,
                    );
                }
                damage_lexie(commands, battle, stats, 90.0, false);
            }
        }
        // Derek Light: Elbow Sweat
        (3, false) => {
            if battle.vfx_stage == 0 && t >= 0.6 {
                battle.vfx_stage = 1;
                play_sfx("lunge");
                let dir = (lexie_hit_pos - hero_pos).normalize_or_zero();
                for k in 0..8 {
                    let spread = (k as f32 - 3.5) * 1.4;
                    spawn_vfx_orb(
                        commands,
                        meshes,
                        materials,
                        hero_pos + Vec3::new(0.4, 0.6, -0.8),
                        dir * 34.0 + Vec3::new(spread, (k % 3) as f32 * 2.5, 0.0),
                        0.42,
                        Color::srgb(0.45, 0.92, 1.0),
                        LinearRgba::rgb(0.6, 1.9, 2.5),
                        0.9,
                        0.2,
                    );
                }
            }
            if !battle.hit_applied && t >= 1.35 {
                battle.hit_applied = true;
                damage_lexie(commands, battle, stats, 52.0, false);
            }
        }
        // Derek Heavy: Kamehameha
        (3, true) => {
            if battle.vfx_stage == 0 && t >= 0.2 {
                battle.vfx_stage = 1;
                spawn_vfx_orb(
                    commands,
                    meshes,
                    materials,
                    hero_pos + Vec3::new(-0.8, 0.5, -0.3),
                    Vec3::ZERO,
                    0.45,
                    Color::srgb(0.25, 0.88, 1.0),
                    LinearRgba::rgb(0.8, 2.4, 3.6),
                    1.05,
                    1.6,
                );
            }
            if battle.vfx_stage == 1 && t >= 1.25 {
                battle.vfx_stage = 2;
                play_sfx("kamehameha");
                let mid = (hero_pos + lexie_hit_pos) * 0.5;
                let dist = (lexie_hit_pos - hero_pos).length();
                commands.spawn((
                    LexieBattleEntity,
                    RpgVfx {
                        age: 0.0,
                        lifetime: 1.2,
                        vel: Vec3::ZERO,
                        spin: 0.0,
                        grow_rate: 0.4,
                    },
                    Mesh3d(meshes.add(Cylinder::new(1.5, dist))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: Color::srgb(0.45, 0.96, 1.0),
                        emissive: LinearRgba::rgb(1.4, 3.2, 4.5),
                        ..default()
                    })),
                    Transform::from_translation(mid)
                        .looking_at(lexie_hit_pos, Vec3::Y)
                        * Transform::from_rotation(Quat::from_rotation_x(PI * 0.5)),
                ));
            }
            if !battle.hit_applied && t >= 1.8 {
                battle.hit_applied = true;
                damage_lexie(commands, battle, stats, 114.0, false);
            }
        }
        // TJ Light: Broken Ankle Complain (Stun)
        (_, false) => {
            if battle.vfx_stage == 0 && t >= 0.55 {
                battle.vfx_stage = 1;
                play_sfx("stun_bell");
                let dir = (lexie_hit_pos - hero_pos).normalize_or_zero();
                for k in 0..3 {
                    commands.spawn((
                        LexieBattleEntity,
                        RpgVfx {
                            age: 0.0,
                            lifetime: 1.1,
                            vel: dir * (22.0 + k as f32 * 5.0),
                            spin: 5.0,
                            grow_rate: 2.5,
                        },
                        Mesh3d(meshes.add(Torus::new(1.25, 1.50))),
                        MeshMaterial3d(materials.add(StandardMaterial {
                            base_color: Color::srgb(1.0, 0.42, 0.20),
                            emissive: LinearRgba::rgb(2.8, 0.9, 0.25),
                            ..default()
                        })),
                        Transform::from_translation(hero_pos + Vec3::Y * 0.8),
                    ));
                }
            }
            if !battle.hit_applied && t >= 1.5 {
                battle.hit_applied = true;
                damage_lexie(commands, battle, stats, 28.0, true);
            }
        }
        // TJ Heavy: Epic Tendy Spirit Bomb
        (_, true) => {
            let sb_origin = hero_pos + Vec3::new(0.0, 12.5, -1.5);
            if battle.vfx_stage == 0 {
                battle.vfx_stage = 1;
                play_sfx("spirit_bomb");
                // Colossal growing Golden Spirit Bomb above TJ
                spawn_vfx_orb(
                    commands,
                    meshes,
                    materials,
                    sb_origin,
                    Vec3::ZERO,
                    1.2,
                    Color::srgb(1.0, 0.88, 0.28),
                    LinearRgba::rgb(3.2, 2.4, 0.45),
                    3.4,
                    1.6,
                );
                // Tendy energy orbs flying in from every party member & Cedar Rapids skyline
                for (i, slot_i) in HERO_SLOTS.iter().enumerate() {
                    let start = BATTLE_STAGE + Vec3::new(slot_i.0, 3.0, slot_i.1);
                    let to_sb = (sb_origin - start).normalize_or_zero();
                    let dist = (sb_origin - start).length();
                    spawn_vfx_orb(
                        commands,
                        meshes,
                        materials,
                        start,
                        to_sb * (dist / 1.4),
                        0.55,
                        hero_accent_color(i),
                        LinearRgba::rgb(2.6, 2.2, 0.6),
                        1.45,
                        0.0,
                    );
                }
            }
            if battle.vfx_stage == 1 && t >= 1.6 {
                battle.vfx_stage = 2;
                // Second wave of golden tendy orbs from the Cedar Rapids skyline!
                for k in 0..8 {
                    let ang = (k as f32) * PI * 0.25;
                    let start = BATTLE_STAGE + Vec3::new(ang.cos() * 24.0, 18.0, ang.sin() * 20.0);
                    let to_sb = (sb_origin - start).normalize_or_zero();
                    let dist = (sb_origin - start).length();
                    spawn_vfx_orb(
                        commands,
                        meshes,
                        materials,
                        start,
                        to_sb * (dist / 1.5),
                        0.65,
                        Color::srgb(1.0, 0.86, 0.25),
                        LinearRgba::rgb(3.0, 2.2, 0.35),
                        1.5,
                        0.0,
                    );
                }
            }
            if battle.vfx_stage == 2 && t >= 3.4 {
                battle.vfx_stage = 3;
                play_sfx("spirit_bomb");
                let to_lx = (lexie_hit_pos - sb_origin).normalize_or_zero();
                let dist = (lexie_hit_pos - sb_origin).length();
                spawn_vfx_orb(
                    commands,
                    meshes,
                    materials,
                    sb_origin,
                    to_lx * (dist / 0.85),
                    5.4,
                    Color::srgb(1.0, 0.92, 0.42),
                    LinearRgba::rgb(4.2, 3.4, 0.9),
                    0.95,
                    0.0,
                );
            }
            if !battle.hit_applied && t >= 4.25 {
                battle.hit_applied = true;
                play_sfx("powerbomb");
                spawn_vfx_ring(
                    commands,
                    meshes,
                    materials,
                    lexie_hit_pos,
                    4.0,
                    Color::srgb(1.0, 0.92, 0.35),
                    LinearRgba::rgb(4.0, 3.0, 0.6),
                    0.95,
                    18.0,
                    PI * 0.5,
                );
                damage_lexie(commands, battle, stats, 165.0, false);
            }
        }
    }
}

fn damage_lexie(
    commands: &mut Commands,
    battle: &mut RpgBattleState,
    stats: &mut PlayerStats,
    dmg: f32,
    apply_stun: bool,
) {
    battle.lexie_hp = (battle.lexie_hp - dmg).max(0.0);
    battle.lexie_hit_flash = 0.65;
    stats.boss_hp = battle.lexie_hp;
    play_sfx("saber_hit");
    spawn_floating_popup(
        commands,
        format!("-{:.0}!", dmg),
        Color::srgb(1.0, 0.86, 0.22),
        47.0,
        24.0,
    );
    if apply_stun {
        if battle.lexie_stunned == 0 && battle.lexie_stun_immune == 0 {
            battle.lexie_stunned = 1;
            battle.lexie_stun_immune = 1;
            spawn_floating_popup(
                commands,
                "STUNNED!",
                Color::srgb(0.92, 0.50, 1.0),
                45.0,
                18.0,
            );
        } else {
            spawn_floating_popup(
                commands,
                "STUN RESISTED!",
                Color::srgb(0.85, 0.65, 0.95),
                45.0,
                18.0,
            );
        }
    }
}

fn spawn_lexie_attack_vfx(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    kind: u8,
    target: usize,
) {
    let origin = BATTLE_STAGE + LEXIE_HOME + Vec3::new(0.0, 14.0, 3.0);
    let t_slot = HERO_SLOTS[target];
    let dst = BATTLE_STAGE + Vec3::new(t_slot.0, 2.2, t_slot.1);
    let dir = (dst - origin).normalize_or_zero();
    let dist = (dst - origin).length();

    match kind {
        0 => {
            for k in 0..3 {
                spawn_vfx_orb(
                    commands,
                    meshes,
                    materials,
                    origin + Vec3::new((k as f32 - 1.0) * 1.8, 0.0, 0.0),
                    dir * (dist / 0.95),
                    1.1,
                    Color::srgb(0.95, 0.94, 0.62),
                    LinearRgba::rgb(0.9, 1.2, 0.2),
                    1.0,
                    0.0,
                );
            }
        }
        1 => {
            spawn_vfx_ring(
                commands,
                meshes,
                materials,
                BATTLE_STAGE + LEXIE_HOME + Vec3::Y * 4.5,
                2.5,
                Color::srgb(0.55, 0.95, 0.22),
                LinearRgba::rgb(1.0, 2.2, 0.25),
                1.1,
                22.0,
                PI * 0.5,
            );
        }
        _ => {
            spawn_vfx_ring(
                commands,
                meshes,
                materials,
                dst - Vec3::Y * 1.8,
                1.8,
                Color::srgb(1.0, 0.32, 0.45),
                LinearRgba::rgb(2.6, 0.4, 0.7),
                1.0,
                7.5,
                PI * 0.5,
            );
        }
    }
}

fn apply_lexie_damage(commands: &mut Commands, battle: &mut RpgBattleState) {
    play_sfx("hit");
    let enrage: f32 = if battle.lexie_hp < battle.lexie_max_hp * 0.5 {
        1.25
    } else {
        1.0
    };
    let dmg_mul = lexie_coop_damage_mul(battle.coop_players) * enrage;
    match battle.lexie_attack_kind {
        0 => {
            let dmg = (52.0 * dmg_mul).round() as i32;
            let t = battle.lexie_target;
            battle.party[t].hp = (battle.party[t].hp - dmg).max(0);
            battle.party[t].hit_flash = 0.6;
            battle.party[t].hit_shake = 0.6;
            let (sx, sy) = hero_screen_pos(t);
            spawn_floating_popup(commands, format!("-{}", dmg), Color::srgb(1.0, 0.32, 0.32), sx, sy);
        }
        1 => {
            let dmg = (34.0 * dmg_mul).round() as i32;
            for i in 0..5 {
                if battle.party[i].hp > 0 {
                    battle.party[i].hp = (battle.party[i].hp - dmg).max(0);
                    battle.party[i].hit_flash = 0.55;
                    battle.party[i].hit_shake = 0.55;
                    let (sx, sy) = hero_screen_pos(i);
                    spawn_floating_popup(
                        commands,
                        format!("-{}", dmg),
                        Color::srgb(0.65, 1.0, 0.30),
                        sx,
                        sy,
                    );
                }
            }
        }
        _ => {
            let dmg = (68.0 * dmg_mul).round() as i32;
            let t = battle.lexie_target;
            battle.party[t].hp = (battle.party[t].hp - dmg).max(0);
            battle.party[t].hit_flash = 0.75;
            battle.party[t].hit_shake = 0.75;
            let (sx, sy) = hero_screen_pos(t);
            spawn_floating_popup(commands, format!("-{}!", dmg), Color::srgb(1.0, 0.25, 0.35), sx, sy);
        }
    }
}

#[derive(Component)]
pub struct StanNapBlankie;

#[derive(Component)]
pub struct StanNapZzz(pub f32);

pub fn animate_lexie_battle_stage(
    time: Res<Time>,
    mut commands: Commands,
    battle: Res<RpgBattleState>,
    mut qs: ParamSet<(
        Query<(&RpgHero, &mut Transform)>,
        Query<&mut Transform, With<RpgTurnRing>>,
        Query<&mut Transform, With<RpgLexieRoot>>,
        Query<(&RpgLexieArm, &mut Transform)>,
        Query<&mut Transform, With<RpgLexieHead>>,
        Query<(&RpgStunStar, &mut Transform, &mut Visibility)>,
        Query<(Entity, &mut RpgVfx, &mut Transform)>,
        Query<&mut Transform, With<Camera3d>>,
    )>,
    mut blankie_q: Query<
        (&mut Transform, &mut Visibility),
        (
            With<StanNapBlankie>,
            Without<RpgHero>,
            Without<RpgTurnRing>,
            Without<RpgLexieRoot>,
            Without<RpgLexieArm>,
            Without<RpgLexieHead>,
            Without<RpgStunStar>,
            Without<RpgVfx>,
            Without<Camera3d>,
            Without<StanNapZzz>,
        ),
    >,
    mut zzz_q: Query<
        (&StanNapZzz, &mut Transform),
        (
            Without<StanNapBlankie>,
            Without<RpgHero>,
            Without<RpgTurnRing>,
            Without<RpgLexieRoot>,
            Without<RpgLexieArm>,
            Without<RpgLexieHead>,
            Without<RpgStunStar>,
            Without<RpgVfx>,
            Without<Camera3d>,
        ),
    >,
) {
    let dt = time.delta_secs().min(0.15);
    let t = time.elapsed_secs();

    let stan_napping = battle.party[0].hp > 0
        && (battle.party[0].asleep_turns > 0
            || (battle.phase == RpgPhase::HeroAction && battle.actor == 0 && battle.is_heavy && battle.phase_timer >= 2.25));
    for (mut b_tf, mut b_vis) in &mut blankie_q {
        let want = if stan_napping { Visibility::Visible } else { Visibility::Hidden };
        if *b_vis != want {
            *b_vis = want;
        }
        if stan_napping {
            let breath = 1.0 + (t * 2.2).sin() * 0.04;
            b_tf.scale = Vec3::new(1.0, breath, 1.0);
        }
    }
    if stan_napping {
        for (z, mut z_tf) in &mut zzz_q {
            let cyc = (t * 0.75 + z.0).rem_euclid(1.0);
            z_tf.translation = Vec3::new(-3.65 + (t * 2.0 + z.0).sin() * 0.25, 1.2 + cyc * 1.9, 0.3);
            z_tf.scale = Vec3::splat((1.0 - cyc * 0.55).max(0.25));
        }
    }

    // Update turn ring position
    if let Ok(mut tr) = qs.p1().get_single_mut() {
        let s = HERO_SLOTS[battle.actor.min(4)];
        let target = BATTLE_STAGE + Vec3::new(s.0, 0.08, s.1);
        tr.translation = tr.translation.lerp(target, (dt * 12.0).min(1.0));
        tr.rotate_y(dt * 2.2);
    }

    // Animate all 5 heroes: RPG idle dance, attack motion, asleep nap, and damage recoil
    for (hero, mut h_tf) in &mut qs.p0() {
        let m = &battle.party[hero.idx];
        let mut pos = hero.home_pos;
        let mut yaw = hero.home_yaw;
        let mut pitch = 0.0_f32;
        let mut roll = 0.0_f32;

        if m.hp <= 0 {
            pitch = -PI * 0.48;
            pos.y = 0.35;
        } else if m.asleep_turns > 0
            && !(battle.phase == RpgPhase::HeroAction && battle.actor == hero.idx && battle.phase_timer < 2.3)
        {
            // Stan's 4-hour nap pose
            roll = PI * 0.48;
            pos.y = 0.55 + (t * 2.2).sin() * 0.05;
        } else if battle.phase == RpgPhase::HeroAction && battle.actor == hero.idx {
            let at = battle.phase_timer;
            match (hero.idx, battle.is_heavy) {
                (0, true) => {
                    // Stan's Cycle: spin -> rocket forward -> nap
                    if at < 1.1 {
                        yaw += at * 38.0;
                        pos.y += 0.3 + (t * 20.0).sin().abs() * 0.25;
                    } else if at < 1.7 {
                        let u = (at - 1.1) / 0.6;
                        pos = hero.home_pos.lerp(BATTLE_STAGE + LEXIE_HOME + Vec3::Z * 4.0, u);
                        yaw += at * 38.0;
                    } else if at < 2.3 {
                        let u = (at - 1.7) / 0.6;
                        pos = (BATTLE_STAGE + LEXIE_HOME + Vec3::Z * 4.0).lerp(hero.home_pos, u);
                    } else {
                        roll = PI * 0.48;
                        pos.y = 0.55;
                    }
                }
                (1, false) => {
                    // Glenn Power Punch dash
                    if at < 0.45 {
                        pos = hero.home_pos.lerp(BATTLE_STAGE + LEXIE_HOME + Vec3::Z * 5.0, at / 0.45);
                    } else if at < 1.55 {
                        pos = BATTLE_STAGE + LEXIE_HOME + Vec3::Z * 5.0;
                        yaw += (at * 24.0).sin() * 0.45;
                    } else {
                        pos = (BATTLE_STAGE + LEXIE_HOME + Vec3::Z * 5.0)
                            .lerp(hero.home_pos, ((at - 1.55) / 0.65).min(1.0));
                    }
                }
                (1, true) => {
                    // Glenn Powerbomb leap & slam
                    if at < 0.7 {
                        let u = at / 0.7;
                        pos = hero.home_pos.lerp(BATTLE_STAGE + LEXIE_HOME + Vec3::Z * 4.5, u);
                        pos.y += (u * PI).sin() * 6.5;
                    } else if at < 1.85 {
                        pos = BATTLE_STAGE + LEXIE_HOME + Vec3::Z * 4.5;
                        if at < 1.45 {
                            pos.y += (at - 0.7) * 4.0;
                        }
                    } else {
                        pos = (BATTLE_STAGE + LEXIE_HOME + Vec3::Z * 4.5)
                            .lerp(hero.home_pos, ((at - 1.85) / 1.05).min(1.0));
                    }
                }
                (3, true) => {
                    // Derek Kamehameha stance
                    pos.y += 0.15;
                }
                (4, false) => {
                    // TJ Broken Ankle hop
                    pos.y += (at * 11.0).sin().abs() * 1.1;
                    roll = (at * 11.0).sin() * 0.22;
                }
                (4, true) => {
                    // TJ Spirit Bomb arms raised
                    pos.y += (at * 6.0).sin() * 0.15;
                }
                _ => {
                    pos.z -= (at * 4.0).sin().abs() * 1.4;
                }
            }
        } else if battle.phase == RpgPhase::HeroAction
            && battle.actor == 4
            && battle.is_heavy
            && battle.phase_timer < 3.4
        {
            // Everyone raises hands to lend TJ energy for the Spirit Bomb!
            pos.y += (t * 4.0 + hero.dance_phase).sin().abs() * 0.15;
        } else if battle.phase == RpgPhase::VictoryDelay {
            // Victory dance!
            pos.y += (t * 9.0 + hero.dance_phase).sin().abs() * 0.9;
        } else {
            // Classic RPG idle dancing/swaying motion
            let w = t * 5.2 + hero.dance_phase;
            pos.y += w.sin().abs() * 0.18;
            roll += (t * 2.6 + hero.dance_phase).sin() * 0.08;
            yaw += (t * 2.6 + hero.dance_phase).cos() * 0.07;
        }

        // Damage recoil reaction
        if m.hit_flash > 0.0 {
            let k = (m.hit_flash / 0.6).min(1.0);
            pos.z += k * 1.8;
            pos.x += (t * 58.0).sin() * k * 0.45;
            pitch -= k * 0.42;
        }

        h_tf.translation = pos;
        h_tf.rotation =
            Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch) * Quat::from_rotation_z(roll);
    }

    // Animate Lexie (idle sway, powerbomb lift, stun wobble, damage recoil)
    if let Ok(mut lx_tf) = qs.p2().get_single_mut() {
        let mut lpos = BATTLE_STAGE + LEXIE_HOME;
        let mut lyaw = 0.0_f32;
        let mut lpitch = 0.0_f32;
        let mut lroll = 0.0_f32;

        if battle.phase == RpgPhase::HeroAction && battle.actor == 1 && battle.is_heavy {
            let at = battle.phase_timer;
            if at >= 0.7 && at < 1.45 {
                lpos.y += (at - 0.7) * 9.0;
                lpitch = -0.35;
            } else if at >= 1.45 && at < 1.85 {
                let u = (at - 1.45) / 0.4;
                lpos.y += (1.0 - u) * 6.75;
                lpitch = 0.4;
            }
        } else if battle.lexie_stunned > 0 {
            lroll = (t * 6.0).sin() * 0.14;
            lyaw = (t * 4.5).cos() * 0.18;
        } else if battle.phase == RpgPhase::VictoryDelay {
            lpos.y -= (battle.phase_timer * 2.5).min(3.5);
            lroll = (t * 14.0).sin() * 0.18;
        } else {
            lpos.y += (t * 4.2).sin().abs() * 0.45;
            lroll = (t * 2.1).sin() * 0.06;
            lyaw = (t * 2.1).cos() * 0.08;
        }

        if battle.lexie_hit_flash > 0.0 {
            let k = (battle.lexie_hit_flash / 0.65).min(1.0);
            lpos.z -= k * 2.4;
            lpos.x += (t * 52.0).sin() * k * 0.7;
            lpitch -= k * 0.28;
        }

        lx_tf.translation = lpos;
        lx_tf.rotation =
            Quat::from_rotation_y(lyaw) * Quat::from_rotation_x(lpitch) * Quat::from_rotation_z(lroll);
    }

    for (arm, mut a_tf) in &mut qs.p3() {
        let sign = if arm.is_left { -1.0 } else { 1.0 };
        let base = if battle.lexie_hit_flash > 0.0 {
            -2.1
        } else if battle.phase == RpgPhase::LexieAction || battle.phase == RpgPhase::LexieWindup {
            -1.45 + (t * 10.0).sin() * 0.35
        } else {
            -0.35 + (t * 4.2 + sign).sin() * 0.25
        };
        a_tf.rotation = Quat::from_rotation_x(base) * Quat::from_rotation_z(sign * 0.22);
    }

    if let Ok(mut hd_tf) = qs.p4().get_single_mut() {
        if battle.lexie_stunned > 0 {
            hd_tf.rotation = Quat::from_rotation_z((t * 8.0).sin() * 0.22);
        } else {
            hd_tf.rotation = Quat::from_rotation_y((t * 2.4).sin() * 0.12);
        }
    }

    for (star, mut s_tf, mut vis) in &mut qs.p5() {
        if battle.lexie_stunned > 0 {
            *vis = Visibility::Visible;
            let ang = t * 4.0 + (star.index as f32) * (PI * 2.0 / 3.0);
            s_tf.translation = Vec3::new(ang.cos() * 0.95, 4.95, ang.sin() * 0.95);
            s_tf.rotate_y(dt * 4.0);
        } else {
            *vis = Visibility::Hidden;
        }
    }

    // Update ephemeral VFX entities
    for (e, mut vfx, mut tf) in &mut qs.p6() {
        vfx.age += dt;
        if vfx.age >= vfx.lifetime {
            commands.entity(e).despawn_recursive();
            continue;
        }
        tf.translation += vfx.vel * dt;
        tf.rotate_y(vfx.spin * dt);
        if vfx.grow_rate != 0.0 {
            tf.scale += Vec3::splat(vfx.grow_rate * dt);
        }
    }

    // Dynamic cinematic RPG camera
    if let Ok(mut c_tf) = qs.p7().get_single_mut() {
        let shake = if battle.lexie_hit_flash > 0.2 {
            Vec3::new((t * 60.0).sin() * 0.35, (t * 48.0).cos() * 0.28, 0.0)
        } else {
            Vec3::ZERO
        };
        let (target_pos, target_look) = match battle.phase {
            RpgPhase::HeroAction => {
                let s = HERO_SLOTS[battle.actor];
                if battle.actor == 4 && battle.is_heavy {
                    // Epic low-angle Spirit Bomb camera behind TJ looking up & toward Lexie!
                    (
                        BATTLE_STAGE + Vec3::new(s.0 + 5.5, 4.0, s.1 + 7.0),
                        BATTLE_STAGE + Vec3::new(1.5, 11.0, -5.0),
                    )
                } else if battle.actor == 2 && !battle.is_heavy {
                    // Nate Lazy Eye close-up
                    (
                        BATTLE_STAGE + Vec3::new(s.0 + 1.8, 5.2, s.1 - 6.5),
                        BATTLE_STAGE + Vec3::new(s.0, 4.6, s.1),
                    )
                } else {
                    (
                        BATTLE_STAGE + Vec3::new(s.0 + 7.5, 7.8, s.1 + 11.0),
                        BATTLE_STAGE + Vec3::new(0.0, 6.0, -6.0),
                    )
                }
            }
            RpgPhase::LexieWindup | RpgPhase::LexieAction => (
                BATTLE_STAGE + Vec3::new(-9.5, 8.5, 16.5),
                BATTLE_STAGE + Vec3::new(0.0, 7.5, -6.0),
            ),
            _ => {
                let s = HERO_SLOTS[battle.actor.min(4)];
                (
                    BATTLE_STAGE + Vec3::new(s.0 * 0.45 + 14.5, 11.5, 29.0),
                    BATTLE_STAGE + Vec3::new(0.0, 9.5, -5.0),
                )
            }
        };
        c_tf.translation = c_tf.translation.lerp(target_pos + shake, (dt * 4.5).min(1.0));
        c_tf.look_at(target_look, Vec3::Y);
    }
}

pub fn animate_lexie_battle_limbs(
    time: Res<Time>,
    battle: Res<RpgBattleState>,
    hero_q: Query<(&RpgHero, &Children)>,
    children_q: Query<&Children>,
    mut stride_arm_q: Query<(&CrewStrideArm, &mut Transform)>,
    mut stride_leg_q: Query<(&CrewStrideLeg, &mut Transform), Without<CrewStrideArm>>,
    mut derek_arm_q: Query<
        (&DerekArmPivot, &mut Transform),
        (Without<CrewStrideLeg>, Without<CrewStrideArm>),
    >,
    mut nate_arm_q: Query<
        (&NateArmPivot, &mut Transform),
        (Without<DerekArmPivot>, Without<CrewStrideLeg>, Without<CrewStrideArm>),
    >,
    mut nate_leg_q: Query<
        (&NateLegPivot, &mut Transform),
        (
            Without<NateArmPivot>,
            Without<DerekArmPivot>,
            Without<CrewStrideLeg>,
            Without<CrewStrideArm>,
        ),
    >,
    mut tj_leg_q: Query<
        (&TjLegPivot, &mut Transform),
        (
            Without<NateLegPivot>,
            Without<NateArmPivot>,
            Without<DerekArmPivot>,
            Without<CrewStrideLeg>,
            Without<CrewStrideArm>,
        ),
    >,
) {
    let t = time.elapsed_secs();
    for (hero, children) in &hero_q {
        let m = &battle.party[hero.idx];
        let mut arm_raise = 0.0_f32;
        if m.hp > 0 && m.asleep_turns == 0 {
            if battle.phase == RpgPhase::HeroAction && battle.actor == hero.idx {
                arm_raise = match (hero.idx, battle.is_heavy) {
                    (1, false) => 1.4,
                    (1, true) => 2.2,
                    (3, true) => if battle.phase_timer < 1.25 { 0.8 } else { 1.65 },
                    (4, false) => 1.4,
                    (4, true) => 2.6,
                    _ => 1.3,
                };
            } else if battle.phase == RpgPhase::HeroAction
                && battle.actor == 4
                && battle.is_heavy
                && battle.phase_timer < 3.4
            {
                arm_raise = 2.5;
            } else if battle.phase == RpgPhase::VictoryDelay {
                arm_raise = 2.3;
            } else if m.hit_flash > 0.0 {
                arm_raise = 2.0 * (m.hit_flash / 0.6).min(1.0);
            }
        }
        animate_hero_limbs_recursive(
            children,
            &children_q,
            t,
            hero.dance_phase,
            arm_raise,
            m.hp <= 0 || m.asleep_turns > 0,
            &mut stride_arm_q,
            &mut stride_leg_q,
            &mut derek_arm_q,
            &mut nate_arm_q,
            &mut nate_leg_q,
            &mut tj_leg_q,
        );
    }
}

pub fn update_lexie_battle_ui(
    time: Res<Time>,
    mut commands: Commands,
    battle: Res<RpgBattleState>,
    mut node_qs: ParamSet<(
        Query<&mut Node, With<RpgBossHpFill>>,
        Query<(&RpgPartyHpFill, &mut Node)>,
        Query<(Entity, &mut RpgFloatingText, &mut Node)>,
    )>,
    mut txt_qs: ParamSet<(
        Query<&mut Text, With<RpgBossStatusText>>,
        Query<&mut Text, With<RpgBannerText>>,
        Query<(&RpgPartyHpText, &mut Text)>,
        Query<(&RpgPartyStateText, &mut Text)>,
        Query<&mut Text, With<RpgCmdHeader>>,
        Query<(&RpgCmdBtnText, &mut Text)>,
    )>,
    mut card_q: Query<(&RpgPartyCard, &mut BorderColor)>,
    mut cmd_panel_q: Query<&mut Visibility, With<RpgCmdPanel>>,
) {
    let dt = time.delta_secs().min(0.15);

    if let Ok(mut fill) = node_qs.p0().get_single_mut() {
        fill.width = Val::Percent((battle.lexie_hp / battle.lexie_max_hp * 100.0).clamp(0.0, 100.0));
    }
    if let Ok(mut txt) = txt_qs.p0().get_single_mut() {
        let coop_tag = if battle.coop_players > 1 {
            format!("  [{}P CO-OP]", battle.coop_players)
        } else {
            String::new()
        };
        let stun_tag = if battle.lexie_stunned > 0 {
            "  [STUNNED!]"
        } else if battle.lexie_hp < battle.lexie_max_hp * 0.5 {
            "  [ENRAGED!]"
        } else if battle.lexie_stun_immune > 0 {
            "  [STUN IMMUNE]"
        } else {
            ""
        };
        txt.0 = format!(
            "LEXIE - PICKLED GARLIC QUEEN   HP {:.0} / {:.0}{}{}",
            battle.lexie_hp, battle.lexie_max_hp, coop_tag, stun_tag
        );
    }
    if let Ok(mut btxt) = txt_qs.p1().get_single_mut() {
        btxt.0 = battle.banner_msg.clone();
    }
    for (card, mut border) in &mut card_q {
        let is_active =
            (battle.phase == RpgPhase::PlayerSelect || battle.phase == RpgPhase::HeroAction)
                && battle.actor == card.0;
        *border = BorderColor(if is_active {
            Color::srgb(1.0, 0.90, 0.25)
        } else {
            hero_accent_color(card.0)
        });
    }
    for (fill, mut node) in &mut node_qs.p1() {
        let m = &battle.party[fill.0];
        node.width = Val::Percent((m.hp as f32 / m.max_hp as f32 * 100.0).clamp(0.0, 100.0));
    }
    for (ht, mut txt) in &mut txt_qs.p2() {
        let m = &battle.party[ht.0];
        txt.0 = format!("{}/{} HP", m.hp, m.max_hp);
    }
    for (st, mut txt) in &mut txt_qs.p3() {
        let m = &battle.party[st.0];
        let is_you = battle.coop_online && actor_to_char_id(st.0) == battle.local_char_id;
        let is_active = (battle.phase == RpgPhase::PlayerSelect || battle.phase == RpgPhase::HeroAction)
            && battle.actor == st.0;
        txt.0 = if m.hp <= 0 {
            "KO".to_string()
        } else if m.asleep_turns > 0 {
            "ASLEEP (4h)".to_string()
        } else if is_active {
            if is_you { "YOU - ACTIVE!" } else { "ACTIVE!" }.to_string()
        } else if is_you {
            "YOU".to_string()
        } else {
            "READY".to_string()
        };
    }

    if let Ok(mut vis) = cmd_panel_q.get_single_mut() {
        *vis = if battle.phase == RpgPhase::PlayerSelect {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    if battle.phase == RpgPhase::PlayerSelect {
        let m = &battle.party[battle.actor];
        let light_v = usize::from(battle.local_vote == Some(false))
            + battle.remote_votes.iter().filter(|&&(_, h)| !h).count();
        let heavy_v = usize::from(battle.local_vote == Some(true))
            + battle.remote_votes.iter().filter(|&&(_, h)| h).count();
        if let Ok(mut hdr) = txt_qs.p4().get_single_mut() {
            hdr.0 = if battle.turn_owner_claimed {
                if battle.turn_local_owns {
                    format!(
                        "YOUR TURN ({})! [1 / 2] OR CLICK ({:.0}s)",
                        m.name,
                        battle.vote_time_left.ceil()
                    )
                } else {
                    format!(
                        "WAITING FOR {}'S PLAYER... ({:.0}s)",
                        m.name,
                        battle.vote_time_left.ceil()
                    )
                }
            } else if battle.vote_coop_active {
                format!(
                    "SQUAD VOTE FOR {} (UNCLAIMED)! ({}/{} VOTED - {:.0}s)",
                    m.name,
                    light_v + heavy_v,
                    battle.vote_total_players,
                    battle.vote_time_left.ceil()
                )
            } else {
                format!("{}'S TURN - PRESS [1 / 2] OR CLICK", m.name)
            };
        }
        for (bt, mut txt) in &mut txt_qs.p5() {
            txt.0 = if bt.0 == 0 {
                if battle.vote_coop_active {
                    format!("[1] LIGHT: {}  [{} VOTE{}]\n({})", m.light_name, light_v, if light_v == 1 { "" } else { "S" }, m.light_tag)
                } else {
                    format!("[1] LIGHT: {}\n({})", m.light_name, m.light_tag)
                }
            } else if battle.vote_coop_active {
                format!("[2] HEAVY: {}  [{} VOTE{}]\n({})", m.heavy_name, heavy_v, if heavy_v == 1 { "" } else { "S" }, m.heavy_tag)
            } else {
                format!("[2] HEAVY: {}\n({})", m.heavy_name, m.heavy_tag)
            };
        }
    }

    if crate::audio::debug_keys_enabled() {
        let phase_name = match battle.phase {
            RpgPhase::Intro => "Intro",
            RpgPhase::HeroAsleepSkip => "HeroAsleepSkip",
            RpgPhase::PlayerSelect => "PlayerSelect",
            RpgPhase::HeroAction => "HeroAction",
            RpgPhase::LexieWindup => "LexieWindup",
            RpgPhase::LexieAction => "LexieAction",
            RpgPhase::VictoryDelay => "VictoryDelay",
            RpgPhase::DefeatDelay => "DefeatDelay",
        };
        let tele = format!(
            "{{\"phase\":\"{}\",\"round\":{},\"actor\":{},\"coopPlayers\":{},\"lexieHp\":{:.0},\"lexieMaxHp\":{:.0},\"ownerClaimed\":{},\"localOwns\":{},\"voteCoop\":{},\"voteTotal\":{},\"partyHp\":[{},{},{},{},{}]}}",
            phase_name,
            battle.turn_round,
            battle.actor,
            battle.coop_players,
            battle.lexie_hp,
            battle.lexie_max_hp,
            battle.turn_owner_claimed,
            battle.turn_local_owns,
            battle.vote_coop_active,
            battle.vote_total_players,
            battle.party[0].hp,
            battle.party[1].hp,
            battle.party[2].hp,
            battle.party[3].hp,
            battle.party[4].hp,
        );
        crate::audio::set_lexie_telemetry(&tele);
    }

    for (e, mut ft, mut node) in &mut node_qs.p2() {
        ft.age += dt;
        if ft.age >= ft.lifetime {
            commands.entity(e).despawn_recursive();
            continue;
        }
        if let Val::Px(y) = node.top {
            node.top = Val::Px(y + ft.vy * dt);
        } else if let Val::Percent(p) = node.top {
            node.top = Val::Percent(p - dt * 6.5);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn animate_hero_limbs_recursive(
    children: &Children,
    children_q: &Query<&Children>,
    t: f32,
    phase: f32,
    arm_raise: f32,
    inert: bool,
    stride_arm_q: &mut Query<(&CrewStrideArm, &mut Transform)>,
    stride_leg_q: &mut Query<(&CrewStrideLeg, &mut Transform), Without<CrewStrideArm>>,
    derek_arm_q: &mut Query<
        (&DerekArmPivot, &mut Transform),
        (Without<CrewStrideLeg>, Without<CrewStrideArm>),
    >,
    nate_arm_q: &mut Query<
        (&NateArmPivot, &mut Transform),
        (Without<DerekArmPivot>, Without<CrewStrideLeg>, Without<CrewStrideArm>),
    >,
    nate_leg_q: &mut Query<
        (&NateLegPivot, &mut Transform),
        (
            Without<NateArmPivot>,
            Without<DerekArmPivot>,
            Without<CrewStrideLeg>,
            Without<CrewStrideArm>,
        ),
    >,
    tj_leg_q: &mut Query<
        (&TjLegPivot, &mut Transform),
        (
            Without<NateLegPivot>,
            Without<NateArmPivot>,
            Without<DerekArmPivot>,
            Without<CrewStrideLeg>,
            Without<CrewStrideArm>,
        ),
    >,
) {
    let sw = if inert { 0.0 } else { (t * 5.2 + phase).sin() };
    for &ch in children.iter() {
        if let Ok((arm, mut tf)) = stride_arm_q.get_mut(ch) {
            let s = if arm.is_left { 1.0 } else { -1.0 };
            tf.rotation = Quat::from_rotation_x(-arm_raise + sw * 0.35 * s);
        }
        if let Ok((leg, mut tf)) = stride_leg_q.get_mut(ch) {
            let s = if leg.is_left { 1.0 } else { -1.0 };
            tf.rotation = Quat::from_rotation_x(sw * 0.25 * s);
        }
        if let Ok((arm, mut tf)) = derek_arm_q.get_mut(ch) {
            let s = if arm.is_left { 1.0 } else { -1.0 };
            tf.rotation = Quat::from_rotation_x(-arm_raise + sw * 0.35 * s);
        }
        if let Ok((arm, mut tf)) = nate_arm_q.get_mut(ch) {
            let s = if arm.is_left { 1.0 } else { -1.0 };
            tf.rotation = Quat::from_rotation_x(-arm_raise + sw * 0.35 * s);
        }
        if let Ok((leg, mut tf)) = nate_leg_q.get_mut(ch) {
            let s = if leg.is_left { 1.0 } else { -1.0 };
            tf.rotation = Quat::from_rotation_x(sw * 0.25 * s);
        }
        if let Ok((leg, mut tf)) = tj_leg_q.get_mut(ch) {
            let s = if leg.is_left { 1.0 } else { -1.0 };
            tf.rotation = Quat::from_rotation_x(sw * 0.25 * s);
        }
        if let Ok(grand) = children_q.get(ch) {
            animate_hero_limbs_recursive(
                grand,
                children_q,
                t,
                phase,
                arm_raise,
                inert,
                stride_arm_q,
                stride_leg_q,
                derek_arm_q,
                nate_arm_q,
                nate_leg_q,
                tj_leg_q,
            );
        }
    }
}
