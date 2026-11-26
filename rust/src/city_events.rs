//! Cedar Rapids & Pub 217 dynamic mechanics and events:
//! - #17 BooberEats: pick up hot food from Pub 217 & deliver to NPCs around the city for TW cash
//! - #20 Lead Bomb: periodic city-wide ticking bomb that crews must hunt & defuse before blast
//! - #21 Pub 217 Code Brown: playable character has to sprint to the Pub 217 shitter
//! - #22 Stan Crashout Cutscene: intense bar crashout cutscene triggered at Pub 217

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::audio::{
    is_voice_speaking, play_cutscene_music, play_sfx, play_viking_chant_music, set_touch_playing,
    speak_voice, stop_voice,
};
use crate::city::{
    is_in_pub217, launch_npc_ragdoll, wrap_angle, CityCar, CityMinimapPlayerDot, CityNpc,
    CityWorldMarker, CITY_HALF_EXTENT, PUB217_ANCHOR, PUB217_SCALE,
};
use crate::derek::spawn_derek_rig;
use crate::glenn::spawn_glenn_rig;
use crate::nate::spawn_nate_rig;
use crate::net::{
    pack_bar_shit, pack_lead_bomb_defused, pack_lead_bomb_spawn, NetCoopEvents, NetState,
    EVENT_BAR_SHIT, EVENT_LEAD_BOMB_DEFUSED, EVENT_LEAD_BOMB_SPAWN,
};
use crate::player::Player;
use crate::ragdoll::Ragdoll;
use crate::state::{GameState, LevelEntity, PlayerStats};
use crate::tj::spawn_tj_character;
use crate::transition::{sim_running, SceneTransition};
use crate::ui::gameplay_running;
use crate::weapons::spawn_explosion_vfx;

pub struct CityEventsPlugin;

impl Plugin for CityEventsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CityEventPrompt>()
            .init_resource::<BooberEatsState>()
            .init_resource::<LeadBombState>()
            .init_resource::<BarChaosState>()
            .init_resource::<CrashoutCutsceneState>()
            .add_systems(OnEnter(GameState::CityPlay), spawn_city_events_props)
            .add_systems(OnExit(GameState::CityPlay), cleanup_city_events)
            .add_systems(
                Update,
                (
                    update_boober_eats,
                    update_lead_bomb,
                    update_bar_chaos,
                    update_city_event_prompt,
                    update_city_event_hud_and_radar,
                )
                    .chain()
                    .before(crate::city::update_city_props)
                    .run_if(in_state(GameState::CityPlay))
                    .run_if(gameplay_running),
            )
            .add_systems(OnEnter(GameState::StanCrashoutCutscene), enter_crashout_cutscene)
            .add_systems(OnExit(GameState::StanCrashoutCutscene), exit_crashout_cutscene)
            .add_systems(
                Update,
                update_crashout_cutscene
                    .run_if(in_state(GameState::StanCrashoutCutscene))
                    .run_if(sim_running),
            );
    }
}

