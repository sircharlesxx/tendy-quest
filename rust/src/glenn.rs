use bevy::prelude::*;
use crate::audio::{
    is_voice_speaking, play_boss_music, play_cutscene_music, play_sfx, play_viking_chant_music,
    speak_voice,
};
use crate::city::{CityWorldMarker, PUB217_ANCHOR, PUB217_SCALE};
use crate::player::Player;
use crate::state::{GameState, LevelEntity, PlayerStats};

#[derive(Component)]
pub struct CityGlenn {
    pub was_inside: bool,
    pub recruited: bool,
}

#[derive(Component)]
pub struct GlennCutsceneEntity;

#[derive(Component)]
pub struct GlennCutsceneOverlay;

#[derive(Component)]
pub struct GlennSpeakerText;

#[derive(Component)]
pub struct GlennBadgeText;

#[derive(Component)]
pub struct GlennSubtitleText;

#[derive(Component)]
pub struct GlennCutsceneActor {
    pub is_stan: bool,
}

#[derive(Component)]
pub struct GlennBeerTower {
    pub base_pos: Vec3,
    pub vel: Vec3,
    pub spin: Vec3,
}

#[derive(Component)]
pub struct GlennTearDrop {
    pub base_pos: Vec3,
    pub phase: f32,
}

#[derive(Component)]
pub struct GlennKiAura {
    pub is_stan: bool,
}

#[derive(Component)]
pub struct GlennTeleportRing {
    pub phase: f32,
}

#[derive(Component)]
pub struct GlennDestructibleBuilding {
    pub index: usize,
    pub base_y: f32,
    pub base_x: f32,
    pub base_z: f32,
}

#[derive(Component)]
pub struct GlennRubble {
    pub base_pos: Vec3,
    pub vel: Vec3,
    pub phase: f32,
}

#[derive(Resource, Default)]
pub struct GlennCutsceneState {
    pub beat: u8,
    pub beat_t: f32,
    pub tp_sfx_timer: f32,
    pub fight_taunt_done: bool,
}

pub struct GlennMaterials {
    skin: Handle<StandardMaterial>,
    tank: Handle<StandardMaterial>,
    hair: Handle<StandardMaterial>,
    shorts: Handle<StandardMaterial>,
    shoe: Handle<StandardMaterial>,
    eye_white: Handle<StandardMaterial>,
    eye_pupil: Handle<StandardMaterial>,
    tear: Handle<StandardMaterial>,
}

pub fn make_glenn_materials(materials: &mut Assets<StandardMaterial>) -> GlennMaterials {
    GlennMaterials {
        skin: materials.add(StandardMaterial {
            base_color: Color::srgb(0.88, 0.70, 0.58),
            perceptual_roughness: 0.55,
            ..default()
        }),
        tank: materials.add(StandardMaterial {
            base_color: Color::srgb(0.10, 0.10, 0.14),
            perceptual_roughness: 0.80,
            ..default()
        }),
        hair: materials.add(StandardMaterial {
            base_color: Color::srgb(0.04, 0.04, 0.05),
            perceptual_roughness: 0.72,
            ..default()
        }),
        shorts: materials.add(StandardMaterial {
            base_color: Color::srgb(0.16, 0.18, 0.24),
            perceptual_roughness: 0.85,
            ..default()
        }),
        shoe: materials.add(StandardMaterial {
            base_color: Color::srgb(0.92, 0.18, 0.22),
            perceptual_roughness: 0.55,
            ..default()
        }),
        eye_white: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            ..default()
        }),
        eye_pupil: materials.add(StandardMaterial {
            base_color: Color::srgb(0.05, 0.05, 0.07),
            ..default()
        }),
        tear: materials.add(StandardMaterial {
            base_color: Color::srgba(0.55, 0.88, 1.0, 0.90),
            emissive: LinearRgba::new(0.22, 0.55, 0.85, 1.0),
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
    }
}

