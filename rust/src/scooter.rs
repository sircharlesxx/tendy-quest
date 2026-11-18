use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use crate::audio::{
    is_voice_speaking, play_bgm, play_hacking_music, play_headphone_music, play_scooter_music,
    play_sfx, play_viking_chant_music, set_uzumaki_mode, speak_voice, touch_move_x, touch_move_y,
};
use crate::city::CityWorldMarker;
use crate::player::Player;
use crate::state::{CameraOrbit, GameState, LevelEntity, PlatformBox, PlayerStats};
use crate::ui::HudOverlay;

/// Retained from the original rental gate; scooters are now unlocked via the Veo phone app (`P`).
pub const SCOOTER_RENT_COST: u32 = 12;

/// Rideable Veo-style electric scooter. Vertical stunt state (`y`, `vy`, `ground_h`, `flip`,
/// `boost_t`) is stored directly on the scooter so `stunts.rs` can layer ramp launches and
/// backflips cleanly after `update_city_scooter_and_phone`.
#[derive(Component)]
pub struct CityScooter {
    pub yaw: f32,
    pub speed: f32,
    pub paid: bool,
    pub occupied: bool,
    // Stunt physics
    pub y: f32,
    pub vy: f32,
    pub grounded: bool,
    pub ground_h: f32,
    pub last_slope: f32,
    pub air_t: f32,
    pub flip: f32,
    pub boost_t: f32,
    // Hop (jump while riding), driven by `stunts.rs`
    /// Seconds a jump press stays queued, so a press just before touchdown still hops.
    pub jump_buf: f32,
    /// Pre-jump crouch countdown; the hop launches when it runs out.
    pub crouch_t: f32,
    /// This airtime started with (or got) a hop: no second hop until touchdown.
    pub hopped: bool,
    /// Space is still held from a flat-ground hop; backflips need a fresh press.
    pub hop_hold: bool,
    /// Ignores jump presses briefly after mounting (the phone unlock mounts on Space).
    pub jump_lock: f32,
    /// Squash & stretch spring for the hop crouch and landing bounce.
    pub squash: f32,
    pub squash_vel: f32,
    /// This airtime is a flat-ground hop (softer rise gravity, no flips, no trick score).
    pub flat_hop: bool,
    /// Riding on a car's hood or roof (`ground_h` came from a car, not a ramp).
    pub on_car: bool,
    /// This hop passed over a car: the landing scores a small stunt bonus.
    pub car_hop: bool,
}

impl CityScooter {
    pub fn new(yaw: f32) -> Self {
        Self {
            yaw,
            speed: 0.0,
            paid: false,
            occupied: false,
            y: 0.0,
            vy: 0.0,
            grounded: true,
            ground_h: 0.0,
            last_slope: 0.0,
            air_t: 0.0,
            flip: 0.0,
            boost_t: 0.0,
            jump_buf: 0.0,
            crouch_t: 0.0,
            hopped: false,
            hop_hold: false,
            jump_lock: 0.0,
            squash: 0.0,
            squash_vel: 0.0,
            flat_hop: false,
            on_car: false,
            car_hop: false,
        }
    }

    /// Clears hop state and arms the post-mount jump lock.
    pub fn reset_hop(&mut self) {
        self.jump_buf = 0.0;
        self.crouch_t = 0.0;
        self.hopped = false;
        self.hop_hold = false;
        self.jump_lock = 0.25;
        self.squash = 0.0;
        self.squash_vel = 0.0;
        self.flat_hop = false;
        self.on_car = false;
        self.car_hop = false;
    }
}

#[derive(Component)]
pub struct CityMike {
    pub was_inside: bool,
}

#[derive(Component)]
pub struct PhoneMazeUi;

#[derive(Component)]
pub struct ScootPromptUi;

#[derive(Component)]
pub struct ScootPromptButton;

#[derive(Component)]
pub struct ScootPromptText;

#[derive(SystemParam)]
pub struct ScootUiParams<'w, 's> {
    pub ui_q: Query<'w, 's, Entity, With<ScootPromptUi>>,
    pub btn_q: Query<'w, 's, &'static Interaction, (Changed<Interaction>, With<ScootPromptButton>)>,
    pub txt_q: Query<
        'w,
        's,
        &'static mut Text,
        (
            With<ScootPromptText>,
            Without<PhoneMazeStatusText>,
        ),
    >,
}

#[derive(Component)]
pub struct PhoneBezelNode;

#[derive(Component)]
pub struct VeoCursorNode;

#[derive(Component)]
pub struct VeoTargetNode;

#[derive(Component)]
pub struct VeoDriftButton;

#[derive(Component)]
pub struct PhoneMazeStatusText;

#[derive(SystemParam)]
pub struct VeoUiParams<'w, 's> {
    pub phone_ui_q: Query<'w, 's, Entity, With<PhoneMazeUi>>,
    pub bezel_q: Query<
        'w,
        's,
        &'static mut Node,
        (
            With<PhoneBezelNode>,
            Without<VeoCursorNode>,
            Without<VeoTargetNode>,
            Without<VeoDriftButton>,
        ),
    >,
    pub cursor_q: Query<
        'w,
        's,
        &'static mut Node,
        (
            With<VeoCursorNode>,
            Without<PhoneBezelNode>,
            Without<VeoTargetNode>,
            Without<VeoDriftButton>,
        ),
    >,
    pub target_q: Query<
        'w,
        's,
        &'static mut Node,
        (
            With<VeoTargetNode>,
            Without<PhoneBezelNode>,
            Without<VeoCursorNode>,
            Without<VeoDriftButton>,
        ),
    >,
    pub drift_btn_q: Query<'w, 's, &'static Interaction, (Changed<Interaction>, With<VeoDriftButton>)>,
    pub status_txt_q: Query<'w, 's, &'static mut Text, (With<PhoneMazeStatusText>, Without<ScootPromptText>)>,
}

#[derive(Resource)]
pub struct PhoneMazeState {
    pub taps_done: u32,
    pub cursor_pos: f32,
    pub cursor_dir: f32,
    pub target_pos: f32,
    pub wobble_t: f32,
    pub last_msg: String,
}

impl Default for PhoneMazeState {
    fn default() -> Self {
        Self {
            taps_done: 0,
            cursor_pos: 0.15,
            cursor_dir: 1.0,
            target_pos: 0.50,
            wobble_t: 0.0,
            last_msg: "20 BEERS DEEP: Align cursor in GREEN zone & press [SPACE] or tap button!"
                .to_string(),
        }
    }
}