fn rand_u64() -> u64 {
    let mut b = [0u8; 8];
    let _ = getrandom::getrandom(&mut b);
    u64::from_le_bytes(b)
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum CityEventPromptKind {
    #[default]
    None,
    BooberPickup,
    BooberDeliver,
    DefuseBomb,
    UseShitter,
}

#[derive(Resource, Default)]
pub struct CityEventPrompt {
    pub label: Option<String>,
    pub pressed: bool,
    pub kind: CityEventPromptKind,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum BooberStage {
    #[default]
    Idle,
    PickupAt217,
    DeliverToNpc,
}

#[derive(Resource, Default)]
pub struct BooberEatsState {
    pub on_shift: bool,
    pub want_start_order: bool,
    pub want_end_shift: bool,
    pub stage: BooberStage,
    target_npc: Option<Entity>,
    last_npc: Option<Entity>,
    customer_name: &'static str,
    order_name: &'static str,
    timer: f32,
    streak: u32,
    shift_deliveries: u32,
    shift_earned_tw: u32,
    cooldown: f32,
}

#[derive(Resource)]
struct LeadBombState {
    active: bool,
    pos: Vec3,
    timer: f32,
    incident_id: u8,
    spawn_cd: f32,
    beep_cd: f32,
}

impl Default for LeadBombState {
    fn default() -> Self {
        Self {
            active: false,
            pos: Vec3::ZERO,
            timer: 0.0,
            incident_id: 1,
            spawn_cd: 42.0,
            beep_cd: 0.0,
        }
    }
}

#[derive(Resource)]
struct BarChaosState {
    hang_secs: f32,
    shit_active: bool,
    shit_char: u8,
    shit_is_local: bool,
    shit_timer: f32,
    shit_cd: f32,
    crashout_hang_secs: f32,
    crashout_cd: f32,
}

impl Default for BarChaosState {
    fn default() -> Self {
        Self {
            hang_secs: 0.0,
            shit_active: false,
            shit_char: 0,
            shit_is_local: false,
            shit_timer: 0.0,
            shit_cd: 16.0,
            crashout_hang_secs: 0.0,
            crashout_cd: 28.0,
        }
    }
}

#[derive(Component)]
struct CityEventEntity;

#[derive(Component)]
struct BooberCounterProp;

#[derive(Component)]
struct BooberCustomerBeacon;

#[derive(Component)]
struct BooberGuideArrow;

#[derive(Component)]
struct BooberCarriedBag;

#[derive(Component)]
struct BooberEndShiftBtn;

#[derive(Component)]
struct LeadBombProp;

#[derive(Component)]
struct LeadBombLed;

#[derive(Component)]
struct LeadBombBeam;

#[derive(Component)]
struct PubShitterStall;

#[derive(Component)]
struct BarPoopMess(f32);

#[derive(Component)]
struct CityEventBannerRoot;

#[derive(Component)]
struct CityEventBannerText;

#[derive(Component)]
struct BooberRadarDot;

#[derive(Component)]
struct LeadBombRadarDot;

const BOOBER_ORDERS: [&str; 6] = [
    "3x Spicy Tendy Basket + Ranch",
    "Busch Light 6-Pack + Cheese Curds",
    "Double Bacon Smash + Waffle Fries",
    "Garlic Parm Wings + Pub Pretzel",
    "Hot Honey Tendy Wrap + Tots",
    "Late-Night Pork Tenderloin + Ranch",
];

const BOOBER_CUSTOMERS: [&str; 12] = [
    "Big Dave",
    "Brenda from Accounting",
    "Skater Kyle",
    "Aunt Linda",
    "Coach Rick",
    "Late-Night Tyler",
    "DJ Marco",
    "Nurse Tammy",
    "Cyclist Greg",
    "Tiffany",
    "Stoner Pat",
    "Mayor Ron",
];

fn boober_counter_pos() -> Vec3 {
    PUB217_ANCHOR + Vec3::new(-3.2, 0.0, -3.2) * PUB217_SCALE
}

fn boober_curbside_pos() -> Vec3 {
    PUB217_ANCHOR + Vec3::new(4.2, 0.0, -3.6) * PUB217_SCALE
}

fn compass_dir(delta: Vec2) -> &'static str {
    let a = delta.x.atan2(-delta.y); // 0 = North (-Z), +PI/2 = East (+X)
    let deg = a.to_degrees().rem_euclid(360.0);
    match ((deg + 22.5) / 45.0).floor() as usize % 8 {
        0 => "N",
        1 => "NE",
        2 => "E",
        3 => "SE",
        4 => "S",
        5 => "SW",
        6 => "W",
        _ => "NW",
    }
}

fn pub_shitter_pos() -> Vec3 {
    PUB217_ANCHOR + Vec3::new(-11.0, 0.0, -4.6) * PUB217_SCALE
}

const LEAD_BOMB_SPOTS: [(f32, f32); 12] = [
    (69.0, 69.0),
    (-69.0, -69.0),
    (23.0, -23.0),
    (-46.0, 0.0),
    (46.0, 0.0),
    (0.0, -46.0),
    (-80.0, -23.0),
    (34.0, -69.0),
    (58.0, 23.0),
    (-23.0, -58.0),
    (-69.0, 58.0),
    (23.0, 92.0),
];

fn spawn_city_events_props(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    stats: Res<PlayerStats>,
    mut boober: ResMut<BooberEatsState>,
    mut bomb: ResMut<LeadBombState>,
    mut chaos: ResMut<BarChaosState>,
) {
    // Keep delivery cooldown clean and prevent instant repeat bar chaos right after Cutscene return.
    boober.on_shift = false;
    boober.want_start_order = false;
    boober.want_end_shift = false;
    boober.stage = BooberStage::Idle;
    boober.target_npc = None;
    boober.last_npc = None;
    boober.shift_deliveries = 0;
    boober.shift_earned_tw = 0;
    bomb.active = false;
    bomb.spawn_cd = if stats.free_roam { 32.0 } else { 52.0 };
    chaos.shit_active = false;
    chaos.hang_secs = 0.0;
    chaos.crashout_hang_secs = 0.0;

    // 1. Pub 217 BooberEats Hot-Bag Pickup Station (Indoor Counter + Outdoor Curbside Drive-Thru Pad)
    let cyan_glow = materials.add(StandardMaterial {
        base_color: Color::srgb(0.15, 0.92, 1.0),
        emissive: LinearRgba::rgb(0.4, 2.0, 2.6),
        unlit: true,
        ..default()
    });
    let bag_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.72, 0.88),
        perceptual_roughness: 0.35,
        ..default()
    });
    let gold_trim = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.82, 0.16),
        emissive: LinearRgba::rgb(1.5, 1.1, 0.12),
        unlit: true,
        ..default()
    });
    let wood_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.28, 0.15, 0.09),
        perceptual_roughness: 0.5,
        ..default()
    });
    let c_pos = boober_counter_pos();
    commands
        .spawn((
            LevelEntity,
            CityWorldMarker,
            CityEventEntity,
            BooberCounterProp,
            Transform::from_translation(c_pos),
            Visibility::default(),
        ))
        .with_children(|c| {
            c.spawn((
                Mesh3d(meshes.add(Cuboid::new(1.3, 1.05, 0.85))),
                MeshMaterial3d(wood_mat.clone()),
                Transform::from_xyz(0.0, 0.52, 0.0),
            ));
            c.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.58, 0.44, 0.48))),
                MeshMaterial3d(bag_mat.clone()),
                Transform::from_xyz(0.0, 1.27, 0.0),
            ));
            c.spawn((
                Mesh3d(meshes.add(Torus::new(0.95, 1.08))),
                MeshMaterial3d(cyan_glow.clone()),
                Transform::from_xyz(0.0, 0.08, 0.0),
            ));
        });

    // Curbside pickup pad right outside Pub 217 doorway for cars/scooters/foot
    let curb_pos = boober_curbside_pos();
    commands
        .spawn((
            LevelEntity,
            CityWorldMarker,
            CityEventEntity,
            BooberCounterProp,
            Transform::from_translation(curb_pos),
            Visibility::default(),
        ))
        .with_children(|c| {
            c.spawn((
                Mesh3d(meshes.add(Torus::new(1.55, 1.85))),
                MeshMaterial3d(cyan_glow.clone()),
                Transform::from_xyz(0.0, 0.10, 0.0),
            ));
            c.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.65, 0.50, 0.52))),
                MeshMaterial3d(bag_mat.clone()),
                Transform::from_xyz(0.0, 0.95, 0.0),
            ));
            c.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.32, 0.32, 0.32))),
                MeshMaterial3d(gold_trim.clone()),
                Transform::from_xyz(0.0, 1.75, 0.0)
                    .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_4) * Quat::from_rotation_x(std::f32::consts::FRAC_PI_4)),
            ));
        });

    // Floating 3D Crazy-Taxi guide arrow + carried delivery hot-bag (hidden unless on shift)
    commands
        .spawn((
            LevelEntity,
            CityWorldMarker,
            CityEventEntity,
            BooberGuideArrow,
            Transform::default(),
            Visibility::Hidden,
        ))
        .with_children(|a| {
            // Arrow points toward -Z in local frame (`looking_to(dir, Vec3::Y)`)
            a.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.22, 0.14, 0.68))),
                MeshMaterial3d(cyan_glow.clone()),
                Transform::from_xyz(0.0, 0.0, 0.20),
            ));
            a.spawn((
                Mesh3d(meshes.add(Cone::new(0.30, 0.55))),
                MeshMaterial3d(gold_trim.clone()),
                Transform::from_xyz(0.0, 0.0, -0.35)
                    .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
            ));
        });
    commands
        .spawn((
            LevelEntity,
            CityWorldMarker,
            CityEventEntity,
            BooberCarriedBag,
            Transform::default(),
            Visibility::Hidden,
        ))
        .with_children(|b| {
            b.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.54, 0.42, 0.44))),
                MeshMaterial3d(bag_mat),
                Transform::default(),
            ));
            b.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.58, 0.08, 0.48))),
                MeshMaterial3d(gold_trim),
                Transform::from_xyz(0.0, 0.16, 0.0),
            ));
        });

    // 2. Pub 217 Back-Corner Emergency Bathroom Stall
    let stall_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.24, 0.28, 0.34),
        perceptual_roughness: 0.5,
        ..default()
    });
    let porcelain_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.96, 0.97, 0.99),
        perceptual_roughness: 0.18,
        ..default()
    });
    let stall_beacon_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.35, 0.95, 0.35, 0.7),
        emissive: LinearRgba::rgb(0.8, 3.2, 0.8),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let s_pos = pub_shitter_pos();
    commands
        .spawn((
            LevelEntity,
            CityWorldMarker,
            CityEventEntity,
            PubShitterStall,
            Transform::from_translation(s_pos),
            Visibility::default(),
        ))
        .with_children(|s| {
            s.spawn((
                Mesh3d(meshes.add(Cuboid::new(1.5, 2.2, 0.14))),
                MeshMaterial3d(stall_mat.clone()),
                Transform::from_xyz(0.0, 1.1, 0.85),
            ));
            s.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.28, 0.52))),
                MeshMaterial3d(porcelain_mat),
                Transform::from_xyz(0.0, 0.26, 0.0),
            ));
            s.spawn((
                Mesh3d(meshes.add(Torus::new(1.0, 1.15))),
                MeshMaterial3d(stall_beacon_mat),
                Transform::from_xyz(0.0, 0.12, 0.0),
            ));
        });

    // 3. Top-Center Event Alert Banner HUD (with clickable END SHIFT pill when working BooberEats)
    commands
        .spawn((
            CityEventEntity,
            CityEventBannerRoot,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(64.0),
                left: Val::Percent(18.0),
                width: Val::Percent(64.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(5.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.06, 0.08, 0.16, 0.88)),
            BorderColor(Color::srgb(0.35, 0.90, 1.0)),
            BorderRadius::all(Val::Px(10.0)),
            GlobalZIndex(18),
            Visibility::Hidden,
        ))
        .with_children(|b| {
            b.spawn((
                CityEventBannerText,
                Text::new(""),
                TextFont {
                    font_size: 13.5,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            b.spawn((
                BooberEndShiftBtn,
                Button,
                Node {
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.55, 0.14, 0.18)),
                BorderColor(Color::srgb(1.0, 0.72, 0.32)),
                BorderRadius::all(Val::Px(6.0)),
                Visibility::Hidden,
            ))
            .with_children(|eb| {
                eb.spawn((
                    Text::new("END SHIFT"),
                    TextFont {
                        font_size: 11.5,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });
        });
}

fn cleanup_city_events(
    mut commands: Commands,
    q: Query<Entity, With<CityEventEntity>>,
    mut prompt: ResMut<CityEventPrompt>,
) {
    for e in &q {
        commands.entity(e).despawn_recursive();
    }
    *prompt = CityEventPrompt::default();
}

#[derive(SystemParam)]
struct BooberQueries<'w, 's> {
    player_q: Query<
        'w,
        's,
        &'static Transform,
        (
            With<Player>,
            Without<BooberCustomerBeacon>,
            Without<BooberGuideArrow>,
            Without<BooberCarriedBag>,
            Without<CityNpc>,
        ),
    >,
    npc_q: Query<
        'w,
        's,
        (Entity, &'static Transform),
        (
            With<CityNpc>,
            Without<Player>,
            Without<BooberCustomerBeacon>,
            Without<BooberGuideArrow>,
            Without<BooberCarriedBag>,
        ),
    >,
    beacon_q: Query<
        'w,
        's,
        (Entity, &'static mut Transform),
        (
            With<BooberCustomerBeacon>,
            Without<CityNpc>,
            Without<Player>,
            Without<BooberGuideArrow>,
            Without<BooberCarriedBag>,
        ),
    >,
    arrow_q: Query<
        'w,
        's,
        (&'static mut Transform, &'static mut Visibility),
        (
            With<BooberGuideArrow>,
            Without<Player>,
            Without<CityNpc>,
            Without<BooberCustomerBeacon>,
            Without<BooberCarriedBag>,
        ),
    >,
    bag_q: Query<
        'w,
        's,
        (&'static mut Transform, &'static mut Visibility),
        (
            With<BooberCarriedBag>,
            Without<Player>,
            Without<CityNpc>,
            Without<BooberCustomerBeacon>,
            Without<BooberGuideArrow>,
        ),
    >,
    end_btn_q: Query<'w, 's, &'static Interaction, (Changed<Interaction>, With<BooberEndShiftBtn>)>,
}

fn pick_random_boober_npc(
    npc_q: &Query<
        (Entity, &Transform),
        (
            With<CityNpc>,
            Without<Player>,
            Without<BooberCustomerBeacon>,
            Without<BooberGuideArrow>,
            Without<BooberCarriedBag>,
        ),
    >,
    last_npc: Option<Entity>,
) -> Option<(Entity, Vec3)> {
    let pub_pos = boober_curbside_pos();
    let candidates: Vec<(Entity, Vec3)> = npc_q
        .iter()
        .filter_map(|(e, tf)| {
            if Some(e) == last_npc {
                return None;
            }
            let d = Vec2::new(tf.translation.x - pub_pos.x, tf.translation.z - pub_pos.z).length();
            if (36.0..=130.0).contains(&d) {
                Some((e, tf.translation))
            } else {
                None
            }
        })
        .collect();
    if !candidates.is_empty() {
        return Some(candidates[(rand_u64() as usize) % candidates.len()]);
    }
    let fallback: Vec<(Entity, Vec3)> = npc_q.iter().map(|(e, tf)| (e, tf.translation)).collect();
    if fallback.is_empty() {
        None
    } else {
        Some(fallback[(rand_u64() as usize) % fallback.len()])
    }
}

/// #17 BooberEats shift delivery job: clock in at Pub 217 (or Maria), grab food, deliver to a
/// random Cedar Rapids NPC on foot / scooter / car, then return to 217 for the next ticket!
fn update_boober_eats(
    mut commands: Commands,
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut stats: ResMut<PlayerStats>,
    mut boober: ResMut<BooberEatsState>,
    prompt: Res<CityEventPrompt>,
    mut qs: BooberQueries,
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();
    boober.cooldown = (boober.cooldown - dt).max(0.0);

    // 1. Check clock-out (`END SHIFT` banner button or Maria's `END BOOBEREATS` button)
    let end_clicked = qs.end_btn_q.iter().any(|i| *i == Interaction::Pressed);
    if boober.want_end_shift || (end_clicked && boober.on_shift) {
        boober.want_end_shift = false;
        boober.want_start_order = false;
        let done = boober.shift_deliveries;
        let earned = boober.shift_earned_tw;
        boober.on_shift = false;
        boober.stage = BooberStage::Idle;
        boober.target_npc = None;
        boober.streak = 0;
        boober.shift_deliveries = 0;
        boober.shift_earned_tw = 0;
        for (be, _) in &qs.beacon_q {
            commands.entity(be).despawn_recursive();
        }
        play_sfx("chip_bet");
        stats.hint_text = format!(
            "BOOBEREATS SHIFT ENDED! Completed {} deliveries (+{} TW total).",
            done, earned
        );
        stats.hint_bump = stats.hint_bump.wrapping_add(1);
    }
    boober.want_end_shift = false;

    let p_tf_opt = qs.player_q.get_single().ok().copied();
    let in_vehicle = stats.driving_car.is_some() || stats.riding_scooter.is_some();
    let near_curb_pad = p_tf_opt.is_some_and(|p| {
        Vec2::new(p.translation.x - boober_curbside_pos().x, p.translation.z - boober_curbside_pos().z).length() < 4.8
    });

    // Auto-dispatch next ticket if on shift at PickupAt217 and pulling onto Pub 217 curb in a vehicle
    let auto_curb_pickup = boober.on_shift
        && boober.stage == BooberStage::PickupAt217
        && in_vehicle
        && near_curb_pad
        && boober.cooldown <= 0.0;

    let pickup_triggered = boober.want_start_order
        || (prompt.pressed && prompt.kind == CityEventPromptKind::BooberPickup)
        || auto_curb_pickup;
    boober.want_start_order = false;

    if pickup_triggered && boober.stage != BooberStage::DeliverToNpc {
        if let Some((npc_e, npc_pos)) = pick_random_boober_npc(&qs.npc_q, boober.last_npc) {
            boober.on_shift = true;
            boober.stage = BooberStage::DeliverToNpc;
            boober.target_npc = Some(npc_e);
            boober.last_npc = Some(npc_e);
            boober.order_name = BOOBER_ORDERS[(rand_u64() as usize) % BOOBER_ORDERS.len()];
            boober.customer_name = BOOBER_CUSTOMERS[(rand_u64() as usize) % BOOBER_CUSTOMERS.len()];
            boober.timer = 65.0;

            for (be, _) in &qs.beacon_q {
                commands.entity(be).despawn_recursive();
            }
            let beam_mat = materials.add(StandardMaterial {
                base_color: Color::srgba(0.15, 0.94, 1.0, 0.65),
                emissive: LinearRgba::rgb(0.4, 2.4, 3.2),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            });
            let ring_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.85, 0.15),
                emissive: LinearRgba::rgb(2.6, 2.0, 0.3),
                unlit: true,
                ..default()
            });
            commands
                .spawn((
                    LevelEntity,
                    CityWorldMarker,
                    CityEventEntity,
                    BooberCustomerBeacon,
                    Transform::from_translation(npc_pos),
                    Visibility::default(),
                ))
                .with_children(|b| {
                    b.spawn((
                        Mesh3d(meshes.add(Cylinder::new(0.32, 36.0))),
                        MeshMaterial3d(beam_mat),
                        Transform::from_xyz(0.0, 18.0, 0.0),
                    ));
                    b.spawn((
                        Mesh3d(meshes.add(Torus::new(1.6, 1.9))),
                        MeshMaterial3d(ring_mat.clone()),
                        Transform::from_xyz(0.0, 0.25, 0.0),
                    ));
                    b.spawn((
                        Mesh3d(meshes.add(Cone::new(0.45, 0.75))),
                        MeshMaterial3d(ring_mat),
                        Transform::from_xyz(0.0, 3.0, 0.0)
                            .with_rotation(Quat::from_rotation_x(std::f32::consts::PI)),
                    ));
                });

            play_sfx("hack_success");
            stats.hint_text = format!(
                "BOOBEREATS: Deliver {} to {} (cyan beacon) in 65s!",
                boober.order_name, boober.customer_name
            );
            stats.hint_bump = stats.hint_bump.wrapping_add(1);
            speak_voice("New BooberEats order up! Deliver it hot!", "bartender");
        }
    }

    // 2. Delivery tracking + completion
    if boober.stage == BooberStage::DeliverToNpc {
        boober.timer -= dt;
        let target_pos = boober.target_npc.and_then(|e| qs.npc_q.get(e).ok()).map(|(_, tf)| tf.translation);

        if let Some(target_pos) = target_pos {
            for (_, mut b_tf) in &mut qs.beacon_q {
                b_tf.translation = target_pos;
                b_tf.rotate_y(dt * 1.9);
            }

            let delivered = p_tf_opt.is_some_and(|p_tf| {
                let d = Vec2::new(p_tf.translation.x - target_pos.x, p_tf.translation.z - target_pos.z).length();
                d < 4.8 || (d < 6.5 && prompt.pressed && prompt.kind == CityEventPromptKind::BooberDeliver)
            });

            if delivered {
                boober.stage = BooberStage::PickupAt217;
                boober.target_npc = None;
                boober.cooldown = 2.0;
                boober.streak += 1;
                boober.shift_deliveries += 1;
                let time_bonus = ((boober.timer / 65.0) * 18.0).round() as u32;
                let streak_bonus = (boober.streak.saturating_sub(1) * 5).min(20);
                let payout = 20 + time_bonus + streak_bonus;
                boober.shift_earned_tw += payout;
                stats.boober_deliveries += 1;
                stats.add_tendy_wendies(payout);
                stats.score += 850 + payout * 35;
                play_sfx("powerup");
                play_sfx("chip_bet");
                stats.hint_text = format!(
                    "DELIVERED TO {}! +{} TW (+{} Tip! Streak x{}) -- Return to Pub 217 for next order!",
                    boober.customer_name,
                    payout,
                    time_bonus + streak_bonus,
                    boober.streak
                );
                stats.hint_bump = stats.hint_bump.wrapping_add(1);
                speak_voice("Delivered hot! Head back to 217 for the next order!", "narrator");
                for (be, _) in &qs.beacon_q {
                    commands.entity(be).despawn_recursive();
                }
            } else if boober.timer <= 0.0 {
                boober.stage = BooberStage::PickupAt217;
                boober.target_npc = None;
                boober.cooldown = 1.5;
                boober.streak = 0;
                play_sfx("hack_deny");
                stats.hint_text = format!(
                    "{}'s food got cold! Return to Pub 217 curbside or counter for another order.",
                    boober.customer_name
                );
                stats.hint_bump = stats.hint_bump.wrapping_add(1);
                for (be, _) in &qs.beacon_q {
                    commands.entity(be).despawn_recursive();
                }
            }
        } else {
            // NPC reference lost unexpectedly: return to PickupAt217 cleanly
            boober.stage = BooberStage::PickupAt217;
            boober.target_npc = None;
            for (be, _) in &qs.beacon_q {
                commands.entity(be).despawn_recursive();
            }
        }
    } else {
        for (be, _) in &qs.beacon_q {
            commands.entity(be).despawn_recursive();
        }
    }

    // 3. Update 3D Crazy-Taxi guide arrow + carried thermal delivery bag above player/car
    let guide_target = if boober.stage == BooberStage::DeliverToNpc {
        boober.target_npc.and_then(|e| qs.npc_q.get(e).ok()).map(|(_, tf)| tf.translation)
    } else if boober.stage == BooberStage::PickupAt217 {
        Some(boober_curbside_pos())
    } else {
        None
    };

    let hide_fx = stats.char_select_open || stats.phone_open || stats.bartender_open || stats.riding_wheel;
    if let (Some(p_tf), Some(tgt)) = (p_tf_opt.filter(|_| !hide_fx), guide_target) {
        let dir = Vec3::new(tgt.x - p_tf.translation.x, 0.0, tgt.z - p_tf.translation.z).normalize_or(Vec3::NEG_Z);
        let top_y = if stats.driving_car.is_some() {
            2.65
        } else {
            1.75 * stats.body_scale().max(0.8) + 0.7
        };
        for (mut a_tf, mut a_vis) in &mut qs.arrow_q {
            *a_vis = Visibility::Visible;
            a_tf.translation = p_tf.translation + Vec3::Y * (top_y + (t * 4.5).sin() * 0.12);
            a_tf.look_to(dir, Vec3::Y);
        }
        for (mut b_tf, mut b_vis) in &mut qs.bag_q {
            if boober.stage == BooberStage::DeliverToNpc {
                *b_vis = Visibility::Visible;
                let back = if stats.driving_car.is_some() {
                    Vec3::Y * 2.05
                } else {
                    (p_tf.rotation * Vec3::Z) * 0.42 + Vec3::Y * (1.05 * stats.body_scale().max(0.8))
                };
                b_tf.translation = p_tf.translation + back;
                b_tf.rotation = p_tf.rotation;
            } else if *b_vis != Visibility::Hidden {
                *b_vis = Visibility::Hidden;
            }
        }
    } else {
        for (_, mut v) in &mut qs.arrow_q {
            if *v != Visibility::Hidden {
                *v = Visibility::Hidden;
            }
        }
        for (_, mut v) in &mut qs.bag_q {
            if *v != Visibility::Hidden {
                *v = Visibility::Hidden;
            }
        }
    }
}

#[derive(SystemParam)]
struct LeadBombTargets<'w, 's> {
    player_q: Query<'w, 's, (&'static Transform, &'static mut Player), Without<CityNpc>>,
    npc_q: Query<'w, 's, (&'static mut Transform, &'static mut CityNpc), Without<Player>>,
    car_q: Query<
        'w,
        's,
        (&'static Transform, &'static mut CityCar),
        (Without<Player>, Without<CityNpc>),
    >,
    bomb_q: Query<
        'w,
        's,
        Entity,
        (With<LeadBombProp>, Without<Player>, Without<CityNpc>, Without<CityCar>),
    >,
    led_q: Query<
        'w,
        's,
        &'static mut Transform,
        (
            With<LeadBombLed>,
            Without<LeadBombProp>,
            Without<Player>,
            Without<CityNpc>,
            Without<CityCar>,
        ),
    >,
}

fn spawn_lead_bomb_entities(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
) {
    let lead_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.24, 0.28),
        metallic: 0.88,
        perceptual_roughness: 0.32,
        ..default()
    });
    let hazard_yellow = materials.add(StandardMaterial {
        base_color: Color::srgb(0.98, 0.78, 0.08),
        emissive: LinearRgba::rgb(0.5, 0.35, 0.0),
        ..default()
    });
    let red_led = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.14, 0.10),
        emissive: LinearRgba::rgb(4.5, 0.3, 0.15),
        unlit: true,
        ..default()
    });
    let red_beam = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.16, 0.12, 0.45),
        emissive: LinearRgba::rgb(3.5, 0.3, 0.2),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    commands
        .spawn((
            LevelEntity,
            CityWorldMarker,
            CityEventEntity,
            LeadBombProp,
            Transform::from_translation(pos),
            Visibility::default(),
        ))
        .with_children(|b| {
            b.spawn((
                Mesh3d(meshes.add(Sphere::new(0.85))),
                MeshMaterial3d(lead_mat.clone()),
                Transform::from_xyz(0.0, 0.92, 0.0),
            ));
            b.spawn((
                Mesh3d(meshes.add(Torus::new(0.14, 0.88))),
                MeshMaterial3d(hazard_yellow),
                Transform::from_xyz(0.0, 0.92, 0.0),
            ));
            b.spawn((
                LeadBombLed,
                Mesh3d(meshes.add(Sphere::new(0.24))),
                MeshMaterial3d(red_led),
                Transform::from_xyz(0.0, 1.85, 0.0),
            ));
            b.spawn((
                LeadBombBeam,
                Mesh3d(meshes.add(Cylinder::new(0.45, 58.0))),
                MeshMaterial3d(red_beam),
                Transform::from_xyz(0.0, 29.0, 0.0),
            ));
        });
}

