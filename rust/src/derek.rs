use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use crate::audio::{
    is_voice_speaking, play_cutscene_music, play_sfx, play_viking_chant_music, speak_voice,
};
use crate::city::{CityWorldMarker, PUB217_SCALE};
use crate::player::Player;
use crate::state::{GameState, LevelEntity, PlayerStats};

/// Pub 217 Derek NPC. `was_inside` edge-triggers `DerekCutscene` once on approach;
/// `derek_return_city` restores `saved_city_pos` on return from the isolated `z = -310` stage
/// and `recruited` switches Derek to follow Stan across Cedar Rapids.
#[derive(Component)]
pub struct CityDerek {
    pub was_inside: bool,
    pub recruited: bool,
}

#[derive(Component)]
pub struct DerekCutsceneEntity;

#[derive(Component)]
pub struct DerekCutsceneOverlay;

#[derive(Component)]
pub struct DerekSpeakerText;

#[derive(Component)]
pub struct DerekBadgeText;

#[derive(Component)]
pub struct DerekSubtitleText;

#[derive(Component)]
pub struct DerekCutsceneActor {
    pub is_stan: bool,
}

#[derive(Component)]
pub struct DerekTearDrop {
    pub base_pos: Vec3,
    pub phase: f32,
    pub min_beat: u8,
}

#[derive(Component)]
pub struct DerekClashSpark {
    pub angle: f32,
    pub radius: f32,
}

#[derive(Component)]
pub struct DerekArmPivot {
    pub is_left: bool,
    pub is_stan: bool,
}

#[derive(Component)]
pub struct DerekKiAura {
    pub is_stan: bool,
}

#[derive(Component)]
pub struct DerekBeamHalf {
    pub is_stan: bool,
}

#[derive(Component)]
pub struct DerekBeamClashOrb;

#[derive(Component)]
pub struct DerekShockRing {
    pub phase: f32,
}

#[derive(Component)]
pub struct DerekDbzRock {
    pub base_pos: Vec3,
    pub phase: f32,
}

#[derive(Resource, Default)]
pub struct DerekCutsceneState {
    pub beat: u8,
    pub beat_t: f32,
    pub fight_sfx_played: bool,
}

pub struct DerekMaterials {
    skin: Handle<StandardMaterial>,
    hoodie: Handle<StandardMaterial>,
    hoodie_dark: Handle<StandardMaterial>,
    beard: Handle<StandardMaterial>,
    hair: Handle<StandardMaterial>,
    pants: Handle<StandardMaterial>,
    shoe: Handle<StandardMaterial>,
    eye_white: Handle<StandardMaterial>,
    eye_pupil: Handle<StandardMaterial>,
    tear: Handle<StandardMaterial>,
}

pub fn make_derek_materials(materials: &mut Assets<StandardMaterial>) -> DerekMaterials {
    DerekMaterials {
        skin: materials.add(StandardMaterial {
            base_color: Color::srgb(0.92, 0.76, 0.66),
            perceptual_roughness: 0.65,
            ..default()
        }),
        hoodie: materials.add(StandardMaterial {
            base_color: Color::srgb(0.24, 0.25, 0.28),
            perceptual_roughness: 0.85,
            ..default()
        }),
        hoodie_dark: materials.add(StandardMaterial {
            base_color: Color::srgb(0.15, 0.16, 0.18),
            perceptual_roughness: 0.9,
            ..default()
        }),
        beard: materials.add(StandardMaterial {
            base_color: Color::srgb(0.11, 0.08, 0.06),
            perceptual_roughness: 0.92,
            ..default()
        }),
        hair: materials.add(StandardMaterial {
            base_color: Color::srgb(0.14, 0.10, 0.07),
            perceptual_roughness: 0.88,
            ..default()
        }),
        pants: materials.add(StandardMaterial {
            base_color: Color::srgb(0.12, 0.14, 0.20),
            perceptual_roughness: 0.85,
            ..default()
        }),
        shoe: materials.add(StandardMaterial {
            base_color: Color::srgb(0.08, 0.08, 0.09),
            perceptual_roughness: 0.75,
            ..default()
        }),
        eye_white: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.3,
            ..default()
        }),
        eye_pupil: materials.add(StandardMaterial {
            base_color: Color::srgb(0.08, 0.09, 0.08),
            perceptual_roughness: 0.2,
            ..default()
        }),
        tear: materials.add(StandardMaterial {
            base_color: Color::srgba(0.55, 0.86, 1.0, 0.88),
            emissive: LinearRgba::new(0.18, 0.45, 0.75, 1.0),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.1,
            ..default()
        }),
    }
}