pub fn spawn_city_scooters(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let unit_box = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let unit_cyl = meshes.add(Cylinder::new(1.0, 1.0));
    let deck_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.12, 0.14, 0.16),
        metallic: 0.4,
        ..default()
    });
    let lime_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.18, 0.92, 0.36),
        emissive: LinearRgba::new(0.12, 0.72, 0.25, 1.0),
        ..default()
    });
    let glow_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.15, 0.95, 0.85, 0.36),
        emissive: LinearRgba::new(0.1, 0.7, 0.65, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let tire_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.08, 0.10),
        ..default()
    });

    let spots = [
        (Vec3::new(-1.8, 0.0, 40.0), 0.25_f32),
        (Vec3::new(-4.5, 0.0, 46.0), -0.4),
        // Stunt course starts: 3rd St SE Gauntlet (north), Cross-Street Send (east), Greene Square Park
        (Vec3::new(1.8, 0.0, 33.0), std::f32::consts::PI),
        (Vec3::new(8.0, 0.0, 49.5), std::f32::consts::FRAC_PI_2),
        (Vec3::new(58.0, 0.0, 50.0), 0.0),
        (Vec3::new(42.0, 0.0, 42.0), 1.2),
        (Vec3::new(-42.0, 0.0, 42.0), -1.1),
        (Vec3::new(42.0, 0.0, 0.0), 1.57),
        (Vec3::new(-42.0, 0.0, 0.0), -1.57),
        (Vec3::new(0.0, 0.0, -42.0), 0.0),
        (Vec3::new(42.0, 0.0, -46.0), 2.4),
    ];

    let flame_mesh = meshes.add(Cone::new(0.2, 1.1));
    let flame_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.55, 0.12),
        emissive: LinearRgba::new(6.0, 2.4, 0.3, 1.0),
        unlit: true,
        ..default()
    });

    for (pos, yaw) in spots {
        commands
            .spawn((
                LevelEntity,
                CityWorldMarker,
                CityScooter::new(yaw),
                Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(yaw)),
            ))
            .with_children(|s| {
                s.spawn((
                    crate::stunts::ScooterBoostFlame,
                    Mesh3d(flame_mesh.clone()),
                    MeshMaterial3d(flame_mat.clone()),
                    Transform::from_xyz(0.0, 0.28, -1.3)
                        .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
                    Visibility::Hidden,
                ));
                s.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(deck_mat.clone()),
                    Transform::from_xyz(0.0, 0.24, 0.0).with_scale(Vec3::new(0.36, 0.08, 1.45)),
                ));
                s.spawn((
                    Mesh3d(unit_box.clone()),
                    MeshMaterial3d(lime_mat.clone()),
                    Transform::from_xyz(0.0, 0.25, 0.0).with_scale(Vec3::new(0.40, 0.05, 1.38)),
                ));
                s.spawn((
                    Mesh3d(unit_cyl.clone()),
                    MeshMaterial3d(lime_mat.clone()),
                    Transform::from_xyz(0.0, 0.88, 0.56).with_scale(Vec3::new(0.045, 1.22, 0.045)),
                ));
                s.spawn((
                    Mesh3d(unit_cyl.clone()),
                    MeshMaterial3d(deck_mat.clone()),
                    Transform::from_xyz(0.0, 1.48, 0.56)
                        .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2))
                        .with_scale(Vec3::new(0.04, 0.62, 0.04)),
                ));
                for wz in [0.62_f32, -0.62_f32] {
                    s.spawn((
                        Mesh3d(unit_cyl.clone()),
                        MeshMaterial3d(tire_mat.clone()),
                        Transform::from_xyz(0.0, 0.18, wz)
                            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2))
                            .with_scale(Vec3::new(0.18, 0.10, 0.18)),
                    ));
                }
                s.spawn((
                    Mesh3d(unit_cyl.clone()),
                    MeshMaterial3d(glow_mat.clone()),
                    Transform::from_xyz(0.0, 0.03, 0.0).with_scale(Vec3::new(1.05, 0.03, 1.05)),
                ));
            });
    }
}

fn spawn_mike_shirt_text(
    parent: &mut ChildBuilder,
    unit_box: &Handle<Mesh>,
    letter_mat: &Handle<StandardMaterial>,
    center_y: f32,
    z: f32,
) {
    // Bold 3D "M I K E" lettering across Mike's black shirt chest
    let bars: [(f32, f32, f32, f32, f32); 14] = [
        // M (left)
        (-0.23, 0.0, 0.025, 0.14, 0.0),
        (-0.14, 0.0, 0.025, 0.14, 0.0),
        (-0.205, 0.025, 0.022, 0.085, -0.55),
        (-0.165, 0.025, 0.022, 0.085, 0.55),
        // I
        (-0.075, 0.0, 0.026, 0.14, 0.0),
        // K
        (-0.005, 0.0, 0.025, 0.14, 0.0),
        (0.032, 0.032, 0.022, 0.085, -0.68),
        (0.032, -0.032, 0.022, 0.085, 0.68),
        // E (right)
        (0.115, 0.0, 0.025, 0.14, 0.0),
        (0.155, 0.058, 0.075, 0.024, 0.0),
        (0.148, 0.0, 0.062, 0.022, 0.0),
        (0.155, -0.058, 0.075, 0.024, 0.0),
        // Underline bar
        (-0.02, -0.105, 0.44, 0.02, 0.0),
        (-0.02, 0.105, 0.44, 0.02, 0.0),
    ];
    for (dx, dy, sx, sy, rz) in bars {
        parent.spawn((
            Mesh3d(unit_box.clone()),
            MeshMaterial3d(letter_mat.clone()),
            Transform::from_xyz(dx, center_y + dy, z)
                .with_rotation(Quat::from_rotation_z(rz))
                .with_scale(Vec3::new(sx, sy, 0.025)),
        ));
    }
}

/// Materials for Mike's shared body builder (city bar + alley cutscene keep their own tints).
struct MikeLook {
    skin: Handle<StandardMaterial>,
    beard: Handle<StandardMaterial>,
    tee: Handle<StandardMaterial>,
    eye_white: Handle<StandardMaterial>,
    eye_pupil: Handle<StandardMaterial>,
    cap: Handle<StandardMaterial>,
    jeans: Handle<StandardMaterial>,
}