/// #20 City-wide Lead Bomb crisis mechanic.
#[allow(clippy::too_many_arguments)]
fn update_lead_bomb(
    mut commands: Commands,
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut stats: ResMut<PlayerStats>,
    net: Res<NetState>,
    mut coop: ResMut<NetCoopEvents>,
    prompt: Res<CityEventPrompt>,
    mut bomb: ResMut<LeadBombState>,
    mut next_state: SceneTransition,
    mut ragdoll: ResMut<Ragdoll>,
    mut targets: LeadBombTargets,
) {
    let dt = time.delta_secs().min(0.05);

    // 1. Handle remote co-op Lead Bomb spawns & defusals
    for (spot_idx, inc_id) in coop.lead_bomb_spawns.drain(..) {
        if !bomb.active || bomb.incident_id != inc_id {
            for e in &targets.bomb_q {
                commands.entity(e).despawn_recursive();
            }
            let &(bx, bz) = &LEAD_BOMB_SPOTS[(spot_idx as usize) % LEAD_BOMB_SPOTS.len()];
            bomb.active = true;
            bomb.pos = Vec3::new(bx, 0.0, bz);
            bomb.timer = 60.0;
            bomb.incident_id = inc_id;
            bomb.beep_cd = 0.2;
            spawn_lead_bomb_entities(&mut commands, &mut meshes, &mut materials, bomb.pos);
            play_sfx("bomb_siren");
        }
    }
    for (inc_id, def_char) in coop.lead_bomb_defused.drain(..) {
        if bomb.active && bomb.incident_id == inc_id {
            bomb.active = false;
            bomb.spawn_cd = 95.0;
            for e in &targets.bomb_q {
                commands.entity(e).despawn_recursive();
            }
            stats.add_tendy_wendies(35);
            stats.score += 2500;
            play_sfx("hack_success");
            play_sfx("win");
            stats.hint_text = format!(
                "{} DEFUSED THE LEAD BOMB! Team bonus +35 TW!",
                NetState::char_display_name(def_char)
            );
            stats.hint_bump = stats.hint_bump.wrapping_add(1);
        }
    }

    // 2. Periodically spawn a new Lead Bomb (anchor in co-op, or solo locally)
    if !bomb.active {
        bomb.spawn_cd -= dt;
        if bomb.spawn_cd <= 0.0 && (!net.is_coop_active() || net.is_anchor()) {
            let spot_idx = (rand_u64() as usize) % LEAD_BOMB_SPOTS.len();
            let &(bx, bz) = &LEAD_BOMB_SPOTS[spot_idx];
            bomb.incident_id = bomb.incident_id.wrapping_add(1).max(1);
            bomb.active = true;
            bomb.pos = Vec3::new(bx, 0.0, bz);
            bomb.timer = 60.0;
            bomb.spawn_cd = 95.0 + (rand_u64() % 40) as f32;
            bomb.beep_cd = 0.3;
            spawn_lead_bomb_entities(&mut commands, &mut meshes, &mut materials, bomb.pos);
            crate::audio::net_event(
                EVENT_LEAD_BOMB_SPAWN,
                pack_lead_bomb_spawn(spot_idx as u8, bomb.incident_id),
            );
            play_sfx("bomb_siren");
            stats.hint_text =
                "LEAD BOMB ALERT! A ticking Lead Bomb dropped in Cedar Rapids -- FIND & DEFUSE IT!"
                    .to_string();
            stats.hint_bump = stats.hint_bump.wrapping_add(1);
            speak_voice("Warning! Lead bomb armed in Cedar Rapids! Go defuse it!", "narrator");
        }
        return;
    }

    bomb.timer -= dt;
    let p_pos = targets
        .player_q
        .get_single()
        .map(|(t, _)| t.translation)
        .unwrap_or(bomb.pos);
    let dist = Vec2::new(p_pos.x - bomb.pos.x, p_pos.z - bomb.pos.z).length();

    // Hot/cold Geiger/bomb countdown beep
    bomb.beep_cd -= dt;
    if bomb.beep_cd <= 0.0 {
        play_sfx("bomb_beep");
        let dist_fac = (dist / 85.0).clamp(0.08, 1.0);
        let urgency = (bomb.timer / 60.0).clamp(0.15, 1.0);
        bomb.beep_cd = (dist_fac * urgency * 1.4).clamp(0.16, 1.35);
    }

    // Pulse red LED
    let pulse = 0.65 + 0.65 * (time.elapsed_secs() * (8.0 + (60.0 - bomb.timer) * 0.4)).sin().abs();
    for mut l_tf in &mut targets.led_q {
        l_tf.scale = Vec3::splat(pulse);
    }

    // Check Defusal [F/E]
    if prompt.pressed && prompt.kind == CityEventPromptKind::DefuseBomb && dist < 5.0 {
        bomb.active = false;
        bomb.spawn_cd = 95.0 + (rand_u64() % 40) as f32;
        for e in &targets.bomb_q {
            commands.entity(e).despawn_recursive();
        }
        crate::audio::net_event(
            EVENT_LEAD_BOMB_DEFUSED,
            pack_lead_bomb_defused(bomb.incident_id, net.local_char()),
        );
        stats.add_tendy_wendies(50);
        stats.score += 4000;
        stats.hearts = (stats.hearts + 4).min(stats.max_hearts);
        play_sfx("hack_success");
        play_sfx("win");
        stats.hint_text = "LEAD BOMB DEFUSED! Cedar Rapids saved! +50 TW & +4000 Score!".to_string();
        stats.hint_bump = stats.hint_bump.wrapping_add(1);
        speak_voice("Lead bomb defused! Huge clutch!", "narrator");
        return;
    }

    // Detonation if timer hits zero!
    if bomb.timer <= 0.0 {
        bomb.active = false;
        bomb.spawn_cd = 100.0 + (rand_u64() % 40) as f32;
        for e in &targets.bomb_q {
            commands.entity(e).despawn_recursive();
        }
        let center = bomb.pos;
        spawn_explosion_vfx(&mut commands, &mut meshes, &mut materials, center, 44.0, 1.4);
        play_sfx("explosion");
        play_sfx("crashout_boom");
        stats.shake_request = stats.shake_request.max(0.95);
        stats.hurt_flash = 0.85;
        stats.hint_text = "THE LEAD BOMB DETONATED!! Huge shockwave across Cedar Rapids!".to_string();
        stats.hint_bump = stats.hint_bump.wrapping_add(1);
        speak_voice("Boom! The lead bomb went off!", "narrator");

        for (mut n_tf, mut npc) in &mut targets.npc_q {
            let delta = n_tf.translation - center;
            let nd = delta.length();
            if nd < 58.0 {
                let k = (1.0 - nd / 58.0).max(0.3);
                let away = delta.with_y(0.0).normalize_or(Vec3::X);
                launch_npc_ragdoll(&mut npc, &mut n_tf, away * (36.0 * k) + Vec3::Y * (22.0 * k + 6.0), 40.0);
            }
        }
        for (c_tf, mut car) in &mut targets.car_q {
            if car.occupied {
                continue;
            }
            let diff = Vec2::new(c_tf.translation.x - center.x, c_tf.translation.z - center.z);
            let cd = diff.length();
            if cd < 55.0 {
                let k = (1.0 - cd / 55.0).max(0.25);
                let away = if cd > 0.01 { diff / cd } else { Vec2::X };
                car.knock_vel += away * (38.0 * k);
                car.yaw = wrap_angle(car.yaw + 1.1 * k);
            }
        }
        if stats.driving_car.is_none() && !stats.riding_wheel {
            if let Ok((ptf, mut player)) = targets.player_q.get_single_mut() {
                if !is_in_pub217(ptf.translation, 0.0) {
                    let away = (ptf.translation - center).with_y(0.0).normalize_or(Vec3::Z);
                    let imp = away * 18.0 + Vec3::Y * 12.0;
                    if crate::ragdoll::can_ragdoll(&stats) {
                        ragdoll.request(Some(player.velocity.with_y(0.0)), imp, -8.0);
                    } else {
                        player.velocity = imp;
                    }
                    let min_hp = if stats.free_roam { 1 } else { 0 };
                    stats.hearts = (stats.hearts - 3).max(min_hp);
                    if stats.hearts <= 0 && !stats.free_roam {
                        play_sfx("gameover");
                        next_state.set(GameState::GameOver);
                    }
                }
            }
        }
    }
}