pub fn spawn_derek_rig(
    parent: &mut ChildBuilder,
    meshes: &mut Assets<Mesh>,
    mats: &DerekMaterials,
    is_cutscene: bool,
) {
    use crate::shapes::{self as sh, Torso};

    // Legs & boots
    let leg_mesh = sh::add(meshes, sh::limb_profile(0.70, &[0.12, 0.11, 0.092], 1.0));
    let boot_mesh = sh::add(meshes, sh::shoe(0.20, 0.12, 0.30));
    let sole_mesh = sh::add(meshes, sh::shoe_sole(0.20, 0.12, 0.30));
    for sx in [-0.14_f32, 0.14] {
        parent.spawn((
            Mesh3d(leg_mesh.clone()),
            MeshMaterial3d(mats.pants.clone()),
            Transform::from_xyz(sx, 0.36, 0.0),
        ));
        parent.spawn((
            Mesh3d(boot_mesh.clone()),
            MeshMaterial3d(mats.shoe.clone()),
            Transform::from_xyz(sx, 0.06, 0.04),
        ));
        parent.spawn((
            Mesh3d(sole_mesh.clone()),
            MeshMaterial3d(mats.hoodie_dark.clone()),
            Transform::from_xyz(sx, 0.06, 0.04),
        ));
    }

    // Broad dark-grey hoodie torso with ribbed waistband and kangaroo pocket
    let hoodie = sh::torso(&Torso {
        height: 0.82,
        hip: Vec2::new(0.31, 0.19),
        waist: Vec2::new(0.31, 0.185),
        chest: Vec2::new(0.335, 0.20),
        shoulder: Vec2::new(0.34, 0.19),
        neck: 0.08,
        sq: 0.8,
        chest_fwd: 0.012,
    });
    let trims = sh::radial_shell(&hoodie, 0.008, |p| sh::smoothstep(-0.31, -0.33, p.y)).merged(sh::radial_shell(
        &hoodie,
        0.014,
        |p| {
            sh::smoothstep(0.18, 0.16, p.x.abs())
                * sh::smoothstep(-0.31, -0.29, p.y)
                * sh::smoothstep(-0.05, -0.07, p.y - 0.25 * (0.18 - p.x.abs()).max(0.0))
                * sh::smoothstep(0.05, 0.10, p.z)
        },
    ));
    parent.spawn((
        Mesh3d(sh::add(meshes, hoodie)),
        MeshMaterial3d(mats.hoodie.clone()),
        Transform::from_xyz(0.0, 1.05, 0.0),
    ));
    parent.spawn((
        Mesh3d(sh::add(meshes, trims)),
        MeshMaterial3d(mats.hoodie_dark.clone()),
        Transform::from_xyz(0.0, 1.05, 0.0),
    ));
    // Hoodie collar / hood bunched behind neck
    parent.spawn((
        Mesh3d(meshes.add(Torus::new(0.09, 0.24))),
        MeshMaterial3d(mats.hoodie_dark.clone()),
        Transform::from_xyz(0.0, 1.44, -0.04).with_rotation(Quat::from_rotation_x(0.35)),
    ));
    // Hoodie drawstrings with white aglet tips
    let mut strings = sh::Geo::default();
    let mut tips = sh::Geo::default();
    for sx in [-0.07_f32, 0.07] {
        let (top, bot) = (Vec3::new(sx, 1.40, 0.20), Vec3::new(sx * 1.12, 1.10, 0.226));
        strings = strings.merged(sh::rod(top, bot, 0.012));
        tips = tips.merged(sh::rod(bot, bot + Vec3::new(0.0, -0.045, 0.002), 0.015));
    }
    parent.spawn((
        Mesh3d(sh::add(meshes, strings)),
        MeshMaterial3d(mats.hoodie_dark.clone()),
        Transform::default(),
    ));
    parent.spawn((
        Mesh3d(sh::add(meshes, tips)),
        MeshMaterial3d(mats.eye_white.clone()),
        Transform::default(),
    ));

    // Arms (pivoted if cutscene so Derek can clash/swing!)
    let sleeve = sh::limb_profile(0.66, &[0.108, 0.10, 0.09], 1.0);
    let cuff = sh::radial_shell(&sleeve, 0.008, |p| sh::smoothstep(-0.22, -0.24, p.y));
    let sleeve_mesh = sh::add(meshes, sleeve);
    let cuff_mesh = sh::add(meshes, cuff);
    for (idx, sx) in [-0.44_f32, 0.44].iter().enumerate() {
        let mut arm = parent.spawn((
            Transform::from_xyz(*sx, 1.38, 0.0),
            Visibility::default(),
        ));
        if is_cutscene {
            arm.insert(DerekArmPivot {
                is_left: idx == 0,
                is_stan: false,
            });
        }
        let hand_mesh = sh::add(meshes, sh::hand(0.09, sx.signum(), false));
        arm.with_children(|a| {
            a.spawn((
                Mesh3d(sleeve_mesh.clone()),
                MeshMaterial3d(mats.hoodie.clone()),
                Transform::from_xyz(0.0, -0.26, 0.0),
            ));
            a.spawn((
                Mesh3d(cuff_mesh.clone()),
                MeshMaterial3d(mats.hoodie_dark.clone()),
                Transform::from_xyz(0.0, -0.26, 0.0),
            ));
            a.spawn((
                Mesh3d(hand_mesh),
                MeshMaterial3d(mats.skin.clone()),
                Transform::from_xyz(0.0, -0.64, 0.02),
            ));
        });
    }

    // Neck, head & ears
    let (hr, jaw) = (0.28_f32, 0.12_f32);
    parent.spawn((
        Mesh3d(sh::add(meshes, sh::limb(0.18, 0.08, 0.088))),
        MeshMaterial3d(mats.skin.clone()),
        Transform::from_xyz(0.0, 1.48, -0.01),
    ));
    parent.spawn((
        Mesh3d(sh::add(meshes, sh::head(hr, jaw))),
        MeshMaterial3d(mats.skin.clone()),
        Transform::from_xyz(0.0, 1.76, 0.0),
    ));
    for sx in [-1.0_f32, 1.0] {
        parent.spawn((
            Mesh3d(sh::add(meshes, sh::ear(0.125, sx))),
            MeshMaterial3d(mats.skin.clone()),
            Transform::from_xyz(sx * 0.258, 1.76, -0.01),
        ));
    }

    // Pulled-back dark brown hair + back bun with a hair tie
    parent.spawn((
        Mesh3d(sh::add(meshes, sh::hair_cap(hr, jaw, 0.66, -0.30, 0.035, 0.04))),
        MeshMaterial3d(mats.hair.clone()),
        Transform::from_xyz(0.0, 1.76, 0.0),
    ));
    parent.spawn((
        Mesh3d(sh::add(meshes, sh::superellipsoid(Vec3::new(0.11, 0.10, 0.10), 0.9, 1.0, 20, 14))),
        MeshMaterial3d(mats.hair.clone()),
        Transform::from_xyz(0.0, 1.81, -0.33),
    ));
    parent.spawn((
        Mesh3d(meshes.add(Torus::new(0.05, 0.075))),
        MeshMaterial3d(mats.hoodie_dark.clone()),
        Transform::from_xyz(0.0, 1.81, -0.265).with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
    ));

    // Thick full dark beard & mustache (matching Derek's photo)
    parent.spawn((
        Mesh3d(sh::add(meshes, sh::beard(hr, jaw, 0.11, 0.16, -0.10))),
        MeshMaterial3d(mats.beard.clone()),
        Transform::from_xyz(0.0, 1.76, 0.0),
    ));
    parent.spawn((
        Mesh3d(sh::add(meshes, sh::arc_bar(0.24, &[0.015, 0.034, 0.036, 0.034, 0.015], 0.012, 0.04, 0.8))),
        MeshMaterial3d(mats.beard.clone()),
        Transform::from_xyz(0.0, 1.688, 0.262),
    ));

    // Nose & brows
    parent.spawn((
        Mesh3d(sh::add(meshes, sh::superellipsoid(Vec3::new(0.046, 0.062, 0.05), 1.0, 1.0, 14, 10))),
        MeshMaterial3d(mats.skin.clone()),
        Transform::from_xyz(0.0, 1.745, 0.276),
    ));
    let brow_l = sh::arc_bar(0.11, &[0.016, 0.017, 0.012, 0.008], 0.008, 0.02, 0.7);
    let brow_r = brow_l.clone().mirrored_x();
    let (brow_l, brow_r) = (sh::add(meshes, brow_l), sh::add(meshes, brow_r));
    let eye_mesh = sh::add(meshes, sh::superellipsoid(Vec3::new(0.042, 0.042, 0.036), 1.0, 1.0, 20, 14));
    let pupil_mesh = sh::add(meshes, sh::superellipsoid(Vec3::new(0.024, 0.024, 0.016), 1.0, 1.0, 14, 10));
    for sx in [-0.095_f32, 0.095] {
        parent.spawn((
            Mesh3d(if sx < 0.0 { brow_l.clone() } else { brow_r.clone() }),
            MeshMaterial3d(mats.beard.clone()),
            Transform::from_xyz(sx, 1.86, 0.245)
                .with_rotation(Quat::from_rotation_z(if sx < 0.0 { -0.14 } else { 0.14 })),
        ));
        // Eyes
        parent.spawn((
            Mesh3d(eye_mesh.clone()),
            MeshMaterial3d(mats.eye_white.clone()),
            Transform::from_xyz(sx, 1.80, 0.242),
        ));
        parent.spawn((
            Mesh3d(pupil_mesh.clone()),
            MeshMaterial3d(mats.eye_pupil.clone()),
            Transform::from_xyz(sx, 1.80, 0.272),
        ));
    }
}