/// Mike's body (black "MIKE" tee, jeans, light trimmed beard, red cap), facing +Z. One
/// builder for the Pub 217 bar version and the alley cutscene so both stay identical.
fn spawn_mike_body(m: &mut ChildBuilder, meshes: &mut Assets<Mesh>, unit_box: &Handle<Mesh>, look: &MikeLook) {
    use crate::shapes::{self as sh, Torso};
    let (hr, jaw) = (0.26_f32, 0.12_f32);
    // Legs + sneakers
    let leg = sh::add(meshes, sh::limb_profile(0.92, &[0.118, 0.11, 0.094], 1.0));
    let shoe = sh::add(meshes, sh::shoe(0.18, 0.10, 0.29));
    let sole = sh::add(meshes, sh::shoe_sole(0.18, 0.10, 0.29));
    for sx in [-0.15_f32, 0.15_f32] {
        m.spawn((Mesh3d(leg.clone()), MeshMaterial3d(look.jeans.clone()), Transform::from_xyz(sx, 0.53, 0.0)));
        m.spawn((Mesh3d(shoe.clone()), MeshMaterial3d(look.tee.clone()), Transform::from_xyz(sx, 0.05, 0.04)));
        m.spawn((Mesh3d(sole.clone()), MeshMaterial3d(look.eye_white.clone()), Transform::from_xyz(sx, 0.05, 0.04)));
    }
    // Black t-shirt torso (flat-fronted so the lettering sits on it) with bold "MIKE" lettering
    m.spawn((
        Mesh3d(sh::add(meshes, sh::torso(&Torso {
            height: 0.76,
            hip: Vec2::new(0.30, 0.17),
            waist: Vec2::new(0.30, 0.165),
            chest: Vec2::new(0.33, 0.178),
            shoulder: Vec2::new(0.34, 0.17),
            neck: 0.08,
            sq: 0.6,
            chest_fwd: 0.0,
        }))),
        MeshMaterial3d(look.tee.clone()),
        Transform::from_xyz(0.0, 1.30, 0.0),
    ));
    spawn_mike_shirt_text(m, unit_box, &look.eye_white, 1.40, 0.19);
    // Short tee sleeves, bare arms & hands
    let sleeve = sh::add(meshes, sh::limb(0.26, 0.09, 0.085));
    let arm = sh::add(meshes, sh::limb_profile(0.56, &[0.078, 0.072, 0.06], 1.0));
    for sx in [-1.0_f32, 1.0] {
        m.spawn((
            Mesh3d(sleeve.clone()),
            MeshMaterial3d(look.tee.clone()),
            Transform::from_xyz(sx * 0.40, 1.53, 0.0).with_rotation(Quat::from_rotation_z(sx * 0.10)),
        ));
        m.spawn((
            Mesh3d(arm.clone()),
            MeshMaterial3d(look.skin.clone()),
            Transform::from_xyz(sx * 0.425, 1.26, 0.02).with_rotation(Quat::from_rotation_z(sx * 0.05)),
        ));
        m.spawn((
            Mesh3d(sh::add(meshes, sh::hand(0.075, sx, false))),
            MeshMaterial3d(look.skin.clone()),
            Transform::from_xyz(sx * 0.435, 0.94, 0.03),
        ));
    }
    // Neck, head, ears & nose
    m.spawn((Mesh3d(sh::add(meshes, sh::limb(0.18, 0.078, 0.085))), MeshMaterial3d(look.skin.clone()), Transform::from_xyz(0.0, 1.68, -0.01)));
    m.spawn((Mesh3d(sh::add(meshes, sh::head(hr, jaw))), MeshMaterial3d(look.skin.clone()), Transform::from_xyz(0.0, 1.88, 0.0)));
    for sx in [-1.0_f32, 1.0] {
        m.spawn((Mesh3d(sh::add(meshes, sh::ear(0.115, sx))), MeshMaterial3d(look.skin.clone()), Transform::from_xyz(sx * 0.24, 1.88, -0.01)));
    }
    m.spawn((
        Mesh3d(sh::add(meshes, sh::superellipsoid(Vec3::new(0.042, 0.055, 0.046), 1.0, 1.0, 14, 10))),
        MeshMaterial3d(look.skin.clone()),
        Transform::from_xyz(0.0, 1.86, 0.25),
    ));
    // Eyes + pupils
    let eye = sh::add(meshes, sh::superellipsoid(Vec3::new(0.05, 0.05, 0.042), 1.0, 1.0, 20, 14));
    let pupil = sh::add(meshes, sh::superellipsoid(Vec3::new(0.027, 0.027, 0.016), 1.0, 1.0, 14, 10));
    for ex in [-0.09_f32, 0.09_f32] {
        m.spawn((Mesh3d(eye.clone()), MeshMaterial3d(look.eye_white.clone()), Transform::from_xyz(ex, 1.92, 0.212)));
        m.spawn((Mesh3d(pupil.clone()), MeshMaterial3d(look.eye_pupil.clone()), Transform::from_xyz(ex, 1.92, 0.252)));
    }
    // Light short trimmed beard hugging the jaw
    m.spawn((
        Mesh3d(sh::add(meshes, sh::beard(hr, jaw, 0.045, 0.05, -0.16))),
        MeshMaterial3d(look.beard.clone()),
        Transform::from_xyz(0.0, 1.88, 0.0),
    ));
    // Baseball Cap (crown hugging the head + curved front brim + button)
    m.spawn((Mesh3d(sh::add(meshes, sh::cap_crown(hr, 0.30, 0.075))), MeshMaterial3d(look.cap.clone()), Transform::from_xyz(0.0, 1.88, 0.0)));
    m.spawn((
        Mesh3d(sh::add(meshes, sh::superellipsoid(Vec3::new(0.025, 0.014, 0.025), 1.0, 1.0, 10, 6))),
        MeshMaterial3d(look.cap.clone()),
        Transform::from_xyz(0.0, 2.165, -0.01),
    ));
    m.spawn((
        Mesh3d(sh::add(meshes, sh::cap_brim(0.32, 0.26, 0.026, 0.022))),
        MeshMaterial3d(look.cap.clone()),
        Transform::from_xyz(0.0, 1.99, 0.15).with_rotation(Quat::from_rotation_x(0.10)),
    ));
}

pub fn spawn_city_mike(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    _seed: u64,
) {
    // Mike is inside Pub 217 at the bar!
    let pos = crate::city::mike_pub_spot();

    let unit_box = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let unit_cyl = meshes.add(Cylinder::new(1.0, 1.0));

    let skin = materials.add(StandardMaterial {
        base_color: Color::srgb(0.90, 0.74, 0.62),
        ..default()
    });
    let beard = materials.add(StandardMaterial {
        base_color: Color::srgb(0.46, 0.34, 0.24),
        ..default()
    });
    let wrestling_tee = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.08, 0.10),
        ..default()
    });
    let eye_white = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        ..default()
    });
    let eye_pupil = materials.add(StandardMaterial {
        base_color: Color::srgb(0.04, 0.04, 0.05),
        ..default()
    });
    let belt_gold = materials.add(StandardMaterial {
        base_color: Color::srgb(0.98, 0.80, 0.16),
        emissive: LinearRgba::new(0.45, 0.32, 0.04, 1.0),
        ..default()
    });
    let cap_red = materials.add(StandardMaterial {
        base_color: Color::srgb(0.76, 0.12, 0.14),
        ..default()
    });
    let jeans = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.24, 0.42),
        ..default()
    });
    let weed_ring = materials.add(StandardMaterial {
        base_color: Color::srgba(0.22, 0.92, 0.36, 0.35),
        emissive: LinearRgba::new(0.12, 0.8, 0.22, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });

    commands
        .spawn((
            LevelEntity,
            CityWorldMarker,
            CityMike { was_inside: false },
            Transform::from_translation(pos),
        ))
        .with_children(|m| {
            let look = MikeLook {
                skin: skin.clone(),
                beard: beard.clone(),
                tee: wrestling_tee.clone(),
                eye_white: eye_white.clone(),
                eye_pupil: eye_pupil.clone(),
                cap: cap_red.clone(),
                jeans: jeans.clone(),
            };
            spawn_mike_body(m, meshes, &unit_box, &look);
            // Alley weed aura ring
            m.spawn((
                Mesh3d(unit_cyl.clone()),
                MeshMaterial3d(weed_ring),
                Transform::from_xyz(0.0, 0.04, 0.0).with_scale(Vec3::new(1.5, 0.03, 1.5)),
            ));
        });
}