pub fn spawn_glenn_rig(
    parent: &mut ChildBuilder,
    meshes: &mut Assets<Mesh>,
    mats: &GlennMaterials,
) {
    use crate::shapes::{self as sh, ring};

    // Thick muscular quads, calves & gym sneakers
    let quad_mesh = sh::add(meshes, sh::limb_profile(0.70, &[0.17, 0.178, 0.155, 0.13], 1.0));
    let calf_mesh = sh::add(meshes, sh::limb_profile(0.52, &[0.105, 0.132, 0.11, 0.075], 1.0));
    let shoe_mesh = sh::add(meshes, sh::shoe(0.23, 0.13, 0.34));
    let sole_mesh = sh::add(meshes, sh::shoe_sole(0.23, 0.13, 0.34));
    for sx in [-0.20_f32, 0.20] {
        parent.spawn((
            Mesh3d(quad_mesh.clone()),
            MeshMaterial3d(mats.shorts.clone()),
            Transform::from_xyz(sx, 0.55, 0.0),
        ));
        parent.spawn((
            Mesh3d(calf_mesh.clone()),
            MeshMaterial3d(mats.skin.clone()),
            Transform::from_xyz(sx, 0.27, 0.0),
        ));
        parent.spawn((
            Mesh3d(shoe_mesh.clone()),
            MeshMaterial3d(mats.shoe.clone()),
            Transform::from_xyz(sx, 0.065, 0.05),
        ));
        parent.spawn((
            Mesh3d(sole_mesh.clone()),
            MeshMaterial3d(mats.eye_white.clone()),
            Transform::from_xyz(sx, 0.065, 0.05),
        ));
    }

    // Massive V-tapered muscular torso (one continuous black tank), sculpted pecs & traps
    let tank = sh::loft(
        &[
            ring(0.63, 0.0, 0.0),
            ring(0.65, 0.29, 0.18),
            ring(0.72, 0.33, 0.20),
            ring(0.90, 0.32, 0.20).dz(0.01),
            ring(1.08, 0.40, 0.24).dz(0.02),
            ring(1.25, 0.48, 0.27).dz(0.03),
            ring(1.40, 0.50, 0.25),
            ring(1.48, 0.38, 0.20),
            ring(1.53, 0.15, 0.12),
            ring(1.54, 0.0, 0.0),
        ],
        3,
        32,
        0.85,
    );
    parent.spawn((
        Mesh3d(sh::add(meshes, tank)),
        MeshMaterial3d(mats.tank.clone()),
        Transform::default(),
    ));
    let pec_mesh = sh::add(meshes, sh::superellipsoid(Vec3::splat(0.22), 0.8, 0.9, 24, 16));
    for sx in [-0.21_f32, 0.21] {
        parent.spawn((
            Mesh3d(pec_mesh.clone()),
            MeshMaterial3d(mats.tank.clone()),
            Transform::from_xyz(sx, 1.27, 0.14).with_scale(Vec3::new(1.1, 0.8, 0.62)),
        ));
    }
    parent.spawn((
        Mesh3d(sh::add(meshes, sh::superellipsoid(Vec3::new(0.34, 0.11, 0.17), 1.0, 1.0, 24, 14))),
        MeshMaterial3d(mats.skin.clone()),
        Transform::from_xyz(0.0, 1.49, -0.03),
    ));

    // Giant boulder shoulders + thick bulging biceps & forearms
    let bicep_mesh = sh::add(meshes, sh::limb_profile(0.71, &[0.15, 0.188, 0.172, 0.13], 1.0));
    let forearm_mesh = sh::add(meshes, sh::limb_profile(0.63, &[0.145, 0.152, 0.125, 0.098], 1.0));
    for sx in [-0.62_f32, 0.62] {
        // Boulder delt
        parent.spawn((
            Mesh3d(meshes.add(Sphere::new(0.25))),
            MeshMaterial3d(mats.skin.clone()),
            Transform::from_xyz(sx, 1.40, 0.0).with_scale(Vec3::new(1.0, 0.94, 0.96)),
        ));
        // Huge bicep
        parent.spawn((
            Mesh3d(bicep_mesh.clone()),
            MeshMaterial3d(mats.skin.clone()),
            Transform::from_xyz(sx * 1.06, 1.12, 0.03)
                .with_rotation(Quat::from_rotation_z(if sx < 0.0 { 0.16 } else { -0.16 })),
        ));
        // Thick forearm + fist
        parent.spawn((
            Mesh3d(forearm_mesh.clone()),
            MeshMaterial3d(mats.skin.clone()),
            Transform::from_xyz(sx * 1.10, 0.78, 0.08),
        ));
        parent.spawn((
            Mesh3d(sh::add(meshes, sh::hand(0.13, sx.signum(), true))),
            MeshMaterial3d(mats.skin.clone()),
            Transform::from_xyz(sx * 1.10, 0.52, 0.10),
        ));
    }

    // Thick muscular neck + head
    parent.spawn((
        Mesh3d(sh::add(meshes, sh::limb(0.28, 0.15, 0.175))),
        MeshMaterial3d(mats.skin.clone()),
        Transform::from_xyz(0.0, 1.58, -0.01),
    ));
    let (hr, jaw) = (0.27_f32, 0.08_f32);
    parent.spawn((
        Mesh3d(sh::add(meshes, sh::head(hr, jaw))),
        MeshMaterial3d(mats.skin.clone()),
        Transform::from_xyz(0.0, 1.82, 0.0),
    ));
    parent.spawn((
        Mesh3d(sh::add(meshes, sh::superellipsoid(Vec3::new(0.042, 0.058, 0.048), 1.0, 1.0, 14, 10))),
        MeshMaterial3d(mats.skin.clone()),
        Transform::from_xyz(0.0, 1.80, 0.268),
    ));

    // Long curly black hair: a scalp layer under layered lumpy curls cascading past the shoulders
    parent.spawn((
        Mesh3d(sh::add(meshes, sh::hair_cap(hr, jaw, 0.64, -0.70, 0.05, 0.08))),
        MeshMaterial3d(mats.hair.clone()),
        Transform::from_xyz(0.0, 1.82, 0.0),
    ));
    let curl_mesh = sh::add(
        meshes,
        sh::blob(28, 18, |d| {
            let th = d.z.atan2(d.x);
            d * 0.13 * (1.0 + 0.09 * (th * 5.0).sin() * (d.y * 7.0).cos() + 0.05 * (d.y * 11.0 + th * 3.0).sin())
        }),
    );
    for (i, (cx, cy, cz, sc)) in [
        // Top crown curls
        (0.0_f32, 2.08_f32, -0.02_f32, 1.35_f32),
        (-0.18, 2.04, 0.06, 1.15),
        (0.18, 2.04, 0.06, 1.15),
        (-0.24, 1.94, -0.08, 1.20),
        (0.24, 1.94, -0.08, 1.20),
        // Long side & shoulder curls
        (-0.30, 1.78, -0.04, 1.10),
        (0.30, 1.78, -0.04, 1.10),
        (-0.32, 1.58, -0.08, 1.15),
        (0.32, 1.58, -0.08, 1.15),
        (-0.34, 1.38, -0.10, 1.05),
        (0.34, 1.38, -0.10, 1.05),
        // Cascading back mane curls
        (0.0, 1.88, -0.26, 1.45),
        (-0.18, 1.72, -0.28, 1.30),
        (0.18, 1.72, -0.28, 1.30),
        (0.0, 1.54, -0.29, 1.40),
        (-0.18, 1.36, -0.28, 1.25),
        (0.18, 1.36, -0.28, 1.25),
        (0.0, 1.22, -0.26, 1.25),
    ]
    .into_iter()
    .enumerate()
    {
        parent.spawn((
            Mesh3d(curl_mesh.clone()),
            MeshMaterial3d(mats.hair.clone()),
            Transform::from_xyz(cx, cy, cz)
                .with_rotation(Quat::from_rotation_y(i as f32 * 1.7) * Quat::from_rotation_x(i as f32 * 0.6))
                .with_scale(Vec3::splat(sc)),
        ));
    }

    // Expressive eyes & brows
    let brow_l = sh::arc_bar(0.11, &[0.015, 0.016, 0.012, 0.008], 0.008, 0.02, 0.7);
    let brow_r = brow_l.clone().mirrored_x();
    let (brow_l, brow_r) = (sh::add(meshes, brow_l), sh::add(meshes, brow_r));
    let eye_mesh = sh::add(meshes, sh::superellipsoid(Vec3::new(0.042, 0.042, 0.036), 1.0, 1.0, 20, 14));
    let pupil_mesh = sh::add(meshes, sh::superellipsoid(Vec3::new(0.023, 0.023, 0.015), 1.0, 1.0, 14, 10));
    for sx in [-0.095_f32, 0.095] {
        parent.spawn((
            Mesh3d(if sx < 0.0 { brow_l.clone() } else { brow_r.clone() }),
            MeshMaterial3d(mats.hair.clone()),
            Transform::from_xyz(sx, 1.92, 0.24)
                .with_rotation(Quat::from_rotation_z(if sx < 0.0 { -0.15 } else { 0.15 })),
        ));
        parent.spawn((
            Mesh3d(eye_mesh.clone()),
            MeshMaterial3d(mats.eye_white.clone()),
            Transform::from_xyz(sx, 1.86, 0.238),
        ));
        parent.spawn((
            Mesh3d(pupil_mesh.clone()),
            MeshMaterial3d(mats.eye_pupil.clone()),
            Transform::from_xyz(sx, 1.86, 0.268),
        ));
    }
}