/// #21 Pub 217 Shitter Emergency + #22 Triggering Stan's Crashout Cutscene while inside Pub 217.
#[allow(clippy::too_many_arguments)]
fn update_bar_chaos(
    mut commands: Commands,
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut stats: ResMut<PlayerStats>,
    net: Res<NetState>,
    mut coop: ResMut<NetCoopEvents>,
    prompt: Res<CityEventPrompt>,
    mut chaos: ResMut<BarChaosState>,
    mut next_state: SceneTransition,
    mut ragdoll: ResMut<Ragdoll>,
    mut player_q: Query<(&Transform, &mut Player), Without<BarPoopMess>>,
    mut mess_q: Query<(Entity, &mut Transform, &mut BarPoopMess), Without<Player>>,
) {
    let dt = time.delta_secs().min(0.05);
    chaos.shit_cd = (chaos.shit_cd - dt).max(0.0);
    chaos.crashout_cd = (chaos.crashout_cd - dt).max(0.0);

    // Drain active poop messes
    for (e, mut tf, mut mess) in &mut mess_q {
        mess.0 -= dt;
        tf.rotate_y(dt * 0.4);
        if mess.0 <= 0.0 {
            commands.entity(e).despawn_recursive();
        }
    }

    // Consume remote Bar Shit notifications
    for (vc, out) in coop.bar_shits.drain(..) {
        let who = NetState::char_display_name(vc);
        if out == 0 {
            play_sfx("fart_splat");
            stats.hint_text = format!("CODE BROWN IN PUB 217! {} has to sprint to the bathroom!", who);
            stats.hint_bump = stats.hint_bump.wrapping_add(1);
        } else if out == 1 {
            play_sfx("toilet_flush");
            stats.hint_text = format!("{} made it to the Pub 217 shitter!", who);
        } else if out == 2 {
            play_sfx("fart_splat");
            stats.hint_text = format!("OH GOD {} SHIT RIGHT IN PUB 217!", who);
            stats.hint_bump = stats.hint_bump.wrapping_add(1);
        }
    }

    let Ok((p_tf, mut player)) = player_q.get_single_mut() else {
        return;
    };
    let in_bar = is_in_pub217(p_tf.translation, -0.5)
        && stats.driving_car.is_none()
        && stats.riding_scooter.is_none()
        && !stats.phone_open
        && !stats.bartender_open
        && !stats.char_select_open;

    if in_bar {
        if !chaos.shit_active && chaos.shit_cd <= 0.0 {
            chaos.hang_secs += dt;
            if chaos.hang_secs >= 15.0 {
                chaos.hang_secs = 0.0;
                chaos.shit_cd = 85.0;
                chaos.shit_active = true;
                let roll = (rand_u64() % 10) as u8;
                if roll < 7 {
                    chaos.shit_char = net.local_char();
                    chaos.shit_is_local = true;
                    chaos.shit_timer = 15.0;
                } else {
                    chaos.shit_char = (net.local_char() + 1 + (roll % 4)) % 5;
                    chaos.shit_is_local = false;
                    chaos.shit_timer = 6.0;
                }
                let who = NetState::char_display_name(chaos.shit_char);
                crate::audio::net_event(EVENT_BAR_SHIT, pack_bar_shit(chaos.shit_char, 0));
                play_sfx("fart_splat");
                stats.hint_text = if chaos.shit_is_local {
                    format!("CODE BROWN AT 217! {} ate bad curds -- RUN TO SW SHITTER IN 15s!", who)
                } else {
                    format!("UH OH! {}'s stomach is rumbling at Pub 217!!", who)
                };
                stats.hint_bump = stats.hint_bump.wrapping_add(1);
                speak_voice("Code brown! Somebody has to shit in Pub 217!", "narrator");
            }
        }

        // #22 Stan Crashout Cutscene trigger after hanging in Pub 217
        if !chaos.shit_active && chaos.crashout_cd <= 0.0 {
            chaos.crashout_hang_secs += dt;
            if chaos.crashout_hang_secs >= 26.0 {
                chaos.crashout_hang_secs = 0.0;
                chaos.crashout_cd = 150.0;
                stats.saved_city_pos = p_tf.translation;
                stats.crashout_return_city = true;
                play_sfx("rage_swell");
                next_state.set(GameState::StanCrashoutCutscene);
                return;
            }
        }
    } else {
        chaos.hang_secs = 0.0;
        chaos.crashout_hang_secs = 0.0;
    }

    if !chaos.shit_active {
        return;
    }
    chaos.shit_timer -= dt;
    let stall = pub_shitter_pos();
    let d_stall = Vec2::new(p_tf.translation.x - stall.x, p_tf.translation.z - stall.z).length();
    let hit_stall = chaos.shit_is_local
        && (d_stall < 2.2 || (prompt.pressed && prompt.kind == CityEventPromptKind::UseShitter));

    if hit_stall {
        chaos.shit_active = false;
        crate::audio::net_event(EVENT_BAR_SHIT, pack_bar_shit(chaos.shit_char, 1));
        play_sfx("toilet_flush");
        play_sfx("powerup");
        stats.add_tendy_wendies(15);
        stats.score += 500;
        stats.dash_energy = 100.0;
        stats.hint_text = "PHEW!! Made it to the Pub 217 shitter just in time! +15 TW".to_string();
        stats.hint_bump = stats.hint_bump.wrapping_add(1);
        speak_voice("Phew! Made it to the bathroom!", "narrator");
        return;
    }

    if chaos.shit_timer <= 0.0 {
        chaos.shit_active = false;
        let who = NetState::char_display_name(chaos.shit_char);
        crate::audio::net_event(EVENT_BAR_SHIT, pack_bar_shit(chaos.shit_char, 2));
        play_sfx("fart_splat");
        stats.shake_request = stats.shake_request.max(0.35);

        let mess_pos = if chaos.shit_is_local {
            p_tf.translation
        } else {
            PUB217_ANCHOR + Vec3::new(-6.0, 0.0, 1.5) * PUB217_SCALE
        };
        spawn_poop_mess(&mut commands, &mut meshes, &mut materials, mess_pos);

        if chaos.shit_is_local && crate::ragdoll::can_ragdoll(&stats) {
            ragdoll.request(
                Some(player.velocity.with_y(0.0)),
                Vec3::new(0.0, 4.2, -3.5),
                -4.5,
            );
            player.invuln_timer = 1.0;
        }
        stats.hint_text = format!("DISASTER AT 217!! {} JUST SHIT RIGHT ON THE BAR FLOOR!", who);
        stats.hint_bump = stats.hint_bump.wrapping_add(1);
        speak_voice("Oh god! Somebody just shit in the bar!", "bartender");
    }
}