fn spawn_phone_maze_ui(commands: &mut Commands, maze: &PhoneMazeState) {
    commands
        .spawn((
            PhoneMazeUi,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.02, 0.03, 0.60)),
        ))
        .with_children(|root| {
            root.spawn((
                PhoneBezelNode,
                Node {
                    width: Val::Px(330.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(16.0)),
                    row_gap: Val::Px(12.0),
                    border: UiRect::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.05, 0.07, 0.09)),
                BorderColor(Color::srgb(0.18, 0.92, 0.62)),
                BorderRadius::all(Val::Px(22.0)),
            ))
            .with_children(|phone| {
                phone.spawn((
                    Text::new("VEO.APP // 20 BEERS IN"),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.22, 0.98, 0.64)),
                ));
                phone.spawn((
                    PhoneMazeStatusText,
                    Text::new(format!("STEP {}/3: {}", maze.taps_done, maze.last_msg)),
                    TextFont {
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.95, 0.92, 0.62)),
                ));

                // Drunk timing slider bar
                phone
                    .spawn((
                        Node {
                            width: Val::Px(270.0),
                            height: Val::Px(28.0),
                            position_type: PositionType::Relative,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.12, 0.14, 0.18)),
                        BorderRadius::all(Val::Px(8.0)),
                    ))
                    .with_children(|bar| {
                        // Green sweet-spot zone (26% wide)
                        bar.spawn((
                            VeoTargetNode,
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Percent(37.0),
                                width: Val::Percent(26.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.16, 0.85, 0.38, 0.55)),
                            BorderRadius::all(Val::Px(6.0)),
                        ));
                        // Drunk jittering cyan cursor
                        bar.spawn((
                            VeoCursorNode,
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Percent(15.0),
                                width: Val::Px(12.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.25, 0.98, 1.0)),
                            BorderRadius::all(Val::Px(4.0)),
                        ));
                    });

                phone
                    .spawn((
                        VeoDriftButton,
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(18.0), Val::Px(10.0)),
                            border: UiRect::all(Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.12, 0.62, 0.32)),
                        BorderColor(Color::srgb(0.45, 1.0, 0.68)),
                        BorderRadius::all(Val::Px(10.0)),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("UNLOCK VEO [SPACE / CLICK]"),
                            TextFont {
                                font_size: 13.5,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });

                phone.spawn((
                    Text::new("[P / ESC] Put Phone Away  |  FREE RIDE ONCE UNLOCKED!"),
                    TextFont {
                        font_size: 10.5,
                        ..default()
                    },
                    TextColor(Color::srgb(0.75, 0.82, 0.85)),
                ));
            });
        });
}