pub fn spawn_city_glenn(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let mats = make_glenn_materials(materials);
    let pos = PUB217_ANCHOR + Vec3::new(-11.2, 0.0, 2.2);
    commands
        .spawn((
            LevelEntity,
            CityWorldMarker,
            CityGlenn {
                was_inside: false,
                recruited: false,
            },
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(1.35)),
            Visibility::default(),
        ))
        .with_children(|p| {
            spawn_glenn_rig(p, meshes, &mats);
        });
}

pub fn update_city_glenn(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut stats: ResMut<PlayerStats>,
    player_q: Query<(&Transform, &Player), Without<CityGlenn>>,
    orbit: Res<crate::state::CameraOrbit>,
    mut glenn_q: Query<(&mut Transform, &mut CityGlenn), Without<Player>>,
    mut next_state: crate::transition::SceneTransition,
    mut g_state: ResMut<GlennCutsceneState>,
) {
    let Ok((p_tf, player)) = player_q.get_single() else {
        return;
    };
    let dt = time.delta_secs().min(0.05);
    let pub_s = PUB217_SCALE;
    for (mut g_tf, mut glenn) in &mut glenn_q {
        // `stats.glenn_recruited` is the gate's source of truth (checkpoint restore / test links).
        if stats.glenn_recruited && !glenn.recruited {
            glenn.recruited = true;
        }
        // Post-game free roam: recruited crew skip following and idle at their pub spots.
        if glenn.recruited && !stats.free_roam {
            let s = (stats.player_scale * 0.76).clamp(1.75, 9.5);
            g_tf.scale = Vec3::splat(s);
            // Outer right flank, beyond TJ, out of the camera line at every scale.
            let target = crate::player::crew_follow_target(
                crate::player::CrewSlot::RightOuter,
                p_tf.translation,
                stats.player_scale,
                s,
                orbit.yaw,
            );
            let delta = Vec3::new(target.x - g_tf.translation.x, 0.0, target.z - g_tf.translation.z);
            let dist = delta.length();
            if dist > 32.0 {
                g_tf.translation = Vec3::new(target.x, 0.0, target.z);
            } else if dist > 0.35 {
                let speed = (10.5 + s * 2.5).max(14.5) * if dist > 7.0 { 1.6 } else { 1.0 };
                let step = (speed * dt).min(dist);
                let dir = delta / dist;
                g_tf.translation.x += dir.x * step;
                g_tf.translation.z += dir.z * step;
                g_tf.translation.y = (time.elapsed_secs() * 10.5).sin().abs() * 0.08 * s;
                g_tf.rotation = Quat::from_rotation_y(dir.x.atan2(dir.z));
            } else {
                g_tf.translation.y = 0.0;
                g_tf.rotation = Quat::from_rotation_y(player.facing_yaw + std::f32::consts::PI);
            }
            continue;
        }

        g_tf.translation = PUB217_ANCHOR + Vec3::new(-11.2, 0.0, 2.2) * pub_s;
        g_tf.scale = Vec3::splat(pub_s);
        let to_p = p_tf.translation - g_tf.translation;
        let flat = Vec3::new(to_p.x, 0.0, to_p.z);
        if flat.length_squared() > 0.01 {
            g_tf.rotation = Quat::from_rotation_y(flat.x.atan2(flat.z));
        }

        let dist = flat.length();
        let trigger_r = 2.0 * pub_s;
        let inside = dist < trigger_r;
        let interact = keys.just_pressed(KeyCode::KeyF) || keys.just_pressed(KeyCode::KeyE);

        // Ground level only: standing on the pub roof above them doesn't count.
        if p_tf.translation.y < 4.0
            && stats.driving_car.is_none()
            && stats.riding_scooter.is_none()
            && ((inside && !glenn.was_inside) || (dist < 4.3 * pub_s && interact))
        {
            glenn.was_inside = true;
            if stats.free_roam {
                // No cutscene replay in free roam: just a friendly line.
                stats.hint_text = "GLENN: \"Still lifting, still winning. Go do a lap on the scooter, bro.\"".to_string();
                stats.hint_bump = stats.hint_bump.wrapping_add(1);
                continue;
            }
            stats.saved_city_pos = p_tf.translation;
            stats.glenn_return_city = true;
            *g_state = GlennCutsceneState::default();
            next_state.set(GameState::GlennCutscene);
            return;
        }
        glenn.was_inside = inside;
    }
}