fn spawn_poop_mess(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
) {
    let brown = materials.add(StandardMaterial {
        base_color: Color::srgb(0.36, 0.21, 0.08),
        perceptual_roughness: 0.45,
        ..default()
    });
    let stink = materials.add(StandardMaterial {
        base_color: Color::srgba(0.45, 0.85, 0.20, 0.55),
        emissive: LinearRgba::rgb(0.4, 1.2, 0.15),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    commands
        .spawn((
            LevelEntity,
            CityWorldMarker,
            CityEventEntity,
            BarPoopMess(18.0),
            Transform::from_translation(Vec3::new(pos.x, 0.05, pos.z)),
            Visibility::default(),
        ))
        .with_children(|m| {
            m.spawn((
                Mesh3d(meshes.add(Torus::new(0.14, 0.28))),
                MeshMaterial3d(brown.clone()),
                Transform::from_xyz(0.0, 0.12, 0.0),
            ));
            m.spawn((
                Mesh3d(meshes.add(Cone::new(0.24, 0.34))),
                MeshMaterial3d(brown),
                Transform::from_xyz(0.0, 0.30, 0.0),
            ));
            m.spawn((
                Mesh3d(meshes.add(Sphere::new(0.55))),
                MeshMaterial3d(stink),
                Transform::from_xyz(0.0, 0.85, 0.0),
            ));
        });
}

fn update_city_event_prompt(
    stats: Res<PlayerStats>,
    boober: Res<BooberEatsState>,
    bomb: Res<LeadBombState>,
    chaos: Res<BarChaosState>,
    player_q: Query<&Transform, With<Player>>,
    npc_q: Query<&Transform, (With<CityNpc>, Without<Player>)>,
    mut prompt: ResMut<CityEventPrompt>,
) {
    prompt.label = None;
    prompt.kind = CityEventPromptKind::None;
    prompt.pressed = false;
    let Ok(p_tf) = player_q.get_single() else {
        return;
    };
    if stats.phone_open
        || stats.bartender_open
        || stats.char_select_open
        || stats.riding_wheel
        || stats.driving_car.is_some()
        || stats.riding_scooter.is_some()
        || p_tf.translation.y >= 3.6
    {
        return;
    }
    let flat = |a: Vec3| Vec2::new(a.x - p_tf.translation.x, a.z - p_tf.translation.z).length();
    let near_boober_pickup = flat(boober_counter_pos()) < 3.6 || flat(boober_curbside_pos()) < 4.2;
    let delivery_target = boober
        .target_npc
        .filter(|_| boober.stage == BooberStage::DeliverToNpc)
        .and_then(|e| npc_q.get(e).ok())
        .map(|t| t.translation);

    if bomb.active && flat(bomb.pos) < 5.0 {
        prompt.label = Some("DEFUSE LEAD BOMB [F/E]".to_string());
        prompt.kind = CityEventPromptKind::DefuseBomb;
    } else if chaos.shit_active && chaos.shit_is_local && flat(pub_shitter_pos()) < 3.6 {
        prompt.label = Some("USE SHITTER! [F/E]".to_string());
        prompt.kind = CityEventPromptKind::UseShitter;
    } else if let Some(t_pos) = delivery_target.filter(|&tp| flat(tp) < 6.4) {
        let _ = t_pos;
        prompt.label = Some(format!("DELIVER TO {} [F/E]", boober.customer_name.to_uppercase()));
        prompt.kind = CityEventPromptKind::BooberDeliver;
    } else if boober.stage != BooberStage::DeliverToNpc && near_boober_pickup {
        prompt.label = Some(if boober.on_shift {
            "GRAB NEXT ORDER [F/E]".to_string()
        } else {
            "START BOOBEREATS JOB [F/E]".to_string()
        });
        prompt.kind = CityEventPromptKind::BooberPickup;
    }
}

#[derive(SystemParam)]
struct EventHudQueries<'w, 's> {
    banner_q: Query<
        'w,
        's,
        (&'static mut Visibility, &'static mut BorderColor),
        With<CityEventBannerRoot>,
    >,
    banner_txt_q: Query<'w, 's, &'static mut Text, With<CityEventBannerText>>,
    end_btn_vis_q: Query<
        'w,
        's,
        &'static mut Visibility,
        (
            With<BooberEndShiftBtn>,
            Without<CityEventBannerRoot>,
            Without<BooberRadarDot>,
            Without<LeadBombRadarDot>,
        ),
    >,
    b_dot_q: Query<
        'w,
        's,
        (&'static mut Node, &'static mut Visibility),
        (
            With<BooberRadarDot>,
            Without<CityEventBannerRoot>,
            Without<LeadBombRadarDot>,
            Without<BooberEndShiftBtn>,
        ),
    >,
    l_dot_q: Query<
        'w,
        's,
        (&'static mut Node, &'static mut Visibility),
        (
            With<LeadBombRadarDot>,
            Without<CityEventBannerRoot>,
            Without<BooberRadarDot>,
            Without<BooberEndShiftBtn>,
        ),
    >,
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_city_event_hud_and_radar(
    mut commands: Commands,
    time: Res<Time>,
    stats: Res<PlayerStats>,
    boober: Res<BooberEatsState>,
    bomb: Res<LeadBombState>,
    chaos: Res<BarChaosState>,
    player_q: Query<&Transform, With<Player>>,
    npc_q: Query<&Transform, (With<CityNpc>, Without<Player>)>,
    radar_q: Query<&Parent, With<CityMinimapPlayerDot>>,
    mut hud: EventHudQueries,
) {
    // 1. Ensure BooberEats (cyan) & Lead Bomb (red) radar dots exist inside minimap
    if let Ok(radar_parent) = radar_q.get_single() {
        let map_ent = radar_parent.get();
        if hud.b_dot_q.is_empty() {
            commands.entity(map_ent).with_children(|m| {
                m.spawn((
                    CityEventEntity,
                    BooberRadarDot,
                    Node {
                        position_type: PositionType::Absolute,
                        width: Val::Px(10.0),
                        height: Val::Px(10.0),
                        margin: UiRect {
                            left: Val::Px(-5.0),
                            top: Val::Px(-5.0),
                            ..default()
                        },
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.15, 0.94, 1.0)),
                    BorderRadius::all(Val::Px(5.0)),
                    Visibility::Hidden,
                ));
            });
        }
        if hud.l_dot_q.is_empty() {
            commands.entity(map_ent).with_children(|m| {
                m.spawn((
                    CityEventEntity,
                    LeadBombRadarDot,
                    Node {
                        position_type: PositionType::Absolute,
                        width: Val::Px(12.0),
                        height: Val::Px(12.0),
                        margin: UiRect {
                            left: Val::Px(-6.0),
                            top: Val::Px(-6.0),
                            ..default()
                        },
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(1.0, 0.16, 0.12)),
                    BorderColor(Color::WHITE),
                    BorderRadius::all(Val::Px(6.0)),
                    Visibility::Hidden,
                ));
            });
        }
    }

    let to_pct = |v: f32| -> f32 {
        ((v + CITY_HALF_EXTENT) / (CITY_HALF_EXTENT * 2.0) * 100.0).clamp(3.0, 97.0)
    };
    let p_pos = player_q
        .get_single()
        .map(|t| t.translation)
        .unwrap_or(Vec3::ZERO);

    let boober_target_pos = boober
        .target_npc
        .filter(|_| boober.stage == BooberStage::DeliverToNpc)
        .and_then(|e| npc_q.get(e).ok())
        .map(|t| t.translation);
    let radar_boober_pos = boober_target_pos.or_else(|| {
        if boober.stage == BooberStage::PickupAt217 {
            Some(boober_curbside_pos())
        } else {
            None
        }
    });

    for (mut node, mut vis) in &mut hud.b_dot_q {
        if let Some(pos) = radar_boober_pos {
            *vis = Visibility::Visible;
            node.left = Val::Percent(to_pct(pos.x));
            node.top = Val::Percent(to_pct(pos.z));
        } else {
            *vis = Visibility::Hidden;
        }
    }

    let blink = (time.elapsed_secs() * 7.0).sin() > -0.25;
    for (mut node, mut vis) in &mut hud.l_dot_q {
        if bomb.active && blink {
            *vis = Visibility::Visible;
            node.left = Val::Percent(to_pct(bomb.pos.x));
            node.top = Val::Percent(to_pct(bomb.pos.z));
        } else {
            *vis = Visibility::Hidden;
        }
    }

    let modals_open = stats.char_select_open || stats.phone_open || stats.bartender_open;
    let mut banner_msg: Option<(String, Color)> = None;
    let mut show_end_shift = false;
    if !modals_open {
        if bomb.active {
            let d = Vec2::new(p_pos.x - bomb.pos.x, p_pos.z - bomb.pos.z).length();
            banner_msg = Some((
                format!(
                    "LEAD BOMB ALERT: {:.0}s LEFT | {:.0}m AWAY -- FIND RED BEAM & [F/E] TO DEFUSE!",
                    bomb.timer.ceil().max(0.0),
                    d
                ),
                Color::srgb(1.0, 0.24, 0.18),
            ));
        } else if chaos.shit_active && chaos.shit_is_local {
            banner_msg = Some((
                format!(
                    "CODE BROWN AT PUB 217: {:.0}s! SPRINT TO THE SW CORNER SHITTER!",
                    chaos.shit_timer.ceil().max(0.0)
                ),
                Color::srgb(0.65, 0.92, 0.22),
            ));
        } else if let Some(pos) = boober_target_pos {
            let delta = Vec2::new(pos.x - p_pos.x, pos.z - p_pos.z);
            let d = delta.length();
            let dir = compass_dir(delta);
            show_end_shift = true;
            banner_msg = Some((
                format!(
                    "BOOBEREATS: {} -> {} ({:.0}m {}) | {:.0}s | Job: {} (+{} TW)",
                    boober.order_name,
                    boober.customer_name,
                    d,
                    dir,
                    boober.timer.ceil().max(0.0),
                    boober.shift_deliveries,
                    boober.shift_earned_tw
                ),
                Color::srgb(0.18, 0.92, 1.0),
            ));
        } else if boober.on_shift && boober.stage == BooberStage::PickupAt217 {
            let curb = boober_curbside_pos();
            let delta = Vec2::new(curb.x - p_pos.x, curb.z - p_pos.z);
            let d = delta.length();
            let dir = compass_dir(delta);
            show_end_shift = true;
            banner_msg = Some((
                format!(
                    "BOOBEREATS SHIFT: Return to Pub 217 ({:.0}m {}) for next order | Done: {} (+{} TW)",
                    d,
                    dir,
                    boober.shift_deliveries,
                    boober.shift_earned_tw
                ),
                Color::srgb(1.0, 0.82, 0.22),
            ));
        }
    }

    for mut v in &mut hud.end_btn_vis_q {
        let want = if show_end_shift {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *v != want {
            *v = want;
        }
    }

    for (mut vis, mut border) in &mut hud.banner_q {
        if let Some((ref msg, col)) = banner_msg {
            *vis = Visibility::Visible;
            border.0 = col;
            if let Ok(mut txt) = hud.banner_txt_q.get_single_mut() {
                if txt.0 != *msg {
                    txt.0 = msg.clone();
                }
            }
        } else if *vis != Visibility::Hidden {
            *vis = Visibility::Hidden;
        }
    }
}

// =================================================================================================
// #22 Intense Pub 217 Stan Crashout Cutscene (`GameState::StanCrashoutCutscene`)
// =================================================================================================

const CRASHOUT_STAGE: Vec3 = Vec3::new(0.0, 62.0, -320.0);

#[derive(Resource, Default)]
struct CrashoutCutsceneState {
    timer: f32,
    beat: u8,
    done: bool,
}

#[derive(Component)]
struct CrashoutSceneEntity;

#[derive(Component)]
struct CrashoutOverlay;

#[derive(Component)]
struct CrashoutSubTitle;

#[derive(Component)]
struct CrashoutSubBody;

#[derive(Component)]
struct CrashoutStanActor;

#[derive(Component)]
struct CrashoutHecklerActor;

#[derive(Component)]
struct CrashoutFlipTable;

#[derive(Component)]
struct CrashoutRageLight;

#[derive(Component)]
struct CrashoutSaberBlade;

fn enter_crashout_cutscene(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cstate: ResMut<CrashoutCutsceneState>,
    mut cam_q: Query<(&mut Transform, &mut Projection), With<Camera3d>>,
) {
    set_touch_playing(false);
    crate::audio::set_touch_boss_mode(false);
    *cstate = CrashoutCutsceneState::default();
    let stage = CRASHOUT_STAGE;

    commands.insert_resource(ClearColor(Color::srgb(0.06, 0.02, 0.03)));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.85, 0.45, 0.42),
        brightness: 420.0,
    });

    // Pub 217 Bar Room Enclosure
    let floor_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.28, 0.16, 0.09),
        perceptual_roughness: 0.72,
        ..default()
    });
    let brick_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.32, 0.12, 0.10),
        perceptual_roughness: 0.85,
        ..default()
    });
    let wood_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.10, 0.06),
        perceptual_roughness: 0.38,
        ..default()
    });
    let glass_mug = materials.add(StandardMaterial {
        base_color: Color::srgb(0.98, 0.78, 0.22),
        emissive: LinearRgba::rgb(0.55, 0.35, 0.05),
        ..default()
    });
    commands.spawn((
        CrashoutSceneEntity,
        Mesh3d(meshes.add(Cuboid::new(16.0, 0.2, 16.0))),
        MeshMaterial3d(floor_mat),
        Transform::from_translation(stage + Vec3::new(0.0, -0.1, 0.0)),
    ));
    commands.spawn((
        CrashoutSceneEntity,
        Mesh3d(meshes.add(Cuboid::new(16.0, 7.5, 0.5))),
        MeshMaterial3d(brick_mat.clone()),
        Transform::from_translation(stage + Vec3::new(0.0, 3.7, -6.5)),
    ));
    commands.spawn((
        CrashoutSceneEntity,
        Mesh3d(meshes.add(Cuboid::new(0.5, 7.5, 16.0))),
        MeshMaterial3d(brick_mat),
        Transform::from_translation(stage + Vec3::new(-6.8, 3.7, 0.0)),
    ));
    // Bar Counter behind them
    commands.spawn((
        CrashoutSceneEntity,
        Mesh3d(meshes.add(Cuboid::new(8.5, 1.35, 1.4))),
        MeshMaterial3d(wood_mat.clone()),
        Transform::from_translation(stage + Vec3::new(0.0, 0.68, -4.6)),
    ));

    // Flip Table + Beer Mugs between Stan & Heckler
    commands
        .spawn((
            CrashoutSceneEntity,
            CrashoutFlipTable,
            Transform::from_translation(stage + Vec3::new(0.0, 0.0, 0.0)),
            Visibility::default(),
        ))
        .with_children(|tb| {
            tb.spawn((
                Mesh3d(meshes.add(Cuboid::new(1.85, 0.12, 1.15))),
                MeshMaterial3d(wood_mat),
                Transform::from_xyz(0.0, 0.92, 0.0),
            ));
            tb.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.09, 0.90))),
                MeshMaterial3d(glass_mug.clone()),
                Transform::from_xyz(0.0, 0.45, 0.0),
            ));
            for mx in [-0.42_f32, 0.40] {
                tb.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.09, 0.24))),
                    MeshMaterial3d(glass_mug.clone()),
                    Transform::from_xyz(mx, 1.10, 0.08),
                ));
            }
        });

    // Dramatic Spotlight + Rage Strobe Light
    commands.spawn((
        CrashoutSceneEntity,
        CrashoutRageLight,
        PointLight {
            color: Color::srgb(1.0, 0.72, 0.45),
            intensity: 260_000.0,
            range: 28.0,
            ..default()
        },
        Transform::from_translation(stage + Vec3::new(0.0, 5.2, 2.5)),
    ));

    // Stan (using plaid_hero.glb + glowing Saber Blade!)
    let hero_scene: Handle<Scene> = asset_server.load("models/plaid_hero.glb#Scene0");
    let saber_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.35, 1.0),
        emissive: LinearRgba::rgb(4.2, 0.8, 3.6),
        unlit: true,
        ..default()
    });
    commands
        .spawn((
            CrashoutSceneEntity,
            CrashoutStanActor,
            SceneRoot(hero_scene),
            Transform::from_translation(stage + Vec3::new(0.0, 0.0, 2.2))
                .with_rotation(Quat::from_rotation_y(0.0))
                .with_scale(Vec3::splat(1.28)),
        ))
        .with_children(|s| {
            s.spawn((
                CrashoutSaberBlade,
                Mesh3d(meshes.add(Cylinder::new(0.06, 2.1))),
                MeshMaterial3d(saber_mat),
                Transform::from_xyz(0.55, 1.45, -0.55).with_rotation(Quat::from_rotation_x(-1.1)),
                Visibility::Hidden,
            ));
        });

    // Obnoxious Bar Heckler in bright neon polo facing Stan
    let skin = materials.add(StandardMaterial {
        base_color: Color::srgb(0.94, 0.76, 0.64),
        ..default()
    });
    let polo = materials.add(StandardMaterial {
        base_color: Color::srgb(0.24, 0.78, 0.22),
        ..default()
    });
    let khakis = materials.add(StandardMaterial {
        base_color: Color::srgb(0.76, 0.66, 0.48),
        ..default()
    });
    commands
        .spawn((
            CrashoutSceneEntity,
            CrashoutHecklerActor,
            Transform::from_translation(stage + Vec3::new(0.0, 0.0, -2.0)),
            Visibility::default(),
        ))
        .with_children(|h| {
            for sx in [-0.14_f32, 0.14] {
                h.spawn((
                    Mesh3d(meshes.add(Capsule3d::new(0.11, 0.46))),
                    MeshMaterial3d(khakis.clone()),
                    Transform::from_xyz(sx, 0.42, 0.0),
                ));
            }
            h.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.32, 0.76))),
                MeshMaterial3d(polo.clone()),
                Transform::from_xyz(0.0, 1.06, 0.0),
            ));
            h.spawn((
                Mesh3d(meshes.add(Sphere::new(0.24))),
                MeshMaterial3d(skin.clone()),
                Transform::from_xyz(0.0, 1.72, 0.0),
            ));
            for sx in [-0.40_f32, 0.40] {
                h.spawn((
                    Mesh3d(meshes.add(Capsule3d::new(0.08, 0.45))),
                    MeshMaterial3d(polo.clone()),
                    Transform::from_xyz(sx, 1.25, 0.12).with_rotation(Quat::from_rotation_x(-0.6)),
                ));
            }
        });

    // Crew onlookers watching in Pub 217
    let glenn_mats = crate::glenn::make_glenn_materials(&mut materials);
    let nate_mats = crate::nate::make_nate_materials(&mut materials);
    let derek_mats = crate::derek::make_derek_materials(&mut materials);
    commands
        .spawn((
            CrashoutSceneEntity,
            Transform::from_translation(stage + Vec3::new(-3.8, 0.0, -1.6))
                .with_rotation(Quat::from_rotation_y(0.75))
                .with_scale(Vec3::splat(1.05)),
            Visibility::default(),
        ))
        .with_children(|c| spawn_glenn_rig(c, &mut meshes, &glenn_mats));
    commands
        .spawn((
            CrashoutSceneEntity,
            Transform::from_translation(stage + Vec3::new(3.8, 0.0, -1.4))
                .with_rotation(Quat::from_rotation_y(-0.75))
                .with_scale(Vec3::splat(1.05)),
            Visibility::default(),
        ))
        .with_children(|c| spawn_nate_rig(c, &mut meshes, &nate_mats, false));
    commands
        .spawn((
            CrashoutSceneEntity,
            Transform::from_translation(stage + Vec3::new(-4.4, 0.0, 1.8))
                .with_rotation(Quat::from_rotation_y(1.4))
                .with_scale(Vec3::splat(1.05)),
            Visibility::default(),
        ))
        .with_children(|c| spawn_derek_rig(c, &mut meshes, &derek_mats, false));
    let tj_e = spawn_tj_character(
        &mut commands,
        &mut meshes,
        &mut materials,
        stage + Vec3::new(4.3, 0.0, 1.9),
        -1.3,
        1.05,
        false,
    );
    commands.entity(tj_e).insert(CrashoutSceneEntity);

    // Camera + Letterbox Subtitle UI
    if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
        c_tf.translation = stage + Vec3::new(3.6, 2.2, 3.8);
        c_tf.look_at(stage + Vec3::new(0.0, 1.4, -0.5), Vec3::Y);
        if let Projection::Perspective(ref mut p) = *proj {
            p.fov = 0.68;
        }
    }

    commands
        .spawn((
            CrashoutOverlay,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(28.0),
                left: Val::Percent(10.0),
                width: Val::Percent(80.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(18.0), Val::Px(12.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.08, 0.02, 0.03, 0.90)),
            BorderColor(Color::srgb(1.0, 0.25, 0.20)),
            BorderRadius::all(Val::Px(10.0)),
        ))
        .with_children(|p| {
            p.spawn((
                CrashoutSubTitle,
                Text::new("BAR HECKLER @ PUB 217"),
                TextFont { font_size: 14.0, ..default() },
                TextColor(Color::srgb(0.45, 1.0, 0.45)),
            ));
            p.spawn((
                CrashoutSubBody,
                Text::new("\"Bro, Vikings are mid and boneless wings clear chicken tendies.\""),
                TextFont { font_size: 16.5, ..default() },
                TextColor(Color::WHITE),
            ));
        });

    play_cutscene_music();
    speak_voice("Bro, Vikings are mid and boneless wings clear chicken tendies.", "heckler");
}