pub fn spawn_city_derek(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
) {
    let mats = make_derek_materials(materials);
    commands
        .spawn((
            LevelEntity,
            CityWorldMarker,
            CityDerek {
                was_inside: false,
                recruited: false,
            },
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(0.65)),
            Visibility::default(),
        ))
        .with_children(|p| {
            spawn_derek_rig(p, meshes, &mats, false);
        });
}

pub fn update_city_derek(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut stats: ResMut<PlayerStats>,
    player_q: Query<(&Transform, &Player), Without<CityDerek>>,
    orbit: Res<crate::state::CameraOrbit>,
    mut derek_q: Query<(&mut Transform, &mut CityDerek), Without<Player>>,
    mut next_state: crate::transition::SceneTransition,
    mut d_state: ResMut<DerekCutsceneState>,
    spots: Option<Res<crate::city::CrewSpots>>,
) {
    let Ok((p_tf, player)) = player_q.get_single() else {
        return;
    };
    let dt = time.delta_secs().min(0.05);
    let pub_s = PUB217_SCALE;
    for (mut d_tf, mut derek) in &mut derek_q {
        // `stats.derek_recruited` is the gate's source of truth (checkpoint restore / test links).
        if stats.derek_recruited && !derek.recruited {
            derek.recruited = true;
        }
        // Post-game free roam: recruited crew skip following and idle in Pub 217.
        if derek.recruited && !stats.free_roam {
            let s = (stats.player_scale * 0.72).clamp(1.65, 9.0);
            d_tf.scale = Vec3::splat(s);
            // Flank Stan on his left, out of the camera line at every scale.
            let target = crate::player::crew_follow_target(
                crate::player::CrewSlot::LeftInner,
                p_tf.translation,
                stats.player_scale,
                s,
                orbit.yaw,
            );
            let delta = Vec3::new(target.x - d_tf.translation.x, 0.0, target.z - d_tf.translation.z);
            let dist = delta.length();
            if dist > 32.0 {
                d_tf.translation = Vec3::new(target.x, 0.0, target.z);
            } else if dist > 0.35 {
                let speed = (10.0 + s * 2.4).max(14.0) * if dist > 7.0 { 1.6 } else { 1.0 };
                let step = (speed * dt).min(dist);
                let dir = delta / dist;
                d_tf.translation.x += dir.x * step;
                d_tf.translation.z += dir.z * step;
                d_tf.translation.y = (time.elapsed_secs() * 11.0).sin().abs() * 0.08 * s;
                d_tf.rotation = Quat::from_rotation_y(dir.x.atan2(dir.z));
            } else {
                d_tf.translation.y = 0.0;
                d_tf.rotation = Quat::from_rotation_y(player.facing_yaw + std::f32::consts::PI);
            }
            continue;
        }

        // Waiting: at this build's scattered spot (free roam: in Pub 217), facing the street
        // until Stan is close. Normal follower size outside, pub size inside.
        let (pos, rot) = crate::city::crew_wait_pose(crate::city::CREW_DEREK, &stats, spots.as_deref(), p_tf.translation);
        d_tf.translation = pos;
        d_tf.rotation = rot;
        d_tf.scale = Vec3::splat(if stats.free_roam { pub_s } else { 1.65 });
        let to_p = p_tf.translation - d_tf.translation;
        let flat = Vec3::new(to_p.x, 0.0, to_p.z);

        let dist = flat.length();
        let trigger_r = 2.2 * pub_s;
        let inside = dist < trigger_r;
        let interact = keys.just_pressed(KeyCode::KeyF) || keys.just_pressed(KeyCode::KeyE);

        // Ground level only: standing on the pub roof above them doesn't count.
        if p_tf.translation.y < 4.0
            && stats.driving_car.is_none()
            && stats.riding_scooter.is_none()
            && ((inside && !derek.was_inside) || (dist < 4.5 * pub_s && interact))
        {
            derek.was_inside = true;
            if stats.free_roam {
                // No cutscene replay in free roam: just a friendly line.
                stats.hint_text = "DEREK: \"Beer tower is on me tonight, champ. Go enjoy the city.\"".to_string();
                stats.hint_bump = stats.hint_bump.wrapping_add(1);
                continue;
            }
            stats.saved_city_pos = p_tf.translation;
            stats.derek_return_city = true;
            *d_state = DerekCutsceneState::default();
            next_state.set(GameState::DerekCutscene);
            return;
        }
        derek.was_inside = inside;
    }
}