pub fn update_city_scooter_and_phone(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut stats: ResMut<PlayerStats>,
    mut orbit: ResMut<CameraOrbit>,
    mut maze: ResMut<PhoneMazeState>,
    mut next_state: crate::transition::SceneTransition,
    mut player_q: Query<
        (&mut Transform, &mut Player),
        (Without<CityScooter>, Without<PlatformBox>, Without<CityMike>),
    >,
    mut scooter_q: Query<
        (Entity, &mut Transform, &mut CityScooter),
        (Without<Player>, Without<PlatformBox>, Without<CityMike>),
    >,
    mut mike_q: Query<
        (&mut Transform, &mut CityMike),
        (Without<Player>, Without<CityScooter>, Without<PlatformBox>),
    >,
    platform_q: Query<
        (&Transform, &PlatformBox),
        (Without<Player>, Without<CityScooter>, Without<CityMike>),
    >,
    mut veo_ui: VeoUiParams,
    mut scoot_ui: ScootUiParams,
) {
    let dt = time.delta_secs().min(0.05);
    let Ok((mut p_tf, mut player)) = player_q.get_single_mut() else {
        return;
    };

    // Toggle phone with [P]
    if keys.just_pressed(KeyCode::KeyP) && stats.driving_car.is_none() && stats.riding_scooter.is_none() {
        if stats.phone_open {
            stats.phone_open = false;
            for e in &veo_ui.phone_ui_q {
                commands.entity(e).despawn_recursive();
            }
            if stats.in_city {
                play_viking_chant_music();
            } else {
                play_bgm();
            }
            return;
        } else if !stats.scooter_app_unlocked {
            stats.phone_open = true;
            *maze = PhoneMazeState::default();
            spawn_phone_maze_ui(&mut commands, &maze);
            play_hacking_music();
            return;
        }
    }

    // Handle active 20-Beers Veo App Unlock Minigame
    if stats.phone_open {
        if keys.just_pressed(KeyCode::Escape) {
            stats.phone_open = false;
            for e in &veo_ui.phone_ui_q {
                commands.entity(e).despawn_recursive();
            }
            if stats.in_city {
                play_viking_chant_music();
            } else {
                play_bgm();
            }
            return;
        }

        maze.wobble_t += dt;
        let speed = 1.25 + (maze.taps_done as f32) * 0.25;
        maze.cursor_pos += maze.cursor_dir * speed * dt;
        // Drunk thumb jitter
        maze.cursor_pos += (maze.wobble_t * 7.5).sin() * 0.35 * dt;
        if maze.cursor_pos >= 0.95 {
            maze.cursor_pos = 0.95;
            maze.cursor_dir = -1.0;
        } else if maze.cursor_pos <= 0.02 {
            maze.cursor_pos = 0.02;
            maze.cursor_dir = 1.0;
        }

        // Drunk swaying phone bezel + moving target zone
        if let Ok(mut bn) = veo_ui.bezel_q.get_single_mut() {
            bn.margin = UiRect {
                left: Val::Px((maze.wobble_t * 2.4).sin() * 38.0),
                top: Val::Px((maze.wobble_t * 3.1).cos() * 24.0),
                ..default()
            };
        }
        if let Ok(mut cn) = veo_ui.cursor_q.get_single_mut() {
            cn.left = Val::Percent((maze.cursor_pos * 100.0).clamp(0.0, 95.0));
        }
        if let Ok(mut tn) = veo_ui.target_q.get_single_mut() {
            tn.left = Val::Percent(((maze.target_pos - 0.13) * 100.0).clamp(0.0, 74.0));
        }

        let btn_tapped = veo_ui.drift_btn_q.iter().any(|i| *i == Interaction::Pressed);
        let attempt = keys.just_pressed(KeyCode::Space)
            || keys.just_pressed(KeyCode::Enter)
            || keys.just_pressed(KeyCode::KeyF)
            || keys.just_pressed(KeyCode::KeyE)
            || btn_tapped;

        if attempt {
            let in_zone = (maze.cursor_pos - maze.target_pos).abs() <= 0.15;
            if in_zone {
                maze.taps_done += 1;
                if maze.taps_done >= 3 {
                    stats.scooter_app_unlocked = true;
                    stats.phone_open = false;
                    for e in &veo_ui.phone_ui_q {
                        commands.entity(e).despawn_recursive();
                    }
                    play_sfx("hack_success");
                    let mut mounted = false;
                    for (se, stf, mut sc) in &mut scooter_q {
                        if p_tf.translation.distance(stf.translation) < 4.5 {
                            sc.paid = true;
                            sc.occupied = true;
                            sc.reset_hop();
                            stats.riding_scooter = Some(se);
                            p_tf.translation = stf.translation + Vec3::Y * 0.25;
                            player.velocity = Vec3::ZERO;
                            play_scooter_music();
                            stats.hint_text =
                                "SCOOTER UNLOCKED! [WASD] Zip | [SHIFT] Turbo | [SPACE] Jump, hold in air to Backflip | [F/E] Hop Off"
                                    .to_string();
                            mounted = true;
                            break;
                        }
                    }
                    if !mounted {
                        if stats.in_city {
                play_viking_chant_music();
            } else {
                play_bgm();
            }
                        stats.hint_text =
                            "VEO APP UNLOCKED! Grab a scooter, hit the VEO STUNT GATES (north + east) & Greene Square!"
                                .to_string();
                    }
                    return;
                }
                play_sfx("hack_success");
                maze.target_pos = if maze.taps_done == 1 { 0.30 } else { 0.72 };
                maze.last_msg = if maze.taps_done == 1 {
                    "QR SCANNED! Now confirm you're sober... (2/3)"
                } else {
                    "ONE EYE CLOSED! Tap PIN confirm! (3/3)"
                }
                .to_string();
            } else {
                play_sfx("hack_deny");
                maze.last_msg = "MISSED! Too many Busch Lights! Try again!".to_string();
            }
            if let Ok(mut st) = veo_ui.status_txt_q.get_single_mut() {
                st.0 = format!("STEP {}/3: {}", maze.taps_done, maze.last_msg);
            }
        }
        return;
    }

    let scoot_btn_clicked = scoot_ui.btn_q.iter().any(|i| *i == Interaction::Pressed);
    let interact = keys.just_pressed(KeyCode::KeyF) || keys.just_pressed(KeyCode::KeyE) || scoot_btn_clicked;

    // Check Mike inside Pub 217 (fixed pub size) and auto-trigger the Uzumaki cutscene when bumped!
    let pub_s = crate::city::PUB217_SCALE;
    for (mut m_tf, mut mike) in &mut mike_q {
        m_tf.scale = Vec3::splat(pub_s);
        m_tf.translation = crate::city::mike_pub_spot();
        let d = Vec2::new(
            m_tf.translation.x - p_tf.translation.x,
            m_tf.translation.z - p_tf.translation.z,
        )
        .length();
        let trigger_r = 1.85 * pub_s.sqrt();
        if d > 3.6 * pub_s.sqrt() {
            mike.was_inside = false;
        }
        let on_ground = p_tf.translation.y < 4.0;
        if stats.free_roam {
            // Post-game free roam: Mike just hangs out (no smoke-break replay), says hi once.
            if on_ground && d < trigger_r && !mike.was_inside {
                mike.was_inside = true;
                stats.hint_text = "MIKE: \"Smoke break's over, legend. Go ride.\"".to_string();
                stats.hint_bump = stats.hint_bump.wrapping_add(1);
            }
            continue;
        }
        if on_ground && d < trigger_r && !mike.was_inside && stats.driving_car.is_none() && stats.riding_scooter.is_none() {
            mike.was_inside = true;
            stats.saved_city_pos = p_tf.translation;
            stats.mike_return_city = true;
            next_state.set(GameState::MikeCutscene);
            return;
        } else if on_ground && d < 4.2 && stats.driving_car.is_none() && stats.riding_scooter.is_none() {
            if interact {
                mike.was_inside = true;
                stats.saved_city_pos = p_tf.translation;
                stats.mike_return_city = true;
                next_state.set(GameState::MikeCutscene);
                return;
            } else {
                stats.hint_text = "Bump Mike or press [F / E] for Uzumaki Alley Wisdom!".to_string();
            }
        }
    }

    // On foot: check nearby scooter
    if stats.riding_scooter.is_none() {
        if stats.driving_car.is_some() || stats.player_scale >= 6.0 {
            for e in &scoot_ui.ui_q {
                commands.entity(e).despawn_recursive();
            }
            return;
        }
        let mut nearest: Option<(Entity, f32)> = None;
        for (se, stf, _) in &scooter_q {
            let d = Vec2::new(
                stf.translation.x - p_tf.translation.x,
                stf.translation.z - p_tf.translation.z,
            )
            .length();
            if d < 5.2 && nearest.map_or(true, |(_, bd)| d < bd) {
                nearest = Some((se, d));
            }
        }

        if let Some((se, _)) = nearest {
            let btn_label = if !stats.scooter_app_unlocked {
                "SCOOT! (HACK PHONE APP [F/E])".to_string()
            } else {
                "SCOOT! [F/E]".to_string()
            };

            if scoot_ui.ui_q.is_empty() {
                commands
                    .spawn((
                        ScootPromptUi,
                        Node {
                            position_type: PositionType::Absolute,
                            bottom: Val::Px(26.0),
                            width: Val::Percent(100.0),
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                    ))
                    .with_children(|root| {
                        root.spawn((
                            ScootPromptButton,
                            Button,
                            Node {
                                padding: UiRect::axes(Val::Px(20.0), Val::Px(10.0)),
                                border: UiRect::all(Val::Px(2.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.08, 0.72, 0.30)),
                            BorderColor(Color::srgb(0.45, 1.0, 0.65)),
                            BorderRadius::all(Val::Px(10.0)),
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                ScootPromptText,
                                Text::new(btn_label.clone()),
                                TextFont {
                                    font_size: 16.0,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));
                        });
                    });
            } else if let Ok(mut st) = scoot_ui.txt_q.get_single_mut() {
                if st.0 != btn_label {
                    st.0 = btn_label;
                }
            }

            if !stats.scooter_app_unlocked {
                if interact {
                    for e in &scoot_ui.ui_q {
                        commands.entity(e).despawn_recursive();
                    }
                    stats.phone_open = true;
                    *maze = PhoneMazeState::default();
                    spawn_phone_maze_ui(&mut commands, &maze);
                    play_hacking_music();
                } else {
                    stats.hint_text =
                        "SCOOTER LOCKED! Click [SCOOT!] or press [P] to hack SCOOTR.OS!"
                            .to_string();
                }
            } else if let Ok((_, stf, mut sc)) = scooter_q.get_mut(se) {
                if interact {
                    sc.paid = true;
                    sc.occupied = true;
                    sc.reset_hop();
                    stats.riding_scooter = Some(se);
                    p_tf.translation = stf.translation + Vec3::Y * 0.25;
                    player.velocity = Vec3::ZERO;
                    play_sfx("hack_success");
                    play_scooter_music();
                    stats.hint_text =
                        "RIDING! [WASD] Zip | [SHIFT] Turbo | [SPACE] Jump, hold in air to Backflip | [F/E] Hop Off"
                            .to_string();
                } else {
                    stats.hint_text = "Click [SCOOT!] or press [F / E] to Ride Scooter".to_string();
                }
            }
        } else {
            for e in &scoot_ui.ui_q {
                commands.entity(e).despawn_recursive();
            }
        }
        return;
    }

    if let Ok(mut st) = scoot_ui.txt_q.get_single_mut() {
        let lbl = "HOP OFF SCOOTER [F/E]";
        if st.0 != lbl {
            st.0 = lbl.to_string();
        }
    }

    let Some(se) = stats.riding_scooter else {
        return;
    };

    let airborne = scooter_q
        .get(se)
        .map(|(_, _, sc)| !sc.grounded)
        .unwrap_or(false);

    if interact && !airborne {
        if let Ok((_, mut stf, mut sc)) = scooter_q.get_mut(se) {
            sc.occupied = false;
            sc.speed = 0.0;
            sc.y = 0.0;
            sc.vy = 0.0;
            sc.grounded = true;
            sc.flip = 0.0;
            sc.air_t = 0.0;
            sc.boost_t = 0.0;
            sc.reset_hop();
            stf.translation.y = 0.0;
            stf.rotation = Quat::from_rotation_y(sc.yaw);
            stf.scale = Vec3::ONE;
            p_tf.translation = Vec3::new(stf.translation.x + 1.4, 0.0, stf.translation.z);
            p_tf.rotation = Quat::from_rotation_y(sc.yaw + std::f32::consts::PI);
        }
        stats.riding_scooter = None;
        play_sfx("car_door");
        if stats.in_city {
            play_viking_chant_music();
        } else {
            play_bgm();
        }
        stats.hint_text = "HOPPED OFF SCOOTER!".to_string();
        return;
    }

    if let Ok((_, mut stf, mut sc)) = scooter_q.get_mut(se) {
        let mut throttle = touch_move_y();
        if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
            throttle += 1.0;
        }
        if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
            throttle -= 1.0;
        }
        throttle = throttle.clamp(-1.0, 1.0);
        let mut steer = -touch_move_x();
        if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
            steer += 1.0;
        }
        if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
            steer -= 1.0;
        }
        if stats.drunk_timer > 0.0 && throttle.abs() > 0.1 {
            // Beer: a gentle wandering bias you have to steer against.
            steer += crate::player::drunk_steer_drift(time.elapsed_secs(), stats.drunk_timer) * 0.22;
        }
        steer = steer.clamp(-1.0, 1.0);

        let turbo = (keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight))
            && stats.dash_energy > 2.0
            && throttle > 0.1;
        if turbo {
            stats.dash_energy = (stats.dash_energy - 28.0 * dt).max(0.0);
        }

        let boosting = sc.boost_t > 0.0;
        if boosting {
            sc.boost_t = (sc.boost_t - dt).max(0.0);
            if throttle > -0.05 {
                throttle = 1.0;
            }
        }
        let max_spd = if boosting {
            58.0
        } else if turbo {
            42.0
        } else {
            30.0
        };
        let accel = if boosting { 95.0 } else { 36.0 };
        if throttle > 0.05 {
            if sc.speed > max_spd {
                // Coming down from a boost: bleed speed off smoothly instead of snapping
                sc.speed = (sc.speed - 18.0 * dt).max(max_spd);
            } else {
                sc.speed = (sc.speed + accel * dt).min(max_spd);
            }
        } else if throttle < -0.05 {
            sc.speed = (sc.speed - 38.0 * dt).max(-12.0);
        } else {
            sc.speed *= (1.0 - 4.0 * dt).max(0.0);
        }

        let turn_scale = (sc.speed.abs() / 6.0).clamp(0.25, 1.0);
        sc.yaw += steer * 3.1 * turn_scale * dt;

        let fwd = Vec2::new(sc.yaw.sin(), sc.yaw.cos());
        stf.translation.y = sc.y;

        // Sub-stepped move + wall push-out: at boost speed (~58 m/s) one frame can travel several
        // times the 0.82 collision radius, and the shallowest-axis push-out would then pop the
        // scooter out the far side of a thin wall. Steps stay under half the radius.
        let wall_r = 0.82;
        let substeps = ((sc.speed.abs() * dt / (wall_r * 0.5)).ceil() as u32).clamp(1, 16);
        let sub_dt = dt / substeps as f32;
        let mut bumped = false;
        for _ in 0..substeps {
            stf.translation.x = (stf.translation.x + fwd.x * sc.speed * sub_dt).clamp(-126.0, 126.0);
            stf.translation.z = (stf.translation.z + fwd.y * sc.speed * sub_dt).clamp(-126.0, 126.0);
            for (b_tf, b_box) in &platform_q {
                let top_y = b_tf.translation.y + b_box.half_extents.y;
                let bot_y = b_tf.translation.y - b_box.half_extents.y;
                if top_y <= 0.25 || top_y < sc.y + 0.1 || bot_y > sc.y + 2.0 {
                    continue;
                }
                let min_x = b_tf.translation.x - b_box.half_extents.x - wall_r;
                let max_x = b_tf.translation.x + b_box.half_extents.x + wall_r;
                let min_z = b_tf.translation.z - b_box.half_extents.z - wall_r;
                let max_z = b_tf.translation.z + b_box.half_extents.z + wall_r;
                if stf.translation.x > min_x
                    && stf.translation.x < max_x
                    && stf.translation.z > min_z
                    && stf.translation.z < max_z
                {
                    let dx_l = stf.translation.x - min_x;
                    let dx_r = max_x - stf.translation.x;
                    let dz_b = stf.translation.z - min_z;
                    let dz_f = max_z - stf.translation.z;
                    let m = dx_l.min(dx_r).min(dz_b).min(dz_f);
                    if (m - dx_l).abs() < 0.001 {
                        stf.translation.x = min_x;
                    } else if (m - dx_r).abs() < 0.001 {
                        stf.translation.x = max_x;
                    } else if (m - dz_b).abs() < 0.001 {
                        stf.translation.z = min_z;
                    } else {
                        stf.translation.z = max_z;
                    }
                    // Same 0.35x speed loss per wall contact as before, applied once per frame.
                    if !bumped {
                        sc.speed *= 0.35;
                        bumped = true;
                    }
                }
            }
        }

        let lean = (-steer * sc.speed * 0.006).clamp(-0.16, 0.16);
        stf.rotation = Quat::from_rotation_y(sc.yaw) * Quat::from_rotation_z(lean);

        p_tf.translation = stf.translation + Vec3::Y * 0.25;
        p_tf.rotation = Quat::from_rotation_y(sc.yaw + std::f32::consts::PI);

        if sc.speed.abs() > 2.0 {
            let target_cam_yaw = sc.yaw + std::f32::consts::PI;
            let mut diff = (target_cam_yaw - orbit.yaw) % std::f32::consts::TAU;
            if diff > std::f32::consts::PI {
                diff -= std::f32::consts::TAU;
            } else if diff < -std::f32::consts::PI {
                diff += std::f32::consts::TAU;
            }
            // Paused for ~1.5 s after manual look input, then eases back in.
            orbit.yaw += diff * (1.0 - (-3.0 * dt).exp()) * orbit.auto_align_weight();
        }
    }
}