#[derive(SystemParam)]
struct CrashoutSceneQueries<'w, 's> {
    stan_q: Query<'w, 's, &'static mut Transform, With<CrashoutStanActor>>,
    heckler_q: Query<
        'w,
        's,
        &'static mut Transform,
        (With<CrashoutHecklerActor>, Without<CrashoutStanActor>),
    >,
    table_q: Query<
        'w,
        's,
        &'static mut Transform,
        (With<CrashoutFlipTable>, Without<CrashoutStanActor>, Without<CrashoutHecklerActor>),
    >,
    light_q: Query<
        'w,
        's,
        &'static mut PointLight,
        (With<CrashoutRageLight>, Without<CrashoutStanActor>, Without<CrashoutHecklerActor>),
    >,
    saber_q: Query<
        'w,
        's,
        &'static mut Visibility,
        (With<CrashoutSaberBlade>, Without<CrashoutOverlay>),
    >,
    cam_q: Query<
        'w,
        's,
        (&'static mut Transform, &'static mut Projection),
        (
            With<Camera3d>,
            Without<CrashoutStanActor>,
            Without<CrashoutHecklerActor>,
            Without<CrashoutFlipTable>,
        ),
    >,
    sub_title_q: Query<
        'w,
        's,
        (&'static mut Text, &'static mut TextColor),
        (With<CrashoutSubTitle>, Without<CrashoutSubBody>),
    >,
    sub_body_q: Query<'w, 's, &'static mut Text, (With<CrashoutSubBody>, Without<CrashoutSubTitle>)>,
}