const DEREK_BEATS: [(&str, &str, &str, &str); 6] = [
    (
        "DEREK (FORMER ARCH-RIVAL)",
        "BEAT 1/6  |  TEARS BY THE BEER TOWER",
        "Stan... I swore the next time we met under these ceiling tiles, one of us wouldn't walk out. Remember the 1990 Ninja Turtles arcade cabinet? You stole my Michelangelo quarter and knocked over a six-foot beer tower!",
        "derek",
    ),
    (
        "STAN",
        "BEAT 2/6  |  CONFESSION IN TEARS",
        "I didn't mean to spill the Donatello pitcher, Derek! Leonardo taught us honor, Raphael taught us fury... and every duel since has left nothing but broken plastic cups and shame!",
        "stan",
    ),
    (
        "DEREK & STAN (KAMEHAMEHA CLASH!)",
        "BEAT 3/6  |  KA-ME-HA-ME-COWABUNGA!!",
        "Ten years of shattered beer towers—KA-ME-HA-ME... COWABUNGA!! TAKE MY SPLINTER-STYLE KAMEHAMEHA!",
        "derek",
    ),
    (
        "STAN (LOCKED IN CLASH)",
        "BEAT 4/6  |  THE EXCEPTION",
        "Wait—look outside! Lexie's pickled garlic is drowning Cedar Rapids! Even Splinter wouldn't let Shredder wreck Pub 217! Today we make an exception!",
        "stan",
    ),
    (
        "DEREK (TEARS OF BROTHERHOOD)",
        "BEAT 5/6  |  THE BEER TOWER TRUCE",
        "Heh... four brothers, one pizza, one impossible beer tower. For tonight only, Stan... our duel waits. Go take down Lexie—and when you get back, the next tower is on me!",
        "derek",
    ),
    (
        "STAN & DEREK",
        "BEAT 6/6  |  COWABUNGA, BROTHER",
        "Cowabunga, brother! Cedar Rapids stands—and when the dust settles, we finish that beer tower together!",
        "stan",
    ),
];