const GLENN_BEATS: [(&str, &str, &str, &str); 7] = [
    (
        "STAN",
        "BEAT 1/7  |  QUESTIONING THE GAINZ",
        "Glenn... look at those massive boulder shoulders and that long curly black hair. Seriously, why do you even work out like that?!",
        "stan",
    ),
    (
        "GLENN (REASONABLE)",
        "BEAT 2/7  |  LONGEVITY & GROCERIES",
        "I train for cardiovascular health, mental clarity, longevity, and so I can carry all the groceries in one trip, Stan.",
        "glenn",
    ),
    (
        "STAN (UNREASONABLE)",
        "BEAT 3/7  |  BEER TOWER BETRAYAL!!",
        "Cardiovascular longevity?! THAT DOESN'T EVEN MAKE SENSE!! WATCH WHAT I THINK OF YOUR MACROS!!",
        "stan",
    ),
    (
        "GLENN (HEARTBROKEN & FURIOUS)",
        "BEAT 4/7  |  WHY?!?!",
        "MY BEER TOWER... WHY?!?! WHY WOULD YOU DO THAT, STAN?! NOW YOU'VE AWAKENED MY MAXIMUM GAINZ!!",
        "glenn",
    ),
    (
        "STAN & GLENN (DBZ TELEPORT WAR!)",
        "BEAT 5/7  |  SKYSCRAPERS VAPORIZED!!",
        "TOO SLOW!! *Z-VANISH!* ANOTHER BUILDING JUST EXPLODED!! WE'RE TEARING DOWN ALL OF CEDAR RAPIDS!!",
        "glenn",
    ),
    (
        "GLENN (MID-AIR STANDOFF)",
        "BEAT 6/7  |  SAVE PUB 217",
        "WAIT - STOP!! Look at the smoking skyline! If we keep fighting, we're going to destroy our precious Pub 217 and our whole city!!",
        "glenn",
    ),
    (
        "STAN & GLENN (SUPER SAIYAN ALLIANCE)",
        "BEAT 7/7  |  DESTROY LEXIE INSTEAD!!",
        "You're right, Glenn... your gainz DO make sense! Let's save Pub 217 and obliterate Lexie together!!",
        "stan",
    ),
];

const TP_SPOTS: [(f32, f32, f32); 4] = [
    (0.0, 2.0, 0.0),
    (-9.0, 6.5, -8.0),
    (11.0, 9.0, -14.0),
    (-6.0, 4.5, -18.0),
];