fn update_crashout_cutscene(
    time: Res<Time>,
    mut cstate: ResMut<CrashoutCutsceneState>,
    mut next_state: SceneTransition,
    mut qs: CrashoutSceneQueries,
) {
    let dt = time.delta_secs().min(0.05);
    cstate.timer += dt;
    if crate::skip::skipping() {
        cstate.timer = 13.8;
    }

    let beat = if cstate.timer < 2.6 {
        0
    } else if cstate.timer < 5.3 {
        1
    } else if cstate.timer < 8.0 {
        2
    } else if cstate.timer < 11.2 {
        3
    } else {
        4
    };

    if beat != cstate.beat {
        cstate.beat = beat;
        match beat {
            1 => {
                play_sfx("crashout_boom");
                play_sfx("killbill_siren");
                if let Ok((mut t, mut c)) = qs.sub_title_q.get_single_mut() {
                    t.0 = "STAN (PURE RAGE)".to_string();
                    c.0 = Color::srgb(1.0, 0.24, 0.20);
                }
                if let Ok(mut b) = qs.sub_body_q.get_single_mut() {
                    b.0 = "\"WHAT DID YOU JUST SAY ABOUT TENDIES AND THE VIKINGS?!\"".to_string();
                }
                speak_voice("What did you just say about tendies and the Vikings?!", "stan");
            }
            2 => {
                play_sfx("car_crash");
                play_sfx("saber_swing");
                if let Ok((mut t, mut c)) = qs.sub_title_q.get_single_mut() {
                    t.0 = "STAN CRASHING OUT!!".to_string();
                    c.0 = Color::srgb(1.0, 0.55, 0.15);
                }
                if let Ok(mut b) = qs.sub_body_q.get_single_mut() {
                    b.0 = "*FLIPS THE ENTIRE TABLE & IGNITES VIKING LIGHTSABER*".to_string();
                }
                speak_voice("Nobody disses tendies in Pub 217!", "stan");
            }
            3 => {
                play_sfx("saber_hit");
                play_sfx("explosion");
                if let Ok((mut t, mut c)) = qs.sub_title_q.get_single_mut() {
                    t.0 = "ORBITAL LAUNCH!!".to_string();
                    c.0 = Color::srgb(1.0, 0.85, 0.25);
                }
                if let Ok(mut b) = qs.sub_body_q.get_single_mut() {
                    b.0 = "\"GET OUTTA CEDAR RAPIDS!!\"  *LAUNCHED THROUGH THE ROOF*".to_string();
                }
                speak_voice("Get out of Cedar Rapids!", "stan");
            }
            4 => {
                play_sfx("win");
                if let Ok((mut t, mut c)) = qs.sub_title_q.get_single_mut() {
                    t.0 = "PUB 217 CREW".to_string();
                    c.0 = Color::srgb(0.45, 0.88, 1.0);
                }
                if let Ok(mut b) = qs.sub_body_q.get_single_mut() {
                    b.0 = "\"Note to self: NEVER insult tendies or Kirk Cousins in 217.\"".to_string();
                }
                speak_voice("Note to self: never insult tendies in 217!", "tj");
            }
            _ => {}
        }
    }

    let stage = CRASHOUT_STAGE;
    let t = cstate.timer;
    if let Ok(mut light) = qs.light_q.get_single_mut() {
        if beat == 0 {
            light.color = Color::srgb(1.0, 0.78, 0.50);
            light.intensity = 250_000.0;
        } else {
            let strobe = 0.7 + 0.3 * (t * 14.0).sin().abs();
            light.color = Color::srgb(1.0, 0.14 * strobe, 0.12 * strobe);
            light.intensity = 340_000.0 * strobe;
        }
    }
    for mut v in &mut qs.saber_q {
        *v = if beat >= 2 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    if let Ok(mut s_tf) = qs.stan_q.get_single_mut() {
        let shake = if beat == 1 || beat == 2 {
            (t * 55.0).sin() * 0.08
        } else {
            0.0
        };
        let z = if beat < 2 {
            2.2
        } else if beat == 2 {
            2.2 - ((t - 5.3) / 2.7).clamp(0.0, 1.0) * 2.4
        } else {
            -0.2
        };
        s_tf.translation = stage + Vec3::new(shake, 0.0, z);
    }

    if let Ok(mut tb_tf) = qs.table_q.get_single_mut() {
        if beat >= 2 {
            let u = (t - 5.3).max(0.0);
            tb_tf.translation = stage + Vec3::new(u * 4.5, (u * 5.2 - u * u * 3.5).max(0.0), -u * 3.0);
            tb_tf.rotation = Quat::from_rotation_x(u * 6.5) * Quat::from_rotation_z(u * 4.2);
        }
    }

    if let Ok(mut h_tf) = qs.heckler_q.get_single_mut() {
        if beat == 3 || beat == 4 {
            let u = (t - 8.0).max(0.0);
            h_tf.translation = stage + Vec3::new(u * 3.2, u * 16.0, -2.0 - u * 5.5);
            h_tf.rotation = Quat::from_rotation_x(u * 12.0) * Quat::from_rotation_y(u * 9.0);
        } else if beat >= 1 {
            h_tf.translation = stage + Vec3::new((t * 32.0).sin() * 0.06, 0.0, -2.2);
        }
    }

    if let Ok((mut c_tf, mut proj)) = qs.cam_q.get_single_mut() {
        let jitter = if beat == 1 || beat == 2 {
            Vec3::new((t * 48.0).sin() * 0.09, (t * 62.0).cos() * 0.07, 0.0)
        } else {
            Vec3::ZERO
        };
        match beat {
            0 => {
                c_tf.translation = stage + Vec3::new(2.8, 1.9, 1.2);
                c_tf.look_at(stage + Vec3::new(0.0, 1.5, -1.8), Vec3::Y);
            }
            1 => {
                c_tf.translation = stage + Vec3::new(0.45, 1.85, 0.35) + jitter;
                c_tf.look_at(stage + Vec3::new(0.0, 1.65, 2.2), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.50;
                }
            }
            2 => {
                c_tf.translation = stage + Vec3::new(-3.2, 1.35, 1.8) + jitter;
                c_tf.look_at(stage + Vec3::new(0.0, 1.3, 0.0), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.72;
                }
            }
            3 => {
                let u = ((t - 8.0) / 3.2).clamp(0.0, 1.0);
                c_tf.translation = stage + Vec3::new(3.5, 1.2, 2.2);
                c_tf.look_at(stage + Vec3::new(u * 2.0, 1.6 + u * 12.0, -2.0 - u * 4.0), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.78;
                }
            }
            _ => {
                c_tf.translation = stage + Vec3::new(0.0, 2.0, 5.2);
                c_tf.look_at(stage + Vec3::new(0.0, 1.5, -0.5), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.65;
                }
            }
        }
    }

    if !cstate.done && cstate.timer >= 13.4 && !is_voice_speaking() {
        cstate.done = true;
        next_state.set(GameState::CityPlay);
    } else if !cstate.done && cstate.timer >= 14.5 {
        cstate.done = true;
        next_state.set(GameState::CityPlay);
    }
}

fn exit_crashout_cutscene(
    mut commands: Commands,
    mut stats: ResMut<PlayerStats>,
    ent_q: Query<Entity, With<CrashoutSceneEntity>>,
    ov_q: Query<Entity, With<CrashoutOverlay>>,
    mut cam_q: Query<&mut Projection, With<Camera3d>>,
) {
    stop_voice();
    for e in &ent_q {
        commands.entity(e).despawn_recursive();
    }
    for e in &ov_q {
        commands.entity(e).despawn_recursive();
    }
    if let Ok(mut proj) = cam_q.get_single_mut() {
        if let Projection::Perspective(ref mut p) = *proj {
            p.fov = std::f32::consts::FRAC_PI_4;
        }
    }
    stats.add_tendy_wendies(20);
    stats.score += 1500;
    stats.dash_energy = 100.0;
    stats.speed_boost_timer = stats.speed_boost_timer.max(12.0);
    play_viking_chant_music();
}