#[derive(Component)]
pub struct MikeCutsceneEntity;

#[derive(Component)]
pub struct MikeCutsceneOverlay;

#[derive(Component)]
pub struct MikeCutsceneSpeakerText;

#[derive(Component)]
pub struct MikeCutsceneSubtitleText;

#[derive(Component)]
pub struct MikeCutsceneSmoke {
    pub base_pos: Vec3,
    pub speed: f32,
    pub phase: f32,
}

#[derive(Resource, Default)]
pub struct MikeCutsceneDialog {
    pub variant: usize,
}

const MIKE_BACKSTORIES: [(&str, &str); 4] = [
    (
        "Are you sure you want this, traveler? Look at the bricks behind Pub 217... Cedar Rapids is built inside a giant spiral. Smoke this and see the truth.",
        "Every street... every tendy... it's all twisting into a spiral, Mike...",
    ),
    (
        "Are you sure you want this, traveler? Time coils inward behind this alley. Take a hit—the spiral will heal your wounds, if your mind survives it.",
        "I can hear the spirals spinning in my skull... thanks for the wisdom, Mike.",
    ),
    (
        "Are you sure you want this, traveler? My championship belt was forged at the center of the Cedar River vortex... inhale the sacred alley smoke.",
        "The ink... the spirals... everything in Downtown Cedar Rapids is connected!",
    ),
    (
        "Are you sure you want this, traveler? Once you step into the spiral behind Pub 217, you never truly leave. Let the smoke mend your body.",
        "My hearts are healed, but the spirals won't stop turning...",
    ),
];