pub fn enter_derek_cutscene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut stats: ResMut<PlayerStats>,
    mut d_state: ResMut<DerekCutsceneState>,
    mut cam_q: Query<(&mut Transform, &mut Projection), With<Camera3d>>,
) {
    *d_state = DerekCutsceneState::default();
    stats.cutscene_timer = 0.0;
    stats.cutscene_beat = 0;

    let stage = Vec3::new(0.0, 0.0, -310.0);
    let mats = make_derek_materials(&mut materials);

    // Dramatic dark Pub 217 floor & ceiling tiles
    let floor_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.09, 0.06, 0.05),
        perceptual_roughness: 0.75,
        ..default()
    });
    let wall_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.06, 0.06, 0.08),
        perceptual_roughness: 0.9,
        ..default()
    });
    commands.spawn((
        DerekCutsceneEntity,
        Mesh3d(meshes.add(Cuboid::new(16.0, 0.2, 16.0))),
        MeshMaterial3d(floor_mat),
        Transform::from_translation(stage + Vec3::new(0.0, -0.1, 0.0)),
    ));
    commands.spawn((
        DerekCutsceneEntity,
        Mesh3d(meshes.add(Cuboid::new(16.0, 0.2, 16.0))),
        MeshMaterial3d(wall_mat.clone()),
        Transform::from_translation(stage + Vec3::new(0.0, 5.2, 0.0)),
    ));

    // Glowing Beer Tower Pyramid in the background between them
    let beer_gold = materials.add(StandardMaterial {
        base_color: Color::srgba(0.95, 0.68, 0.12, 0.90),
        emissive: LinearRgba::new(0.85, 0.52, 0.08, 1.0),
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let rim_silver = materials.add(StandardMaterial {
        base_color: Color::srgb(0.82, 0.85, 0.90),
        metallic: 0.8,
        perceptual_roughness: 0.25,
        ..default()
    });
    for tier in 0..5 {
        let count = 5 - tier;
        let y = 0.45 + tier as f32 * 0.78;
        for c in 0..count {
            let x_off = -2.4 + (c as f32 - (count - 1) as f32 * 0.5) * 0.42;
            commands.spawn((
                DerekCutsceneEntity,
                Mesh3d(meshes.add(Cylinder::new(0.16, 0.72))),
                MeshMaterial3d(beer_gold.clone()),
                Transform::from_translation(stage + Vec3::new(x_off, y, -2.9)),
            ));
            commands.spawn((
                DerekCutsceneEntity,
                Mesh3d(meshes.add(Cylinder::new(0.17, 0.05))),
                MeshMaterial3d(rim_silver.clone()),
                Transform::from_translation(stage + Vec3::new(x_off, y + 0.36, -2.9)),
            ));
        }
    }

    // Dramatic warm spotlight
    commands.spawn((
        DerekCutsceneEntity,
        PointLight {
            color: Color::srgb(1.0, 0.85, 0.65),
            intensity: 260_000.0,
            range: 24.0,
            ..default()
        },
        Transform::from_translation(stage + Vec3::new(0.0, 4.2, 0.5)),
    ));

    // Spawn Derek actor at stage -Z facing +Z
    commands
        .spawn((
            DerekCutsceneEntity,
            DerekCutsceneActor { is_stan: false },
            Transform::from_translation(stage + Vec3::new(0.0, 0.0, -1.55)),
            Visibility::default(),
        ))
        .with_children(|p| {
            spawn_derek_rig(p, &mut meshes, &mats, true);
        });

    // Spawn Stan actor at stage +Z facing -Z
    let stan_plaid = materials.add(StandardMaterial {
        base_color: Color::srgb(0.75, 0.14, 0.16),
        perceptual_roughness: 0.75,
        ..default()
    });
    let stan_gold = materials.add(StandardMaterial {
        base_color: Color::srgb(0.98, 0.82, 0.22),
        metallic: 0.65,
        ..default()
    });
    let stan_purple = materials.add(StandardMaterial {
        base_color: Color::srgb(0.40, 0.15, 0.65),
        perceptual_roughness: 0.45,
        ..default()
    });
    commands
        .spawn((
            DerekCutsceneEntity,
            DerekCutsceneActor { is_stan: true },
            Transform::from_translation(stage + Vec3::new(0.0, 0.0, 1.55))
                .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
            Visibility::default(),
        ))
        .with_children(|p| {
            for sx in [-0.14_f32, 0.14] {
                p.spawn((
                    Mesh3d(meshes.add(Capsule3d::new(0.11, 0.45))),
                    MeshMaterial3d(stan_plaid.clone()),
                    Transform::from_xyz(sx, 0.36, 0.0),
                ));
            }
            p.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.34, 0.78))),
                MeshMaterial3d(stan_plaid.clone()),
                Transform::from_xyz(0.0, 1.04, 0.0),
            ));
            // Vikings belt
            p.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.355, 0.12))),
                MeshMaterial3d(stan_gold.clone()),
                Transform::from_xyz(0.0, 0.82, 0.0),
            ));
            for (idx, sx) in [-0.42_f32, 0.42].iter().enumerate() {
                p.spawn((
                    DerekArmPivot {
                        is_left: idx == 0,
                        is_stan: true,
                    },
                    Transform::from_xyz(*sx, 1.32, 0.0),
                    Visibility::default(),
                ))
                .with_children(|a| {
                    a.spawn((
                        Mesh3d(meshes.add(Capsule3d::new(0.09, 0.42))),
                        MeshMaterial3d(stan_plaid.clone()),
                        Transform::from_xyz(0.0, -0.24, 0.0),
                    ));
                    if idx == 1 {
                        a.spawn((
                            Mesh3d(meshes.add(Cylinder::new(0.04, 0.95))),
                            MeshMaterial3d(stan_purple.clone()),
                            Transform::from_xyz(0.0, -0.52, 0.35)
                                .with_rotation(Quat::from_rotation_x(1.1)),
                        ));
                    }
                });
            }
            // Stan head + beard + Santa/Viking cap
            p.spawn((
                Mesh3d(meshes.add(Sphere::new(0.27))),
                MeshMaterial3d(mats.skin.clone()),
                Transform::from_xyz(0.0, 1.72, 0.0),
            ));
            p.spawn((
                Mesh3d(meshes.add(Sphere::new(0.25))),
                MeshMaterial3d(mats.hair.clone()),
                Transform::from_xyz(0.0, 1.60, 0.07).with_scale(Vec3::new(0.95, 0.88, 0.85)),
            ));
            p.spawn((
                Mesh3d(meshes.add(Cone::new(0.24, 0.36))),
                MeshMaterial3d(stan_plaid.clone()),
                Transform::from_xyz(0.0, 2.02, -0.02),
            ));
            for sx in [-0.09_f32, 0.09] {
                p.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.04))),
                    MeshMaterial3d(mats.eye_white.clone()),
                    Transform::from_xyz(sx, 1.76, 0.235),
                ));
                p.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.022))),
                    MeshMaterial3d(mats.eye_pupil.clone()),
                    Transform::from_xyz(sx, 1.76, 0.258),
                ));
            }
        });

    // Animated 3D teardrops on Derek's & Stan's cheeks
    let tear_mesh = meshes.add(Sphere::new(0.022));
    for (i, sx) in [-0.095_f32, 0.095].iter().enumerate() {
        commands.spawn((
            DerekCutsceneEntity,
            DerekTearDrop {
                base_pos: stage + Vec3::new(*sx, 1.76, -1.29),
                phase: i as f32 * 0.65,
                min_beat: 0,
            },
            Mesh3d(tear_mesh.clone()),
            MeshMaterial3d(mats.tear.clone()),
            Transform::from_translation(stage + Vec3::new(*sx, 1.76, -1.29)),
        ));
        commands.spawn((
            DerekCutsceneEntity,
            DerekTearDrop {
                base_pos: stage + Vec3::new(*sx, 1.72, 1.30),
                phase: i as f32 * 0.55 + 0.3,
                min_beat: 1,
            },
            Mesh3d(tear_mesh.clone()),
            MeshMaterial3d(mats.tear.clone()),
            Transform::from_translation(stage + Vec3::new(*sx, 1.72, 1.30)),
            Visibility::Hidden,
        ));
    }

    // Anime clash sparks + DBZ Ki Auras, Kamehameha Beams, Clash Orb, Shockwave Rings & Floating Rocks
    let spark_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.88, 0.25),
        emissive: LinearRgba::new(4.0, 2.8, 0.4, 1.0),
        ..default()
    });
    let spark_mesh = meshes.add(Cuboid::new(0.05, 0.05, 0.38));
    for i in 0..10 {
        let ang = (i as f32 / 10.0) * std::f32::consts::TAU;
        commands.spawn((
            DerekCutsceneEntity,
            DerekClashSpark {
                angle: ang,
                radius: 0.45 + (i % 3) as f32 * 0.25,
            },
            Mesh3d(spark_mesh.clone()),
            MeshMaterial3d(spark_mat.clone()),
            Transform::from_translation(stage + Vec3::new(0.0, 1.35, 0.0)),
            Visibility::Hidden,
        ));
    }

    let derek_ki_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.25, 0.82, 1.0, 0.34),
        emissive: LinearRgba::new(0.6, 2.4, 4.2, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let stan_ki_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.82, 0.22, 0.34),
        emissive: LinearRgba::new(3.8, 2.6, 0.35, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    for (is_stan, mat, z_pos) in [
        (false, derek_ki_mat.clone(), -2.35_f32),
        (true, stan_ki_mat.clone(), 2.35_f32),
    ] {
        commands.spawn((
            DerekCutsceneEntity,
            DerekKiAura { is_stan },
            Mesh3d(meshes.add(Cylinder::new(0.75, 2.75))),
            MeshMaterial3d(mat),
            Transform::from_translation(stage + Vec3::new(0.0, 1.38, z_pos)),
            Visibility::Hidden,
        ));
    }

    // Twin Kamehameha energy beams meeting in the center
    let derek_beam_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.35, 0.92, 1.0, 0.92),
        emissive: LinearRgba::new(1.2, 4.5, 6.5, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let stan_beam_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.85, 0.38, 1.0, 0.92),
        emissive: LinearRgba::new(4.2, 1.4, 6.2, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    for (is_stan, mat) in [(false, derek_beam_mat), (true, stan_beam_mat)] {
        commands.spawn((
            DerekCutsceneEntity,
            DerekBeamHalf { is_stan },
            Mesh3d(meshes.add(Cylinder::new(0.22, 1.0))),
            MeshMaterial3d(mat),
            Transform::from_translation(stage + Vec3::new(0.0, 1.28, 0.0))
                .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            Visibility::Hidden,
        ));
    }

    // Blazing white/gold Kamehameha clash orb + expanding shockwave rings
    let clash_orb_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        emissive: LinearRgba::new(7.0, 6.2, 4.0, 1.0),
        unlit: true,
        ..default()
    });
    commands.spawn((
        DerekCutsceneEntity,
        DerekBeamClashOrb,
        Mesh3d(meshes.add(Sphere::new(0.45))),
        MeshMaterial3d(clash_orb_mat),
        Transform::from_translation(stage + Vec3::new(0.0, 1.28, 0.0)),
        Visibility::Hidden,
    ));

    let ring_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.92, 0.55, 0.72),
        emissive: LinearRgba::new(3.5, 2.8, 1.0, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    for i in 0..3 {
        commands.spawn((
            DerekCutsceneEntity,
            DerekShockRing {
                phase: i as f32 * 0.33,
            },
            Mesh3d(meshes.add(Torus::new(0.04, 0.55))),
            MeshMaterial3d(ring_mat.clone()),
            Transform::from_translation(stage + Vec3::new(0.0, 1.28, 0.0))
                .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            Visibility::Hidden,
        ));
    }

    // Floating DBZ pebbles/debris rising around the clash
    let rock_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.20, 0.24),
        perceptual_roughness: 0.9,
        ..default()
    });
    let rock_mesh = meshes.add(Cuboid::new(0.14, 0.11, 0.14));
    for i in 0..12 {
        let ang = (i as f32 / 12.0) * std::f32::consts::TAU;
        let r = 1.6 + (i % 3) as f32 * 0.65;
        commands.spawn((
            DerekCutsceneEntity,
            DerekDbzRock {
                base_pos: stage + Vec3::new(ang.cos() * r, 0.25, ang.sin() * r),
                phase: i as f32 * 0.41,
            },
            Mesh3d(rock_mesh.clone()),
            MeshMaterial3d(rock_mat.clone()),
            Transform::from_translation(stage + Vec3::new(ang.cos() * r, 0.25, ang.sin() * r)),
            Visibility::Hidden,
        ));
    }

    if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
        c_tf.translation = stage + Vec3::new(0.45, 1.72, -0.72);
        c_tf.look_at(stage + Vec3::new(0.0, 1.79, -1.55), Vec3::Y);
        if let Projection::Perspective(ref mut p) = *proj {
            p.fov = 0.56;
        }
    }

    let (spk, badge, line, voice) = DEREK_BEATS[0];
    commands
        .spawn((
            DerekCutsceneOverlay,
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
                BackgroundColor(Color::srgba(0.07, 0.05, 0.11, 0.88)),
                BorderColor(Color::srgb(0.95, 0.72, 0.22)),
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
                        DerekSpeakerText,
                        Text::new(spk),
                        TextFont {
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.80, 0.28)),
                    ));
                    hdr.spawn((
                        DerekBadgeText,
                        Text::new(badge),
                        TextFont {
                            font_size: 12.5,
                            ..default()
                        },
                        TextColor(Color::srgb(0.65, 0.85, 1.0)),
                    ));
                });
                card.spawn((
                    DerekSubtitleText,
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
    play_sfx("thunder");
    speak_voice(line, voice);
}

#[derive(SystemParam)]
pub struct DerekVfxQueries<'w, 's> {
    tear_q: Query<
        'w,
        's,
        (&'static mut Transform, &'static mut Visibility, &'static DerekTearDrop),
        (Without<DerekCutsceneActor>, Without<DerekArmPivot>, Without<Camera3d>),
    >,
    spark_q: Query<
        'w,
        's,
        (&'static mut Transform, &'static mut Visibility, &'static DerekClashSpark),
        (
            Without<DerekCutsceneActor>,
            Without<DerekArmPivot>,
            Without<DerekTearDrop>,
            Without<Camera3d>,
        ),
    >,
    ki_q: Query<
        'w,
        's,
        (&'static mut Transform, &'static mut Visibility, &'static DerekKiAura),
        (
            Without<DerekCutsceneActor>,
            Without<DerekArmPivot>,
            Without<DerekTearDrop>,
            Without<DerekClashSpark>,
            Without<Camera3d>,
        ),
    >,
    beam_q: Query<
        'w,
        's,
        (&'static mut Transform, &'static mut Visibility, &'static DerekBeamHalf),
        (
            Without<DerekCutsceneActor>,
            Without<DerekArmPivot>,
            Without<DerekTearDrop>,
            Without<DerekClashSpark>,
            Without<DerekKiAura>,
            Without<Camera3d>,
        ),
    >,
    orb_q: Query<
        'w,
        's,
        (&'static mut Transform, &'static mut Visibility),
        (
            With<DerekBeamClashOrb>,
            Without<DerekCutsceneActor>,
            Without<DerekArmPivot>,
            Without<DerekTearDrop>,
            Without<DerekClashSpark>,
            Without<DerekKiAura>,
            Without<DerekBeamHalf>,
            Without<Camera3d>,
        ),
    >,
    ring_q: Query<
        'w,
        's,
        (&'static mut Transform, &'static mut Visibility, &'static DerekShockRing),
        (
            Without<DerekCutsceneActor>,
            Without<DerekArmPivot>,
            Without<DerekTearDrop>,
            Without<DerekClashSpark>,
            Without<DerekKiAura>,
            Without<DerekBeamHalf>,
            Without<DerekBeamClashOrb>,
            Without<Camera3d>,
        ),
    >,
    rock_q: Query<
        'w,
        's,
        (&'static mut Transform, &'static mut Visibility, &'static DerekDbzRock),
        (
            Without<DerekCutsceneActor>,
            Without<DerekArmPivot>,
            Without<DerekTearDrop>,
            Without<DerekClashSpark>,
            Without<DerekKiAura>,
            Without<DerekBeamHalf>,
            Without<DerekBeamClashOrb>,
            Without<DerekShockRing>,
            Without<Camera3d>,
        ),
    >,
}

pub fn update_derek_cutscene(
    time: Res<Time>,
    mut d_state: ResMut<DerekCutsceneState>,
    mut next_state: crate::transition::SceneTransition,
    mut actor_q: Query<(&mut Transform, &DerekCutsceneActor), Without<Camera3d>>,
    mut arm_q: Query<(&mut Transform, &DerekArmPivot), (Without<DerekCutsceneActor>, Without<Camera3d>)>,
    mut vfx: DerekVfxQueries,
    mut cam_q: Query<(&mut Transform, &mut Projection), With<Camera3d>>,
    mut spk_q: Query<&mut Text, With<DerekSpeakerText>>,
    mut badge_q: Query<&mut Text, (With<DerekBadgeText>, Without<DerekSpeakerText>)>,
    mut sub_q: Query<&mut Text, (With<DerekSubtitleText>, Without<DerekSpeakerText>, Without<DerekBadgeText>)>,
) {
    let dt = time.delta_secs().min(0.05);
    d_state.beat_t += dt;
    // Hold-to-skip: jump to the last beat with its wait elapsed so the normal end path runs.
    let skipping = crate::skip::skipping();
    if skipping {
        d_state.beat = (DEREK_BEATS.len() as u8).saturating_sub(1);
        d_state.beat_t = d_state.beat_t.max(4.0);
    }

    if d_state.beat_t >= 4.0 && (skipping || !is_voice_speaking() || d_state.beat_t >= 18.0) {
        if d_state.beat + 1 >= DEREK_BEATS.len() as u8 {
            next_state.set(GameState::CityPlay);
            return;
        }
        d_state.beat += 1;
        d_state.beat_t = 0.0;
        let (spk, badge, line, voice) = DEREK_BEATS[d_state.beat as usize];
        if d_state.beat == 2 {
            play_sfx("sword_clash");
        } else if d_state.beat == 4 {
            play_sfx("thunder");
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

    let beat = d_state.beat;
    let bt = d_state.beat_t;
    let stage = Vec3::new(0.0, 0.0, -310.0);

    // Animate actor positions (DBZ beam struggle stance at z = ±2.35 in beats 2-3, truce in beat 5)
    for (mut tf, act) in &mut actor_q {
        if beat == 2 || beat == 3 {
            let z = if act.is_stan { 2.35 } else { -2.35 };
            let recoil = (bt * 16.0).sin() * 0.03;
            tf.translation = stage + Vec3::new(0.0, 0.0, z + recoil);
            tf.rotation = Quat::from_rotation_y(if act.is_stan { std::f32::consts::PI } else { 0.0 });
        } else if beat == 5 {
            tf.translation = stage + Vec3::new(if act.is_stan { 0.72 } else { -0.72 }, 0.0, 0.4);
            tf.rotation = Quat::from_rotation_y(0.0);
        } else {
            tf.translation = stage + Vec3::new(0.0, 0.0, if act.is_stan { 1.55 } else { -1.55 });
            tf.rotation = Quat::from_rotation_y(if act.is_stan { std::f32::consts::PI } else { 0.0 });
        }
    }

    // Animate Kamehameha outstretched palms during beats 2-3
    for (mut atf, piv) in &mut arm_q {
        if beat == 2 || beat == 3 {
            let side_in = if piv.is_left { -0.18 } else { 0.18 };
            atf.rotation = Quat::from_euler(EulerRot::XYZ, -1.48, side_in, 0.0);
        } else {
            atf.rotation = Quat::IDENTITY;
        }
    }

    // Animate glistening teardrops streaming down cheeks
    for (mut ttf, mut vis, tear) in &mut vfx.tear_q {
        if beat >= tear.min_beat && beat != 2 {
            *vis = Visibility::Visible;
            let fall = ((bt * 0.42 + tear.phase) % 1.0) * 0.14;
            ttf.translation = tear.base_pos - Vec3::Y * fall;
        } else {
            *vis = Visibility::Hidden;
        }
    }

    // Animate DBZ Kamehameha beam struggle + Ki auras + Clash orb + Shockwaves + Floating rocks
    let beam_active = beat == 2 || beat == 3;
    let clash_z = (bt * 2.2).sin() * 0.48;
    for (mut stf, mut vis, sp) in &mut vfx.spark_q {
        if beam_active {
            *vis = Visibility::Visible;
            let pulse = ((bt * 8.5 + sp.angle) % 1.0) * (sp.radius + 0.35);
            let a = sp.angle + bt * 5.0;
            stf.translation = stage + Vec3::new(a.cos() * pulse, 1.28 + a.sin() * pulse * 0.85, clash_z);
            stf.rotation = Quat::from_rotation_z(a);
        } else {
            *vis = Visibility::Hidden;
        }
    }
    for (mut ktf, mut vis, ki) in &mut vfx.ki_q {
        if beam_active {
            *vis = Visibility::Visible;
            let pulse = 1.0 + (bt * 18.0 + if ki.is_stan { 1.2 } else { 0.0 }).sin() * 0.18;
            ktf.translation = stage + Vec3::new(0.0, 1.38, if ki.is_stan { 2.35 } else { -2.35 });
            ktf.scale = Vec3::new(pulse, 1.0 + (pulse - 1.0) * 0.6, pulse);
        } else {
            *vis = Visibility::Hidden;
        }
    }
    for (mut btf, mut vis, bh) in &mut vfx.beam_q {
        if beam_active {
            *vis = Visibility::Visible;
            let origin_z = if bh.is_stan { 1.85 } else { -1.85 };
            let len = (clash_z - origin_z).abs().max(0.2);
            let mid_z = (origin_z + clash_z) * 0.5;
            let rad_pulse = 1.0 + (bt * 22.0).sin() * 0.22;
            btf.translation = stage + Vec3::new(0.0, 1.28, mid_z);
            btf.scale = Vec3::new(rad_pulse, len, rad_pulse);
        } else {
            *vis = Visibility::Hidden;
        }
    }
    for (mut otf, mut vis) in &mut vfx.orb_q {
        if beam_active {
            *vis = Visibility::Visible;
            let s = 1.0 + (bt * 20.0).sin() * 0.28;
            otf.translation = stage + Vec3::new(0.0, 1.28, clash_z);
            otf.scale = Vec3::splat(s);
        } else {
            *vis = Visibility::Hidden;
        }
    }
    for (mut rtf, mut vis, ring) in &mut vfx.ring_q {
        if beam_active {
            *vis = Visibility::Visible;
            let prog = (bt * 1.8 + ring.phase) % 1.0;
            rtf.translation = stage + Vec3::new(0.0, 1.28, clash_z);
            rtf.scale = Vec3::splat(0.4 + prog * 2.6);
        } else {
            *vis = Visibility::Hidden;
        }
    }
    for (mut rktf, mut vis, rk) in &mut vfx.rock_q {
        if beam_active {
            *vis = Visibility::Visible;
            let lift = ((bt * 0.55 + rk.phase) % 1.0) * 2.4;
            rktf.translation = rk.base_pos + Vec3::Y * lift;
            rktf.rotation = Quat::from_euler(EulerRot::XYZ, bt * 2.0 + rk.phase, bt * 1.5, 0.0);
        } else {
            *vis = Visibility::Hidden;
        }
    }

    // Dramatic close-up & DBZ Kamehameha camera angles
    if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
        let push = (bt / 5.0).min(1.0);
        match beat {
            0 => {
                // Extreme low-angle close-up on Derek's face & tears
                c_tf.translation = stage + Vec3::new(0.32 - push * 0.10, 1.66, -0.78 + push * 0.10);
                c_tf.look_at(stage + Vec3::new(0.0, 1.79, -1.55), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.54 - push * 0.06;
                }
            }
            1 => {
                // Extreme close-up on Stan's face & tears
                c_tf.translation = stage + Vec3::new(-0.34 + push * 0.08, 1.66, 0.76 - push * 0.10);
                c_tf.look_at(stage + Vec3::new(0.0, 1.74, 1.55), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.54 - push * 0.05;
                }
            }
            2 => {
                // Wide/low sweeping DBZ Kamehameha beam-clash angle!
                let ang = 1.25 + (bt * 0.65).sin() * 0.55;
                let dist = 3.35 - push * 0.45;
                c_tf.translation = stage + Vec3::new(ang.sin() * dist, 1.35 + (bt * 2.4).cos() * 0.18, ang.cos() * dist);
                c_tf.look_at(stage + Vec3::new(0.0, 1.35, clash_z), Vec3::Y);
                c_tf.rotate_local_z((bt * 4.5).sin() * 0.045);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.74;
                }
            }
            3 => {
                // Dramatic DBZ over-the-beam close-up looking past the clash orb at Stan & Derek!
                c_tf.translation = stage + Vec3::new(1.55 - push * 0.25, 1.62, clash_z + 0.45);
                c_tf.look_at(stage + Vec3::new(0.0, 1.55, -0.35), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.62;
                }
            }
            4 => {
                // Emotional close-up on Derek dropping his guard + Beer Tower Pyramid behind
                c_tf.translation = stage + Vec3::new(0.45, 1.72, -0.55 - push * 0.15);
                c_tf.look_at(stage + Vec3::new(-0.15, 1.76, -1.65), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.56;
                }
            }
            _ => {
                // Heroic low-angle shot of Stan & Derek side-by-side in front of the Beer Tower Pyramid
                c_tf.translation = stage + Vec3::new(0.25, 1.15, 3.25 - push * 0.35);
                c_tf.look_at(stage + Vec3::new(0.0, 1.65, -0.5), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.66;
                }
            }
        }
    }
}

pub fn exit_derek_cutscene(
    mut commands: Commands,
    mut stats: ResMut<PlayerStats>,
    mut derek_q: Query<&mut CityDerek>,
    ent_q: Query<Entity, With<DerekCutsceneEntity>>,
    ov_q: Query<Entity, With<DerekCutsceneOverlay>>,
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
    for mut d in &mut derek_q {
        d.recruited = true;
    }
    stats.derek_recruited = true;
    stats.hearts = (stats.hearts + 2).min(stats.max_hearts);
    stats.dash_energy = 100.0;
    stats.tendy_credits += 25;
    stats.hint_text = "DEREK JOINED YOUR SQUAD! +2 Hearts, +25 Credits.".to_string();
    play_viking_chant_music();
}