pub fn enter_glenn_cutscene(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut g_state: ResMut<GlennCutsceneState>,
    mut cam_q: Query<(&mut Transform, &mut Projection), With<Camera3d>>,
) {
    *g_state = GlennCutsceneState::default();
    let stage = Vec3::new(0.0, 0.0, -640.0);
    let mats = make_glenn_materials(&mut materials);

    // Nighttime city + Pub 217 floor & skyline arena
    let floor_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.09, 0.07, 0.08),
        perceptual_roughness: 0.85,
        ..default()
    });
    commands.spawn((
        GlennCutsceneEntity,
        Mesh3d(meshes.add(Cuboid::new(64.0, 0.2, 64.0))),
        MeshMaterial3d(floor_mat),
        Transform::from_translation(stage + Vec3::new(0.0, -0.1, -8.0)),
    ));

    // Destructible city skyscrapers framing the DBZ teleport battle!
    let bldg_mats = [
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.22, 0.26, 0.38),
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.36, 0.20, 0.18),
            ..default()
        }),
    ];
    for (idx, (bx, bz, h)) in [
        (-13.0_f32, -10.0_f32, 16.0_f32),
        (13.0, -11.0, 18.0),
        (-10.0, -21.0, 22.0),
        (10.0, -20.0, 20.0),
        (0.0, -25.0, 26.0),
    ]
    .iter()
    .enumerate()
    {
        commands.spawn((
            GlennCutsceneEntity,
            GlennDestructibleBuilding {
                index: idx,
                base_y: *h * 0.5,
                base_x: *bx,
                base_z: *bz,
            },
            Mesh3d(meshes.add(Cuboid::new(5.5, *h, 5.5))),
            MeshMaterial3d(bldg_mats[idx % 2].clone()),
            Transform::from_translation(stage + Vec3::new(*bx, *h * 0.5, *bz)),
        ));
    }

    // Beer Tower Pyramid pieces between them (Stan knocks these flying in Beat 2!)
    let beer_gold = materials.add(StandardMaterial {
        base_color: Color::srgba(0.95, 0.68, 0.12, 0.92),
        emissive: LinearRgba::new(0.95, 0.55, 0.08, 1.0),
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let mut tower_i = 0;
    for tier in 0..4 {
        let count = 4 - tier;
        let y = 0.38 + tier as f32 * 0.68;
        for c in 0..count {
            let x_off = (c as f32 - (count - 1) as f32 * 0.5) * 0.36;
            let pos = stage + Vec3::new(x_off, y, -0.2);
            let side = if tower_i % 2 == 0 { 1.0 } else { -1.0 };
            commands.spawn((
                GlennCutsceneEntity,
                GlennBeerTower {
                    base_pos: pos,
                    vel: Vec3::new(side * (1.8 + (tower_i % 3) as f32), 4.5 + tier as f32, -2.5),
                    spin: Vec3::new(4.2, side * 5.0, 3.6),
                },
                Mesh3d(meshes.add(Cylinder::new(0.14, 0.64))),
                MeshMaterial3d(beer_gold.clone()),
                Transform::from_translation(pos),
            ));
            tower_i += 1;
        }
    }

    commands.spawn((
        GlennCutsceneEntity,
        PointLight {
            color: Color::srgb(1.0, 0.92, 0.78),
            intensity: 460_000.0,
            range: 45.0,
            ..default()
        },
        Transform::from_translation(stage + Vec3::new(0.0, 8.0, 2.0)),
    ));

    // Spawn Muscular Glenn at -Z facing +Z
    commands
        .spawn((
            GlennCutsceneEntity,
            GlennCutsceneActor { is_stan: false },
            Transform::from_translation(stage + Vec3::new(0.0, 0.0, -1.75)),
            Visibility::default(),
        ))
        .with_children(|p| {
            spawn_glenn_rig(p, &mut meshes, &mats);
        });

    // Spawn REAL Stan (`models/plaid_hero.glb#Scene0`) at +Z facing -Z
    let hero_scene: Handle<Scene> = asset_server.load("models/plaid_hero.glb#Scene0");
    commands.spawn((
        GlennCutsceneEntity,
        GlennCutsceneActor { is_stan: true },
        SceneRoot(hero_scene),
        Transform::from_translation(stage + Vec3::new(0.0, 0.0, 1.75)).with_scale(Vec3::splat(1.22)),
    ));

    // Glistening 3D teardrops for Glenn's "WHY?!" breakdown
    let tear_mesh = meshes.add(Sphere::new(0.025));
    for (i, sx) in [-0.095_f32, 0.095].iter().enumerate() {
        commands.spawn((
            GlennCutsceneEntity,
            GlennTearDrop {
                base_pos: stage + Vec3::new(*sx, 1.82, -1.49),
                phase: i as f32 * 0.55,
            },
            Mesh3d(tear_mesh.clone()),
            MeshMaterial3d(mats.tear.clone()),
            Transform::from_translation(stage + Vec3::new(*sx, 1.82, -1.49)),
            Visibility::Hidden,
        ));
    }

    // DBZ Super Ki Auras, Teleport Sonic Rings & Flying Building Rubble
    // Keep aura alpha (0.11) and HDR emissive (0.85) restrained: in Bevy's AlphaMode::Blend,
    // high emissive values (>1.0) multiply with alpha and saturate into an opaque cylinder
    // that hides the characters inside.
    let glenn_aura = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.25, 0.20, 0.11),
        emissive: LinearRgba::new(0.85, 0.20, 0.08, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let stan_aura = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.85, 0.20, 0.11),
        emissive: LinearRgba::new(0.85, 0.62, 0.08, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    for (is_stan, mat) in [(false, glenn_aura), (true, stan_aura)] {
        commands.spawn((
            GlennCutsceneEntity,
            GlennKiAura { is_stan },
            Mesh3d(meshes.add(Cylinder::new(0.85, 2.85))),
            MeshMaterial3d(mat),
            Transform::from_translation(stage),
            Visibility::Hidden,
        ));
    }

    let ring_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.55, 0.95, 1.0, 0.82),
        emissive: LinearRgba::new(2.0, 4.5, 5.5, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    for i in 0..4 {
        commands.spawn((
            GlennCutsceneEntity,
            GlennTeleportRing {
                phase: i as f32 * 0.25,
            },
            Mesh3d(meshes.add(Torus::new(0.05, 0.72))),
            MeshMaterial3d(ring_mat.clone()),
            Transform::from_translation(stage),
            Visibility::Hidden,
        ));
    }

    let rubble_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.32, 0.30, 0.34),
        perceptual_roughness: 0.9,
        ..default()
    });
    let rubble_mesh = meshes.add(Cuboid::new(0.45, 0.38, 0.42));
    for i in 0..18 {
        let ang = (i as f32 / 18.0) * std::f32::consts::TAU;
        let r = 2.5 + (i % 4) as f32 * 1.4;
        let pos = stage + Vec3::new(ang.cos() * r, 0.4, -10.0 + ang.sin() * r);
        commands.spawn((
            GlennCutsceneEntity,
            GlennRubble {
                base_pos: pos,
                vel: Vec3::new(ang.cos() * 3.5, 6.5 + (i % 3) as f32 * 2.0, ang.sin() * 3.5),
                phase: i as f32 * 0.37,
            },
            Mesh3d(rubble_mesh.clone()),
            MeshMaterial3d(rubble_mat.clone()),
            Transform::from_translation(pos),
            Visibility::Hidden,
        ));
    }

    if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
        c_tf.translation = stage + Vec3::new(-0.55, 1.72, 0.72);
        c_tf.look_at(stage + Vec3::new(0.0, 1.65, 1.75), Vec3::Y);
        if let Projection::Perspective(ref mut p) = *proj {
            p.fov = 0.60;
        }
    }

    let (spk, badge, line, voice) = GLENN_BEATS[0];
    commands
        .spawn((
            GlennCutsceneOverlay,
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
                    max_width: Val::Px(820.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(14.0)),
                    row_gap: Val::Px(6.0),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.08, 0.03, 0.06, 0.90)),
                BorderColor(Color::srgb(1.0, 0.42, 0.20)),
                BorderRadius::all(Val::Px(12.0)),
            ))
            .with_children(|card| {
                card.spawn((
                    Node {
                        justify_content: JustifyContent::SpaceBetween,
                        width: Val::Percent(100.0),
                        ..default()
                    },
                ))
                .with_children(|hdr| {
                    hdr.spawn((
                        GlennSpeakerText,
                        Text::new(spk),
                        TextFont {
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.55, 0.25)),
                    ));
                    hdr.spawn((
                        GlennBadgeText,
                        Text::new(badge),
                        TextFont {
                            font_size: 12.5,
                            ..default()
                        },
                        TextColor(Color::srgb(0.65, 0.88, 1.0)),
                    ));
                });
                card.spawn((
                    GlennSubtitleText,
                    Text::new(format!("\"{}\"", line)),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });
        });

    play_cutscene_music();
    speak_voice(line, voice);
}