pub fn enter_mike_cutscene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut stats: ResMut<PlayerStats>,
    mut dialog: ResMut<MikeCutsceneDialog>,
    mut hud_q: Query<&mut Node, With<HudOverlay>>,
    mut cam_q: Query<(&mut Transform, &mut Projection), With<Camera3d>>,
) {
    for mut n in &mut hud_q {
        n.display = Display::None;
    }

    let mut rbytes = [0u8; 4];
    let _ = getrandom::getrandom(&mut rbytes);
    dialog.variant = (u32::from_le_bytes(rbytes) as usize) % MIKE_BACKSTORIES.len();
    stats.cutscene_timer = 0.0;
    stats.cutscene_beat = 0;

    // Intense Uzumaki stark black-and-white manga ink aesthetic
    set_uzumaki_mode(2);
    commands.insert_resource(ClearColor(Color::srgb(0.02, 0.02, 0.02)));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.95, 0.95, 0.95),
        brightness: 1100.0,
    });

    let stage = Vec3::new(0.0, 0.0, -210.0);
    let unit_box = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let unit_sphere = meshes.add(Sphere::new(1.0));
    let spiral_torus = meshes.add(Torus::new(0.10, 1.0));

    let brick = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.08, 0.08),
        ..default()
    });
    let floor_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.04, 0.04, 0.04),
        ..default()
    });
    let smoke_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.92, 0.92, 0.92, 0.42),
        emissive: LinearRgba::new(0.75, 0.75, 0.75, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let spiral_white = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        emissive: LinearRgba::new(1.6, 1.6, 1.6, 1.0),
        unlit: true,
        ..default()
    });

    // Alley floor & brick walls
    commands.spawn((
        MikeCutsceneEntity,
        Mesh3d(unit_box.clone()),
        MeshMaterial3d(floor_mat),
        Transform::from_translation(stage + Vec3::new(0.0, -0.25, 0.0))
            .with_scale(Vec3::new(16.0, 0.5, 16.0)),
    ));
    for sx in [-4.5_f32, 4.5_f32] {
        commands.spawn((
            MikeCutsceneEntity,
            Mesh3d(unit_box.clone()),
            MeshMaterial3d(brick.clone()),
            Transform::from_translation(stage + Vec3::new(sx, 3.5, 0.0))
                .with_scale(Vec3::new(1.0, 7.5, 16.0)),
        ));
    }

    // Spawn Mike (beard, red baseball cap, black wrestling shirt + gold belt)
    let skin = materials.add(StandardMaterial {
        base_color: Color::srgb(0.90, 0.74, 0.62),
        ..default()
    });
    let beard = materials.add(StandardMaterial {
        base_color: Color::srgb(0.46, 0.34, 0.24),
        ..default()
    });
    let wrestling_tee = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.08, 0.10),
        ..default()
    });
    let eye_white = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        ..default()
    });
    let eye_pupil = materials.add(StandardMaterial {
        base_color: Color::srgb(0.04, 0.04, 0.05),
        ..default()
    });
    let belt_gold = materials.add(StandardMaterial {
        base_color: Color::srgb(0.98, 0.80, 0.16),
        emissive: LinearRgba::new(0.5, 0.35, 0.05, 1.0),
        ..default()
    });
    let cap_red = materials.add(StandardMaterial {
        base_color: Color::srgb(0.82, 0.14, 0.16),
        ..default()
    });
    let jeans = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.24, 0.42),
        ..default()
    });
    let plaid_red = materials.add(StandardMaterial {
        base_color: Color::srgb(0.76, 0.14, 0.16),
        ..default()
    });

    // Mike facing Stan
    commands
        .spawn((
            MikeCutsceneEntity,
            Transform::from_translation(stage + Vec3::new(0.0, 0.0, -1.6)),
        ))
        .with_children(|m| {
            let look = MikeLook {
                skin: skin.clone(),
                beard: beard.clone(),
                tee: wrestling_tee.clone(),
                eye_white: eye_white.clone(),
                eye_pupil: eye_pupil.clone(),
                cap: cap_red.clone(),
                jeans: jeans.clone(),
            };
            spawn_mike_body(m, &mut meshes, &unit_box, &look);
        });

    // Stan silhouette facing Mike
    commands
        .spawn((
            MikeCutsceneEntity,
            Transform::from_translation(stage + Vec3::new(0.0, 0.0, 1.6))
                .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
        ))
        .with_children(|s| {
            s.spawn((
                Mesh3d(unit_box.clone()),
                MeshMaterial3d(plaid_red),
                Transform::from_xyz(0.0, 1.1, 0.0).with_scale(Vec3::new(0.75, 1.3, 0.45)),
            ));
            s.spawn((
                Mesh3d(unit_sphere.clone()),
                MeshMaterial3d(skin),
                Transform::from_xyz(0.0, 1.95, 0.0).with_scale(Vec3::splat(0.28)),
            ));
        });

    // Swirling Uzumaki concentric spirals & ink smoke clouds
    for i in 0..16 {
        let ang = i as f32 * 0.42;
        let rad = 0.6 + (i % 4) as f32 * 0.55;
        let bp = stage + Vec3::new(ang.cos() * rad, 0.4 + (i % 5) as f32 * 0.55, ang.sin() * rad);
        commands.spawn((
            MikeCutsceneEntity,
            MikeCutsceneSmoke {
                base_pos: bp,
                speed: 0.7 + (i % 3) as f32 * 0.35,
                phase: i as f32 * 0.8,
            },
            Mesh3d(unit_sphere.clone()),
            MeshMaterial3d(smoke_mat.clone()),
            Transform::from_translation(bp).with_scale(Vec3::splat(0.32 + (i % 3) as f32 * 0.16)),
        ));
    }
    for ring_i in 0..8 {
        let sc = 0.8 + ring_i as f32 * 0.65;
        commands.spawn((
            MikeCutsceneEntity,
            Mesh3d(spiral_torus.clone()),
            MeshMaterial3d(spiral_white.clone()),
            Transform::from_translation(stage + Vec3::new(0.0, 1.7, -2.8))
                .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
                .with_scale(Vec3::splat(sc)),
        ));
    }

    if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
        c_tf.translation = stage + Vec3::new(2.6, 2.2, 3.4);
        c_tf.look_at(stage + Vec3::new(0.0, 1.5, -0.4), Vec3::Y);
        if let Projection::Perspective(ref mut p) = *proj {
            p.fov = std::f32::consts::FRAC_PI_4;
        }
    }

    // Subtitle overlay
    let (mike_line, _) = MIKE_BACKSTORIES[dialog.variant];
    commands
        .spawn((
            MikeCutsceneOverlay,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::FlexEnd,
                align_items: AlignItems::Center,
                padding: UiRect::bottom(Val::Px(22.0)),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Percent(88.0),
                    max_width: Val::Px(780.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(14.0)),
                    row_gap: Val::Px(6.0),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.08, 0.02, 0.14, 0.86)),
                BorderColor(Color::srgb(0.35, 0.98, 0.55)),
                BorderRadius::all(Val::Px(12.0)),
            ))
            .with_children(|card| {
                card.spawn((
                    MikeCutsceneSpeakerText,
                    Text::new("MIKE (ALLEY ORACLE)"),
                    TextFont {
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.40, 1.0, 0.62)),
                ));
                card.spawn((
                    MikeCutsceneSubtitleText,
                    Text::new(format!("\"{}\"", mike_line)),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });
        });

    play_headphone_music(2);
    play_sfx("smoke_puff");
    speak_voice(mike_line, "mike");
}

pub fn update_mike_cutscene(
    time: Res<Time>,
    mut stats: ResMut<PlayerStats>,
    dialog: Res<MikeCutsceneDialog>,
    mut next_state: crate::transition::SceneTransition,
    mut smoke_q: Query<(&mut Transform, &MikeCutsceneSmoke), Without<Camera3d>>,
    mut cam_q: Query<(&mut Transform, &mut Projection), With<Camera3d>>,
    mut speaker_q: Query<&mut Text, With<MikeCutsceneSpeakerText>>,
    mut sub_q: Query<&mut Text, (With<MikeCutsceneSubtitleText>, Without<MikeCutsceneSpeakerText>)>,
    mut beat1_start: Local<f32>,
) {
    let dt = time.delta_secs().min(0.05);
    stats.cutscene_timer += dt;
    let t = stats.cutscene_timer;
    if stats.cutscene_beat == 0 && t <= dt * 2.0 {
        *beat1_start = 0.0;
    }
    // Hold-to-skip: jump to Stan's beat with its wait elapsed so the normal end path runs.
    let skipping = crate::skip::skipping();
    if skipping {
        stats.cutscene_beat = 1;
        *beat1_start = (*beat1_start).min(t - 3.5);
    }

    let (mike_line, stan_line) = MIKE_BACKSTORIES[dialog.variant];
    if stats.cutscene_beat == 0 && t >= 4.0 && (!is_voice_speaking() || t >= 16.0) {
        stats.cutscene_beat = 1;
        *beat1_start = t;
        play_sfx("smoke_puff");
        speak_voice(stan_line, "stan");
        if let Ok(mut sp) = speaker_q.get_single_mut() {
            sp.0 = "STAN (ENLIGHTENED)".to_string();
        }
        if let Ok(mut sb) = sub_q.get_single_mut() {
            sb.0 = format!("\"{}\"", stan_line);
        }
    }
    let b1_t = (t - *beat1_start).max(0.0);
    if stats.cutscene_beat == 1 && b1_t >= 3.5 && (skipping || !is_voice_speaking() || b1_t >= 16.0) {
        next_state.set(GameState::CityPlay);
        return;
    }

    // Animate swirling weed smoke clouds
    for (mut stf, sm) in &mut smoke_q {
        let rise = ((t * sm.speed + sm.phase) % 2.8) * 1.1;
        stf.translation.x = sm.base_pos.x + (t * 1.4 + sm.phase).sin() * 0.45;
        stf.translation.y = sm.base_pos.y + rise;
        stf.translation.z = sm.base_pos.z + (t * 1.1 + sm.phase).cos() * 0.45;
    }

    // Uzumaki 3/4 dramatic close-up on Mike (eyes, baseball cap, light short beard) -> reverse 3/4 on Stan
    let stage = Vec3::new(0.0, 0.0, -210.0);
    if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
        if stats.cutscene_beat == 0 {
            let push = (t / 7.0).min(1.0);
            let cam_pos = stage + Vec3::new(0.75 - push * 0.22, 1.72 + push * 0.04, 0.18 - push * 0.38);
            c_tf.translation = cam_pos;
            c_tf.look_at(stage + Vec3::new(0.0, 1.82, -1.55), Vec3::Y);
            c_tf.rotate_local_z(0.035 + (t * 0.65).sin() * 0.025);
            if let Projection::Perspective(ref mut p) = *proj {
                p.fov = 0.64 - push * 0.06;
            }
        } else {
            let push = (b1_t / 6.0).min(1.0);
            let cam_pos = stage + Vec3::new(-1.10 + (b1_t * 0.35).sin() * 0.18, 1.80, 2.55 - push * 0.35);
            c_tf.translation = cam_pos;
            c_tf.look_at(stage + Vec3::new(0.0, 1.62, 0.9), Vec3::Y);
            c_tf.rotate_local_z(-0.035 + (b1_t * 0.65).sin() * 0.025);
            if let Projection::Perspective(ref mut p) = *proj {
                p.fov = 0.68 - push * 0.05;
            }
        }
    }
    let _ = mike_line;
}

pub fn exit_mike_cutscene(
    mut commands: Commands,
    mut stats: ResMut<PlayerStats>,
    ent_q: Query<Entity, With<MikeCutsceneEntity>>,
    ov_q: Query<Entity, With<MikeCutsceneOverlay>>,
    mut cam_q: Query<&mut Projection, With<Camera3d>>,
) {
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
    set_uzumaki_mode(0);
    if stats.in_city {
        // Last step of the Level 2 gate (see `PlayerStats::gate_open`).
        stats.mike_smoked = true;
    }
    stats.hearts = (stats.hearts + 3).min(stats.max_hearts);
    stats.dash_energy = 100.0;
    stats.high_timer = 30.0;
    if stats.in_city {
        play_viking_chant_music();
    } else {
        play_bgm();
    }
}