#[derive(bevy::ecs::system::SystemParam)]
pub struct GlennVfxQueries<'w, 's> {
    tower_q: Query<
        'w,
        's,
        (&'static mut Transform, &'static GlennBeerTower),
        (Without<GlennCutsceneActor>, Without<Camera3d>),
    >,
    tear_q: Query<
        'w,
        's,
        (&'static mut Transform, &'static mut Visibility, &'static GlennTearDrop),
        (Without<GlennCutsceneActor>, Without<GlennBeerTower>, Without<Camera3d>),
    >,
    aura_q: Query<
        'w,
        's,
        (&'static mut Transform, &'static mut Visibility, &'static GlennKiAura),
        (
            Without<GlennCutsceneActor>,
            Without<GlennBeerTower>,
            Without<GlennTearDrop>,
            Without<Camera3d>,
        ),
    >,
    ring_q: Query<
        'w,
        's,
        (&'static mut Transform, &'static mut Visibility, &'static GlennTeleportRing),
        (
            Without<GlennCutsceneActor>,
            Without<GlennBeerTower>,
            Without<GlennTearDrop>,
            Without<GlennKiAura>,
            Without<Camera3d>,
        ),
    >,
    bldg_q: Query<
        'w,
        's,
        (&'static mut Transform, &'static GlennDestructibleBuilding),
        (
            Without<GlennCutsceneActor>,
            Without<GlennBeerTower>,
            Without<GlennTearDrop>,
            Without<GlennKiAura>,
            Without<GlennTeleportRing>,
            Without<Camera3d>,
        ),
    >,
    rubble_q: Query<
        'w,
        's,
        (&'static mut Transform, &'static mut Visibility, &'static GlennRubble),
        (
            Without<GlennCutsceneActor>,
            Without<GlennBeerTower>,
            Without<GlennTearDrop>,
            Without<GlennKiAura>,
            Without<GlennTeleportRing>,
            Without<GlennDestructibleBuilding>,
            Without<Camera3d>,
        ),
    >,
}

#[allow(clippy::type_complexity)]
pub fn update_glenn_cutscene(
    time: Res<Time>,
    mut g_state: ResMut<GlennCutsceneState>,
    mut next_state: crate::transition::SceneTransition,
    mut actor_q: Query<(&mut Transform, &GlennCutsceneActor), Without<Camera3d>>,
    mut vfx: GlennVfxQueries,
    mut cam_q: Query<(&mut Transform, &mut Projection), With<Camera3d>>,
    mut spk_q: Query<&mut Text, With<GlennSpeakerText>>,
    mut badge_q: Query<&mut Text, (With<GlennBadgeText>, Without<GlennSpeakerText>)>,
    mut sub_q: Query<&mut Text, (With<GlennSubtitleText>, Without<GlennSpeakerText>, Without<GlennBadgeText>)>,
) {
    let dt = time.delta_secs().min(0.05);
    g_state.beat_t += dt;
    // Hold-to-skip: jump to the last beat with its wait elapsed so the normal end path runs.
    let skipping = crate::skip::skipping();
    if skipping {
        g_state.beat = (GLENN_BEATS.len() as u8).saturating_sub(1);
        g_state.beat_t = g_state.beat_t.max(4.0);
    }

    if g_state.beat == 4 && !g_state.fight_taunt_done && g_state.beat_t >= 6.5 {
        g_state.fight_taunt_done = true;
        play_sfx("thunder");
        play_sfx("car_crash");
        let mid_line = "MAXIMUM GAINZ OVERDRIVE!! *KRAKOOM* THE WHOLE SKYLINE IS COLLAPSING!!";
        speak_voice(mid_line, "stan");
        if let Ok(mut sub) = sub_q.get_single_mut() {
            sub.0 = format!("\"{}\"", mid_line);
        }
    }

    let min_t = if g_state.beat == 4 { 13.5 } else { 4.0 };
    if g_state.beat_t >= min_t && (skipping || !is_voice_speaking() || g_state.beat_t >= 22.0) {
        if g_state.beat + 1 >= GLENN_BEATS.len() as u8 {
            next_state.set(GameState::CityPlay);
            return;
        }
        g_state.beat += 1;
        g_state.beat_t = 0.0;
        let (spk, badge, line, voice) = GLENN_BEATS[g_state.beat as usize];
        if g_state.beat == 2 {
            play_sfx("car_crash");
        } else if g_state.beat == 3 {
            play_sfx("thunder");
        } else if g_state.beat == 4 {
            play_boss_music();
            play_sfx("sword_clash");
        } else if g_state.beat == 5 {
            play_cutscene_music();
        }
        speak_voice(line, voice);
        if let Ok(mut s) = spk_q.get_single_mut() {
            s.0 = spk.to_string();
        }
        if let Ok(mut b) = badge_q.get_single_mut() {
            b.0 = badge.to_string();
        }
        if let Ok(mut sub) = sub_q.get_single_mut() {
            sub.0 = format!("\"{}\"", line);
        }
    }

    let beat = g_state.beat;
    let bt = g_state.beat_t;
    let stage = Vec3::new(0.0, 0.0, -640.0);

    if beat == 4 {
        g_state.tp_sfx_timer -= dt;
        if g_state.tp_sfx_timer <= 0.0 {
            g_state.tp_sfx_timer = 0.65;
            play_sfx("dash");
            play_sfx("sword_clash");
        }
    }

    // Beer Tower topples & scatters starting in Beat 2!
    for (mut ttf, bt_info) in &mut vfx.tower_q {
        if beat >= 2 {
            let t_fall = if beat == 2 { (bt - 0.6).max(0.0) } else { 3.0 };
            let y_drop = (bt_info.vel.y * t_fall - 6.8 * t_fall * t_fall).max(-0.35);
            ttf.translation = bt_info.base_pos + Vec3::new(bt_info.vel.x * t_fall, y_drop, bt_info.vel.z * t_fall);
            ttf.rotation = Quat::from_euler(
                EulerRot::XYZ,
                bt_info.spin.x * t_fall,
                bt_info.spin.y * t_fall,
                bt_info.spin.z * t_fall,
            );
        } else {
            ttf.translation = bt_info.base_pos;
            ttf.rotation = Quat::IDENTITY;
        }
    }

    // Glenn's teardrops stream down in Beat 3 ("WHY?!")
    for (mut ttf, mut vis, tear) in &mut vfx.tear_q {
        if beat == 3 {
            *vis = Visibility::Visible;
            let fall = ((bt * 0.50 + tear.phase) % 1.0) * 0.16;
            ttf.translation = tear.base_pos - Vec3::Y * fall;
        } else {
            *vis = Visibility::Hidden;
        }
    }

    // High-speed DBZ teleport index in Beat 4 (cycles rapidly through 4 skyline coordinates!)
    let tp_idx = ((bt * 1.8) as usize) % TP_SPOTS.len();
    let (tx, ty, tz) = if beat == 4 {
        TP_SPOTS[tp_idx]
    } else if beat == 5 {
        (0.0, 4.5, -6.0)
    } else {
        (0.0, 0.0, 0.0)
    };
    let fight_center = stage + Vec3::new(tx, ty, tz);

    for (mut tf, act) in &mut actor_q {
        if beat == 4 {
            let z_off = if act.is_stan { 2.05 } else { -2.05 };
            tf.translation = fight_center + Vec3::new(0.0, 0.0, z_off);
            tf.rotation = Quat::IDENTITY;
        } else if beat == 5 {
            let z_off = if act.is_stan { 1.5 } else { -1.5 };
            tf.translation = fight_center + Vec3::new(0.0, (bt * 2.5).sin() * 0.15, z_off);
            tf.rotation = Quat::IDENTITY;
        } else if beat == 6 {
            tf.translation = stage + Vec3::new(if act.is_stan { 0.85 } else { -0.85 }, 0.0, 0.6);
            tf.rotation = Quat::IDENTITY;
        } else if beat == 2 && act.is_stan {
            // Stan lunges forward to smash the beer tower!
            let lunge = (bt * 1.4).min(1.0) * 0.9;
            tf.translation = stage + Vec3::new(0.0, 0.0, 1.75 - lunge);
            tf.rotation = Quat::IDENTITY;
        } else {
            tf.translation = stage + Vec3::new(0.0, 0.0, if act.is_stan { 1.75 } else { -1.75 });
            tf.rotation = Quat::IDENTITY;
        }
    }

    // DBZ Ki Auras & Teleport Sonic Rings
    for (mut atf, mut vis, aura) in &mut vfx.aura_q {
        if beat >= 3 && beat <= 5 {
            *vis = Visibility::Visible;
            let z_off = if beat == 4 {
                if aura.is_stan { 2.05 } else { -2.05 }
            } else if beat == 5 {
                if aura.is_stan { 1.5 } else { -1.5 }
            } else if aura.is_stan {
                1.75
            } else {
                -1.75
            };
            let base = if beat >= 4 { fight_center } else { stage };
            let pulse = 1.0 + (bt * 20.0).sin() * 0.22;
            atf.translation = base + Vec3::new(0.0, 1.35, z_off);
            atf.scale = Vec3::new(pulse, 1.0 + (pulse - 1.0) * 0.5, pulse);
        } else {
            *vis = Visibility::Hidden;
        }
    }
    for (mut rtf, mut vis, ring) in &mut vfx.ring_q {
        if beat == 4 {
            *vis = Visibility::Visible;
            let prog = (bt * 2.4 + ring.phase) % 1.0;
            rtf.translation = fight_center + Vec3::Y * 1.3;
            rtf.scale = Vec3::splat(0.5 + prog * 3.8);
        } else {
            *vis = Visibility::Hidden;
        }
    }

    // Buildings shatter & sink during Beat 4 & stay ruined in Beats 5-6!
    for (mut btf, bldg) in &mut vfx.bldg_q {
        if beat >= 4 {
            let ruin = if beat == 4 {
                ((bt - bldg.index as f32 * 0.45).max(0.0) * 0.45).min(0.75)
            } else {
                0.75
            };
            btf.translation = stage + Vec3::new(bldg.base_x, bldg.base_y * (1.0 - ruin), bldg.base_z);
            btf.rotation = Quat::from_rotation_z(ruin * if bldg.index % 2 == 0 { 0.24 } else { -0.24 });
        } else {
            btf.translation = stage + Vec3::new(bldg.base_x, bldg.base_y, bldg.base_z);
            btf.rotation = Quat::IDENTITY;
        }
    }
    for (mut rtf, mut vis, rb) in &mut vfx.rubble_q {
        if beat >= 4 {
            *vis = Visibility::Visible;
            let cyc = (bt * 0.75 + rb.phase) % 1.8;
            let y = (rb.vel.y * cyc - 4.5 * cyc * cyc).max(-0.3);
            rtf.translation = rb.base_pos + Vec3::new(rb.vel.x * cyc, y, rb.vel.z * cyc);
            rtf.rotation = Quat::from_euler(EulerRot::XYZ, bt * 3.0 + rb.phase, bt * 2.2, 0.0);
        } else {
            *vis = Visibility::Hidden;
        }
    }

    // DBZ Anime Camera Cuts
    if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
        let push = (bt / 5.0).min(1.0);
        match beat {
            0 | 2 => {
                // Wide side shot showing BOTH Stan & Glenn clearly with the Beer Tower between them!
                c_tf.translation = stage + Vec3::new(4.8 - push * 0.4, 2.25, 0.3);
                c_tf.look_at(stage + Vec3::new(0.0, 1.35, 0.0), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.72;
                }
            }
            1 | 3 => {
                // Close-up on muscular Glenn (Beat 3 extreme close-up on his tears & rage!)
                let dist_z = if beat == 3 { -0.82 } else { -0.62 };
                c_tf.translation = stage + Vec3::new(0.45 - push * 0.15, 1.82, dist_z);
                c_tf.look_at(stage + Vec3::new(0.0, 1.80, -1.75), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = if beat == 3 { 0.48 - push * 0.05 } else { 0.58 };
                }
            }
            4 => {
                // Zoomed-out high-speed DBZ teleport tracking angle showing both fighters & collapsing buildings!
                let ang = 0.8 + bt * 0.95;
                let dist = 8.4;
                c_tf.translation = fight_center + Vec3::new(ang.sin() * dist, 2.6, ang.cos() * dist);
                c_tf.look_at(fight_center + Vec3::Y * 1.4, Vec3::Y);
                c_tf.rotate_local_z((bt * 4.5).sin() * 0.05);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.80;
                }
            }
            5 => {
                // Mid-air standoff above ruined skyline
                c_tf.translation = fight_center + Vec3::new(3.6, 1.4, 3.8 - push * 0.5);
                c_tf.look_at(fight_center + Vec3::Y * 1.2, Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.72;
                }
            }
            _ => {
                // Heroic shoulder-to-shoulder alliance shot
                c_tf.translation = stage + Vec3::new(0.2, 1.65, 3.8 - push * 0.4);
                c_tf.look_at(stage + Vec3::new(0.0, 1.55, 0.2), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.66;
                }
            }
        }
    }
}

pub fn exit_glenn_cutscene(
    mut commands: Commands,
    mut stats: ResMut<PlayerStats>,
    mut glenn_q: Query<&mut CityGlenn>,
    ent_q: Query<Entity, With<GlennCutsceneEntity>>,
    ov_q: Query<Entity, With<GlennCutsceneOverlay>>,
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
    for mut g in &mut glenn_q {
        g.recruited = true;
    }
    stats.glenn_recruited = true;
    stats.hearts = (stats.hearts + 2).min(stats.max_hearts);
    stats.dash_energy = 100.0;
    stats.tendy_credits += 25;
    stats.hint_text = "GLENN JOINED YOUR SQUAD! +2 Hearts, +25 Credits.".to_string();
    play_viking_chant_music();
}
