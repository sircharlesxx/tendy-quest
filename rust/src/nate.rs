use bevy::prelude::*;
use crate::audio::{
    is_voice_speaking, play_cutscene_music, play_sfx, play_viking_chant_music, speak_voice,
};
use crate::city::{CityWorldMarker, PUB217_SCALE};
use crate::player::Player;
use crate::state::{GameState, LevelEntity, PlayerStats};

#[derive(Component)]
pub struct CityNate {
    pub was_inside: bool,
    pub recruited: bool,
}

#[derive(Component)]
pub struct NateCutsceneEntity;

#[derive(Component)]
pub struct NateCutsceneOverlay;

#[derive(Component)]
pub struct NateSpeakerText;

#[derive(Component)]
pub struct NateBadgeText;

#[derive(Component)]
pub struct NateSubtitleText;

#[derive(Component)]
pub struct NateCutsceneActor {
    pub is_stan: bool,
}

#[derive(Component)]
pub struct NateArmPivot {
    pub is_left: bool,
}

#[derive(Component)]
pub struct NateLegPivot {
    pub is_left: bool,
}

#[derive(Component)]
pub struct NateKungFuSpark {
    pub angle: f32,
    pub radius: f32,
}

#[derive(Resource, Default)]
pub struct NateCutsceneState {
    pub beat: u8,
    pub beat_t: f32,
}

pub struct NateMaterials {
    skin: Handle<StandardMaterial>,
    jacket: Handle<StandardMaterial>,
    jacket_trim: Handle<StandardMaterial>,
    beard: Handle<StandardMaterial>,
    cap_navy: Handle<StandardMaterial>,
    cap_white: Handle<StandardMaterial>,
    glasses_frame: Handle<StandardMaterial>,
    glasses_lens: Handle<StandardMaterial>,
    jeans: Handle<StandardMaterial>,
    shoe: Handle<StandardMaterial>,
    eye_white: Handle<StandardMaterial>,
    eye_pupil: Handle<StandardMaterial>,
}

pub fn make_nate_materials(materials: &mut Assets<StandardMaterial>) -> NateMaterials {
    NateMaterials {
        skin: materials.add(StandardMaterial {
            base_color: Color::srgb(0.91, 0.75, 0.64),
            perceptual_roughness: 0.65,
            ..default()
        }),
        jacket: materials.add(StandardMaterial {
            base_color: Color::srgb(0.14, 0.24, 0.44),
            perceptual_roughness: 0.78,
            ..default()
        }),
        jacket_trim: materials.add(StandardMaterial {
            base_color: Color::srgb(0.88, 0.88, 0.92),
            perceptual_roughness: 0.65,
            ..default()
        }),
        beard: materials.add(StandardMaterial {
            base_color: Color::srgb(0.22, 0.14, 0.09),
            perceptual_roughness: 0.90,
            ..default()
        }),
        cap_navy: materials.add(StandardMaterial {
            base_color: Color::srgb(0.08, 0.14, 0.34),
            perceptual_roughness: 0.75,
            ..default()
        }),
        cap_white: materials.add(StandardMaterial {
            base_color: Color::srgb(0.95, 0.95, 0.96),
            perceptual_roughness: 0.65,
            ..default()
        }),
        glasses_frame: materials.add(StandardMaterial {
            base_color: Color::srgb(0.06, 0.06, 0.08),
            metallic: 0.35,
            perceptual_roughness: 0.3,
            ..default()
        }),
        glasses_lens: materials.add(StandardMaterial {
            base_color: Color::srgba(0.68, 0.88, 1.0, 0.40),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.1,
            ..default()
        }),
        jeans: materials.add(StandardMaterial {
            base_color: Color::srgb(0.16, 0.22, 0.36),
            perceptual_roughness: 0.85,
            ..default()
        }),
        shoe: materials.add(StandardMaterial {
            base_color: Color::srgb(0.92, 0.92, 0.94),
            perceptual_roughness: 0.6,
            ..default()
        }),
        eye_white: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            ..default()
        }),
        eye_pupil: materials.add(StandardMaterial {
            base_color: Color::srgb(0.06, 0.06, 0.08),
            ..default()
        }),
    }
}

pub fn spawn_nate_rig(
    parent: &mut ChildBuilder,
    meshes: &mut Assets<Mesh>,
    mats: &NateMaterials,
    is_cutscene: bool,
) {
    use crate::shapes::{self as sh, Torso};

    // Articulated legs for kung-fu kicks
    let leg_mesh = sh::add(meshes, sh::limb_profile(0.66, &[0.112, 0.10, 0.084], 1.0));
    let shoe_mesh = sh::add(meshes, sh::shoe(0.19, 0.11, 0.28));
    let sole_mesh = sh::add(meshes, sh::shoe_sole(0.19, 0.11, 0.28));
    for (idx, sx) in [-0.14_f32, 0.14].iter().enumerate() {
        let mut leg = parent.spawn((Transform::from_xyz(*sx, 0.66, 0.0), Visibility::default()));
        if is_cutscene {
            leg.insert(NateLegPivot { is_left: idx == 0 });
        } else {
            // Why: reuses Nate's existing hip pivots for Free Mode multiplayer stride animation
            // without touching CityNate (which stays static in Pub 217 unless spawned under CrewRigAnimRoot).
            leg.insert(crate::net::CrewStrideLeg { is_left: idx == 0 });
        }
        leg.with_children(|l| {
            l.spawn((
                Mesh3d(leg_mesh.clone()),
                MeshMaterial3d(mats.jeans.clone()),
                Transform::from_xyz(0.0, -0.32, 0.0),
            ));
            l.spawn((
                Mesh3d(shoe_mesh.clone()),
                MeshMaterial3d(mats.shoe.clone()),
                Transform::from_xyz(0.0, -0.60, 0.04),
            ));
            l.spawn((
                Mesh3d(sole_mesh.clone()),
                MeshMaterial3d(mats.cap_navy.clone()),
                Transform::from_xyz(0.0, -0.60, 0.04),
            ));
        });
    }

    // Sports jacket torso + white centre stripe hugging the chest and back
    let jacket = sh::torso(&Torso {
        height: 0.78,
        hip: Vec2::new(0.27, 0.165),
        waist: Vec2::new(0.27, 0.16),
        chest: Vec2::new(0.30, 0.18),
        shoulder: Vec2::new(0.31, 0.17),
        neck: 0.075,
        sq: 0.8,
        chest_fwd: 0.01,
    });
    let stripe = sh::radial_shell(&jacket, 0.006, |p| sh::smoothstep(0.056, 0.046, p.x.abs()));
    parent.spawn((
        Mesh3d(sh::add(meshes, jacket)),
        MeshMaterial3d(mats.jacket.clone()),
        Transform::from_xyz(0.0, 1.04, 0.0),
    ));
    parent.spawn((
        Mesh3d(sh::add(meshes, stripe)),
        MeshMaterial3d(mats.jacket_trim.clone()),
        Transform::from_xyz(0.0, 1.04, 0.0),
    ));

    // Articulated arms for kung-fu blocks & punches
    let sleeve_mesh = sh::add(meshes, sh::limb_profile(0.63, &[0.10, 0.092, 0.08], 1.0));
    for (idx, sx) in [-0.40_f32, 0.40].iter().enumerate() {
        let mut arm = parent.spawn((Transform::from_xyz(*sx, 1.36, 0.0), Visibility::default()));
        if is_cutscene {
            arm.insert(NateArmPivot { is_left: idx == 0 });
        } else {
            arm.insert(crate::net::CrewStrideArm { is_left: idx == 0 });
        }
        let hand_mesh = sh::add(meshes, sh::hand(0.085, sx.signum(), false));
        arm.with_children(|a| {
            a.spawn((
                Mesh3d(sleeve_mesh.clone()),
                MeshMaterial3d(mats.jacket.clone()),
                Transform::from_xyz(0.0, -0.24, 0.0),
            ));
            a.spawn((
                Mesh3d(hand_mesh),
                MeshMaterial3d(mats.skin.clone()),
                Transform::from_xyz(0.0, -0.60, 0.02),
            ));
        });
    }

    // Neck, head & ears
    let (hr, jaw) = (0.265_f32, 0.12_f32);
    parent.spawn((
        Mesh3d(sh::add(meshes, sh::limb(0.18, 0.075, 0.082))),
        MeshMaterial3d(mats.skin.clone()),
        Transform::from_xyz(0.0, 1.47, -0.01),
    ));
    parent.spawn((
        Mesh3d(sh::add(meshes, sh::head(hr, jaw))),
        MeshMaterial3d(mats.skin.clone()),
        Transform::from_xyz(0.0, 1.74, 0.0),
    ));
    for sx in [-1.0_f32, 1.0] {
        parent.spawn((
            Mesh3d(sh::add(meshes, sh::ear(0.12, sx))),
            MeshMaterial3d(mats.skin.clone()),
            Transform::from_xyz(sx * 0.245, 1.74, -0.01),
        ));
    }

    // Full trimmed beard (jaw-hugging shell) & mustache
    parent.spawn((
        Mesh3d(sh::add(meshes, sh::beard(hr, jaw, 0.07, 0.10, -0.12))),
        MeshMaterial3d(mats.beard.clone()),
        Transform::from_xyz(0.0, 1.74, 0.0),
    ));
    parent.spawn((
        Mesh3d(sh::add(meshes, sh::arc_bar(0.21, &[0.012, 0.028, 0.03, 0.028, 0.012], 0.012, 0.035, 0.75))),
        MeshMaterial3d(mats.beard.clone()),
        Transform::from_xyz(0.0, 1.672, 0.248),
    ));

    // Nose, eyes & glasses (rounded frames + bridge + translucent lenses)
    parent.spawn((
        Mesh3d(sh::add(meshes, sh::superellipsoid(Vec3::new(0.04, 0.055, 0.045), 1.0, 1.0, 14, 10))),
        MeshMaterial3d(mats.skin.clone()),
        Transform::from_xyz(0.0, 1.732, 0.268),
    ));
    let lens_mesh = sh::add(meshes, sh::plate(0.132, 0.084, 0.010, 0.35));
    let rim_mesh = sh::add(meshes, sh::frame_ring(0.14, 0.09, 0.009, 0.3));
    for sx in [-0.092_f32, 0.092] {
        let is_lazy = sx > 0.0;
        let eye_r = if is_lazy { 0.046 } else { 0.038 };
        let pupil_x = if is_lazy { sx + 0.030 } else { sx };
        let pupil_y = if is_lazy { 1.768 } else { 1.79 };
        let pupil_z = if is_lazy { 0.248 } else { 0.255 };
        parent.spawn((
            Mesh3d(sh::add(meshes, sh::superellipsoid(Vec3::splat(eye_r), 1.0, 1.0, 20, 14))),
            MeshMaterial3d(mats.eye_white.clone()),
            Transform::from_xyz(sx, 1.79, 0.232).with_scale(Vec3::new(1.0, 1.0, 0.85)),
        ));
        parent.spawn((
            Mesh3d(sh::add(meshes, sh::superellipsoid(Vec3::splat(0.022), 1.0, 1.0, 14, 10))),
            MeshMaterial3d(mats.eye_pupil.clone()),
            Transform::from_xyz(pupil_x, pupil_y, pupil_z).with_scale(Vec3::new(1.0, 1.0, 0.7)),
        ));
        // Glasses lens + frame rim
        parent.spawn((
            Mesh3d(lens_mesh.clone()),
            MeshMaterial3d(mats.glasses_lens.clone()),
            Transform::from_xyz(sx, 1.79, 0.285),
        ));
        parent.spawn((
            Mesh3d(rim_mesh.clone()),
            MeshMaterial3d(mats.glasses_frame.clone()),
            Transform::from_xyz(sx, 1.79, 0.285),
        ));
    }
    // Glasses nose bridge & side temples running back to the ears
    let mut glasses = sh::arc_bar(0.05, &[0.008, 0.008], 0.008, 0.0, 1.0).at(0.0, 1.80, 0.286);
    for sx in [-1.0_f32, 1.0] {
        glasses = glasses.merged(sh::rod(Vec3::new(sx * 0.16, 1.805, 0.282), Vec3::new(sx * 0.243, 1.79, 0.0), 0.008));
    }
    parent.spawn((
        Mesh3d(sh::add(meshes, glasses)),
        MeshMaterial3d(mats.glasses_frame.clone()),
        Transform::default(),
    ));

    // Sports Baseball Cap (navy crown hugging the head + white front logo panel + curved brim)
    parent.spawn((
        Mesh3d(sh::add(meshes, sh::cap_crown(hr, 0.30, 0.075))),
        MeshMaterial3d(mats.cap_navy.clone()),
        Transform::from_xyz(0.0, 1.74, 0.0),
    ));
    parent.spawn((
        Mesh3d(sh::add(meshes, sh::head_panel(hr, 0.0, (1.30, 1.84), (0.42, 0.86), 0.006, |d| {
            0.075 + 0.10 * d.y.max(0.0) + 0.004
        }))),
        MeshMaterial3d(mats.cap_white.clone()),
        Transform::from_xyz(0.0, 1.74, 0.0),
    ));
    parent.spawn((
        Mesh3d(sh::add(meshes, sh::superellipsoid(Vec3::new(0.025, 0.014, 0.025), 1.0, 1.0, 10, 6))),
        MeshMaterial3d(mats.cap_navy.clone()),
        Transform::from_xyz(0.0, 2.03, -0.01),
    ));
    parent.spawn((
        Mesh3d(sh::add(meshes, sh::cap_brim(0.31, 0.25, 0.024, 0.022))),
        MeshMaterial3d(mats.cap_navy.clone()),
        Transform::from_xyz(0.0, 1.853, 0.15).with_rotation(Quat::from_rotation_x(0.10)),
    ));
}

pub fn spawn_city_nate(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
) {
    let mats = make_nate_materials(materials);
    commands
        .spawn((
            LevelEntity,
            CityWorldMarker,
            CityNate {
                was_inside: false,
                recruited: false,
            },
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(0.45)),
            Visibility::default(),
        ))
        .with_children(|p| {
            spawn_nate_rig(p, meshes, &mats, false);
        });
}

pub fn update_city_nate(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut stats: ResMut<PlayerStats>,
    player_q: Query<(&Transform, &Player), Without<CityNate>>,
    orbit: Res<crate::state::CameraOrbit>,
    mut nate_q: Query<(&mut Transform, &mut CityNate, &Visibility), Without<Player>>,
    mut next_state: crate::transition::SceneTransition,
    mut n_state: ResMut<NateCutsceneState>,
    spots: Option<Res<crate::city::CrewSpots>>,
) {
    let Ok((p_tf, player)) = player_q.get_single() else {
        return;
    };
    let dt = time.delta_secs().min(0.05);
    let pub_s = PUB217_SCALE;
    for (mut n_tf, mut nate, n_vis) in &mut nate_q {
        // Skip Free Mode pub greetings while a human player currently controls Nate.
        if stats.free_roam && *n_vis == Visibility::Hidden {
            nate.was_inside = false;
            continue;
        }
        // `stats.nate_recruited` is the gate's source of truth (checkpoint restore / test links).
        if stats.nate_recruited && !nate.recruited {
            nate.recruited = true;
        }
        // Post-game free roam: recruited crew skip following and idle in Pub 217.
        if nate.recruited && !stats.free_roam {
            let s = (stats.player_scale * 0.72).clamp(1.65, 9.0);
            n_tf.scale = Vec3::splat(crate::state::city_body_scale(s));
            // Outer left flank, beyond Derek, out of the camera line at every scale.
            let target = crate::player::crew_follow_target(
                crate::player::CrewSlot::LeftOuter,
                p_tf.translation,
                stats.player_scale,
                s,
                orbit.yaw,
            );
            let delta = Vec3::new(target.x - n_tf.translation.x, 0.0, target.z - n_tf.translation.z);
            let dist = delta.length();
            if dist > 32.0 {
                n_tf.translation = Vec3::new(target.x, 0.0, target.z);
            } else if dist > 0.35 {
                let speed = (10.0 + s * 2.4).max(14.0) * if dist > 7.0 { 1.6 } else { 1.0 };
                let step = (speed * dt).min(dist);
                let dir = delta / dist;
                n_tf.translation.x += dir.x * step;
                n_tf.translation.z += dir.z * step;
                n_tf.translation.y = (time.elapsed_secs() * 11.5).sin().abs() * 0.08 * s;
                n_tf.rotation = Quat::from_rotation_y(dir.x.atan2(dir.z));
            } else {
                n_tf.translation.y = 0.0;
                n_tf.rotation = Quat::from_rotation_y(player.facing_yaw + std::f32::consts::PI);
            }
            continue;
        }

        // Waiting: at this build's scattered spot (free roam: in Pub 217), facing the street
        // until Stan is close. Normal follower size outside, pub size inside.
        let (pos, rot) = crate::city::crew_wait_pose(crate::city::CREW_NATE, &stats, spots.as_deref(), p_tf.translation);
        n_tf.translation = pos;
        n_tf.rotation = rot;
        n_tf.scale = Vec3::splat(crate::state::city_body_scale(if stats.free_roam { pub_s } else { 1.65 }));
        let to_p = p_tf.translation - n_tf.translation;
        let flat = Vec3::new(to_p.x, 0.0, to_p.z);

        let dist = flat.length();
        let trigger_r = 2.0 * pub_s;
        let inside = dist < trigger_r;
        let interact = keys.just_pressed(KeyCode::KeyF) || keys.just_pressed(KeyCode::KeyE);

        // Ground level only: standing on the pub roof above them doesn't count.
        if p_tf.translation.y < 4.0
            && stats.driving_car.is_none()
            && stats.riding_scooter.is_none()
            && ((inside && !nate.was_inside) || (dist < 4.3 * pub_s && interact))
        {
            nate.was_inside = true;
            if stats.free_roam {
                // No cutscene replay in free roam: just a friendly line.
                stats.hint_text = "NATE: \"Saved by Stan again. I am not leaving this booth.\"".to_string();
                stats.hint_bump = stats.hint_bump.wrapping_add(1);
                continue;
            }
            stats.saved_city_pos = p_tf.translation;
            stats.nate_return_city = true;
            *n_state = NateCutsceneState::default();
            next_state.set(GameState::NateCutscene);
            return;
        }
        nate.was_inside = inside;
    }
}

const NATE_BEATS: [(&str, &str, &str, &str); 6] = [
    (
        "NATE (DES MOINES EXPAT)",
        "BEAT 1/6  |  THE CONFESSION",
        "Stan... don't look at me like that. I had to move to Des Moines. The rent, the bills... I didn't have a choice!",
        "nate",
    ),
    (
        "STAN (IN DISBELIEF)",
        "BEAT 2/6  |  UNEMPLOYED LOGIC",
        "MOVE TO DES MOINES?! Why would you leave Cedar Rapids and Pub 217 when you could simply just... NOT have a job?!",
        "stan",
    ),
    (
        "NATE (HEATED ARGUMENT)",
        "BEAT 3/6  |  RESPONSIBILITY?",
        "ARE YOU OUT OF YOUR MIND?! I NEED A JOB TO PAY RENT AND EAT, STAN!!",
        "nate",
    ),
    (
        "STAN (DEADPAN)",
        "BEAT 4/6  |  SO?",
        "So?",
        "stan",
    ),
    (
        "NATE & STAN (KUNG-FU DUEL!)",
        "BEAT 5/6  |  PUB 217 MARTIAL ARTS SHOWDOWN",
        "HYAAAH!! YOU THINK RENT IS OPTIONAL?! TASTE MY DES MOINES FLYING CRANE KICK!!",
        "nate",
    ),
    (
        "NATE (DEFEATED & RECRUITED)",
        "BEAT 6/6  |  RESIGNATION",
        "Ugh... your jobless kung-fu is too strong... Fine. Des Moines can wait. I'm back in Cedar Rapids, Stan - let's go wreck Lexie!",
        "nate",
    ),
];

pub fn enter_nate_cutscene(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut n_state: ResMut<NateCutsceneState>,
    mut cam_q: Query<(&mut Transform, &mut Projection), With<Camera3d>>,
) {
    *n_state = NateCutsceneState::default();
    // Spawn cutscene stage far outside Cedar Rapids (±135m) at z = -520 so city geometry
    // never clips into the shot. Procedural `spawn_nate_rig` faces +Z, while `plaid_hero.glb`
    // faces -Z at Quat::IDENTITY, so they naturally face each other across z = ±1.55.
    let stage = Vec3::new(0.0, 0.0, -520.0);
    let mats = make_nate_materials(&mut materials);

    // Pub 217 dojo floor & spotlights
    let floor_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.11, 0.07, 0.05),
        perceptual_roughness: 0.78,
        ..default()
    });
    let wall_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.05, 0.05, 0.08),
        perceptual_roughness: 0.9,
        ..default()
    });
    commands.spawn((
        NateCutsceneEntity,
        Mesh3d(meshes.add(Cuboid::new(18.0, 0.2, 18.0))),
        MeshMaterial3d(floor_mat),
        Transform::from_translation(stage + Vec3::new(0.0, -0.1, 0.0)),
    ));
    commands.spawn((
        NateCutsceneEntity,
        Mesh3d(meshes.add(Cuboid::new(18.0, 0.2, 18.0))),
        MeshMaterial3d(wall_mat),
        Transform::from_translation(stage + Vec3::new(0.0, 5.4, 0.0)),
    ));

    // Beer Tower glowing in the back corner
    let beer_gold = materials.add(StandardMaterial {
        base_color: Color::srgba(0.95, 0.68, 0.12, 0.90),
        emissive: LinearRgba::new(0.85, 0.52, 0.08, 1.0),
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    for tier in 0..4 {
        let count = 4 - tier;
        let y = 0.45 + tier as f32 * 0.75;
        for c in 0..count {
            let x_off = -2.2 + (c as f32 - (count - 1) as f32 * 0.5) * 0.42;
            commands.spawn((
                NateCutsceneEntity,
                Mesh3d(meshes.add(Cylinder::new(0.15, 0.70))),
                MeshMaterial3d(beer_gold.clone()),
                Transform::from_translation(stage + Vec3::new(x_off, y, -3.2)),
            ));
        }
    }

    commands.spawn((
        NateCutsceneEntity,
        PointLight {
            color: Color::srgb(1.0, 0.90, 0.72),
            intensity: 340_000.0,
            range: 28.0,
            ..default()
        },
        Transform::from_translation(stage + Vec3::new(0.0, 4.4, 0.8)),
    ));

    // Spawn Nate at -Z facing +Z
    commands
        .spawn((
            NateCutsceneEntity,
            NateCutsceneActor { is_stan: false },
            Transform::from_translation(stage + Vec3::new(0.0, 0.0, -1.55)),
            Visibility::default(),
        ))
        .with_children(|p| {
            spawn_nate_rig(p, &mut meshes, &mats, true);
        });

    // Spawn REAL Stan (`models/plaid_hero.glb#Scene0`) at +Z facing -Z
    let hero_scene: Handle<Scene> = asset_server.load("models/plaid_hero.glb#Scene0");
    commands.spawn((
        NateCutsceneEntity,
        NateCutsceneActor { is_stan: true },
        SceneRoot(hero_scene),
        Transform::from_translation(stage + Vec3::new(0.0, 0.0, 1.55)).with_scale(Vec3::splat(1.20)),
    ));

    // Kung-fu clash shockwave sparks (visible in beat 4)
    let spark_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.92, 0.35),
        emissive: LinearRgba::new(5.0, 3.8, 0.6, 1.0),
        unlit: true,
        ..default()
    });
    let spark_mesh = meshes.add(Cuboid::new(0.06, 0.06, 0.45));
    for i in 0..12 {
        let ang = (i as f32 / 12.0) * std::f32::consts::TAU;
        commands.spawn((
            NateCutsceneEntity,
            NateKungFuSpark {
                angle: ang,
                radius: 0.45 + (i % 3) as f32 * 0.28,
            },
            Mesh3d(spark_mesh.clone()),
            MeshMaterial3d(spark_mat.clone()),
            Transform::from_translation(stage + Vec3::new(0.0, 1.35, 0.0)),
            Visibility::Hidden,
        ));
    }

    if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
        c_tf.translation = stage + Vec3::new(0.42, 1.76, -0.72);
        c_tf.look_at(stage + Vec3::new(0.0, 1.76, -1.55), Vec3::Y);
        if let Projection::Perspective(ref mut p) = *proj {
            p.fov = 0.56;
        }
    }

    let (spk, badge, line, voice) = NATE_BEATS[0];
    commands
        .spawn((
            NateCutsceneOverlay,
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
                BackgroundColor(Color::srgba(0.06, 0.05, 0.12, 0.90)),
                BorderColor(Color::srgb(0.32, 0.78, 1.0)),
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
                        NateSpeakerText,
                        Text::new(spk),
                        TextFont {
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.45, 0.88, 1.0)),
                    ));
                    hdr.spawn((
                        NateBadgeText,
                        Text::new(badge),
                        TextFont {
                            font_size: 12.5,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.82, 0.30)),
                    ));
                });
                card.spawn((
                    NateSubtitleText,
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

#[allow(clippy::type_complexity)]
pub fn update_nate_cutscene(
    time: Res<Time>,
    mut n_state: ResMut<NateCutsceneState>,
    mut next_state: crate::transition::SceneTransition,
    mut actor_q: Query<(&mut Transform, &NateCutsceneActor), Without<Camera3d>>,
    mut arm_q: Query<(&mut Transform, &NateArmPivot), (Without<NateCutsceneActor>, Without<Camera3d>)>,
    mut leg_q: Query<(&mut Transform, &NateLegPivot), (Without<NateCutsceneActor>, Without<NateArmPivot>, Without<Camera3d>)>,
    mut spark_q: Query<(&mut Transform, &mut Visibility, &NateKungFuSpark), (Without<NateCutsceneActor>, Without<NateArmPivot>, Without<NateLegPivot>, Without<Camera3d>)>,
    mut cam_q: Query<(&mut Transform, &mut Projection), With<Camera3d>>,
    mut spk_q: Query<&mut Text, With<NateSpeakerText>>,
    mut badge_q: Query<&mut Text, (With<NateBadgeText>, Without<NateSpeakerText>)>,
    mut sub_q: Query<&mut Text, (With<NateSubtitleText>, Without<NateSpeakerText>, Without<NateBadgeText>)>,
) {
    let dt = time.delta_secs().min(0.05);
    n_state.beat_t += dt;
    // Hold-to-skip: jump to the last beat with its wait elapsed so the normal end path runs.
    let skipping = crate::skip::skipping();
    if skipping {
        n_state.beat = (NATE_BEATS.len() as u8).saturating_sub(1);
        n_state.beat_t = n_state.beat_t.max(4.0);
    }

    let min_t = if n_state.beat == 3 { 2.4 } else { 4.0 };
    if n_state.beat_t >= min_t && (skipping || !is_voice_speaking() || n_state.beat_t >= 18.0) {
        if n_state.beat + 1 >= NATE_BEATS.len() as u8 {
            next_state.set(GameState::CityPlay);
            return;
        }
        n_state.beat += 1;
        n_state.beat_t = 0.0;
        let (spk, badge, line, voice) = NATE_BEATS[n_state.beat as usize];
        if n_state.beat == 3 {
            play_sfx("thunder");
        } else if n_state.beat == 4 {
            play_sfx("sword_clash");
            play_sfx("lunge");
        } else if n_state.beat == 5 {
            play_sfx("hit");
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

    let beat = n_state.beat;
    let bt = n_state.beat_t;
    // Spawn cutscene stage far outside Cedar Rapids (±135m) at z = -520 so city geometry
    // never clips into the shot. Procedural `spawn_nate_rig` faces +Z, while `plaid_hero.glb`
    // faces -Z at Quat::IDENTITY, so they naturally face each other across z = ±1.55.
    let stage = Vec3::new(0.0, 0.0, -520.0);

    // Animate actor stances (kung-fu leaps & spinning kicks in beat 4, defeated kneel in beat 5)
    for (mut tf, act) in &mut actor_q {
        if beat == 4 {
            let hop = (bt * 7.5 + if act.is_stan { 1.5 } else { 0.0 }).sin().abs() * 0.65;
            let clash_z = 0.72 + (bt * 9.0).sin().abs() * 0.25;
            tf.translation = stage + Vec3::new(0.0, hop, if act.is_stan { clash_z } else { -clash_z });
            tf.rotation = if act.is_stan {
                Quat::from_rotation_y((bt * 8.0).sin() * 0.35)
            } else {
                Quat::from_rotation_y((bt * 8.0).sin() * 0.35)
            };
        } else if beat == 5 && !act.is_stan {
            // Nate defeated on one knee
            tf.translation = stage + Vec3::new(0.0, -0.35, -1.55);
            tf.rotation = Quat::from_rotation_x(0.28);
        } else {
            tf.translation = stage + Vec3::new(0.0, 0.0, if act.is_stan { 1.55 } else { -1.55 });
            tf.rotation = Quat::IDENTITY;
        }
    }

    // Kung-fu arm blocks & punches
    for (mut atf, piv) in &mut arm_q {
        if beat == 4 {
            let punch = (bt * 16.0 + if piv.is_left { 0.0 } else { 2.2 }).sin() * 1.35;
            atf.rotation = Quat::from_rotation_x(-1.2 + punch);
        } else if beat == 2 {
            // Heated arguing gesture
            atf.rotation = Quat::from_rotation_x(-0.9 + (bt * 10.0).sin() * 0.35);
        } else {
            atf.rotation = Quat::IDENTITY;
        }
    }

    // Kung-fu high crane kicks!
    for (mut ltf, piv) in &mut leg_q {
        if beat == 4 {
            let kick = if piv.is_left {
                (-1.2 + (bt * 12.0).sin() * 0.6).min(0.0)
            } else {
                (bt * 9.0).cos() * 0.35
            };
            ltf.rotation = Quat::from_rotation_x(kick);
        } else {
            ltf.rotation = Quat::IDENTITY;
        }
    }

    // Martial arts impact sparks in beat 4
    for (mut stf, mut vis, sp) in &mut spark_q {
        if beat == 4 {
            *vis = Visibility::Visible;
            let pulse = ((bt * 9.0 + sp.angle) % 1.0) * sp.radius;
            let a = sp.angle + bt * 6.0;
            stf.translation = stage + Vec3::new(a.cos() * pulse, 1.35 + a.sin() * pulse * 0.8, 0.0);
            stf.rotation = Quat::from_rotation_z(a);
        } else {
            *vis = Visibility::Hidden;
        }
    }

    // Dramatic close-up & kung-fu movie camera angles
    if let Ok((mut c_tf, mut proj)) = cam_q.get_single_mut() {
        let push = (bt / 5.0).min(1.0);
        match beat {
            0 | 2 => {
                // Tight dramatic close-up on Nate (glasses, beard, sports cap)
                c_tf.translation = stage + Vec3::new(0.40 - push * 0.12, 1.76, -0.74 + push * 0.10);
                c_tf.look_at(stage + Vec3::new(0.0, 1.76, -1.55), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = if beat == 2 { 0.50 } else { 0.56 - push * 0.05 };
                }
            }
            1 => {
                // Reverse close-up on Stan's face (HeroHead scaled 1.20 sits around y = 1.82..2.55)
                c_tf.translation = stage + Vec3::new(-0.46 + push * 0.12, 2.16, 0.25 - push * 0.12);
                c_tf.look_at(stage + Vec3::new(0.0, 2.14, 1.55), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.54;
                }
            }
            3 => {
                // Extreme Dutch-angle snap close-up on Stan's face for "So?"
                c_tf.translation = stage + Vec3::new(-0.32, 2.15, 0.38 - push * 0.14);
                c_tf.look_at(stage + Vec3::new(0.0, 2.14, 1.55), Vec3::Y);
                c_tf.rotate_local_z(0.09);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.48;
                }
            }
            4 => {
                // Fast Hong-Kong kung-fu movie sweeping orbit!
                let ang = 0.9 + bt * 1.15;
                let dist = 2.55 + (bt * 4.5).sin() * 0.25;
                c_tf.translation = stage + Vec3::new(ang.sin() * dist, 1.45 + (bt * 3.5).cos() * 0.22, ang.cos() * dist);
                c_tf.look_at(stage + Vec3::new(0.0, 1.40, 0.0), Vec3::Y);
                c_tf.rotate_local_z((bt * 5.5).sin() * 0.06);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.70;
                }
            }
            _ => {
                // Low-angle shot looking past Stan down at kneeling defeated Nate
                c_tf.translation = stage + Vec3::new(1.55 - push * 0.25, 1.55, 1.25);
                c_tf.look_at(stage + Vec3::new(0.0, 1.25, -1.55), Vec3::Y);
                if let Projection::Perspective(ref mut p) = *proj {
                    p.fov = 0.64;
                }
            }
        }
    }
}

pub fn exit_nate_cutscene(
    mut commands: Commands,
    mut stats: ResMut<PlayerStats>,
    mut nate_q: Query<&mut CityNate>,
    ent_q: Query<Entity, With<NateCutsceneEntity>>,
    ov_q: Query<Entity, With<NateCutsceneOverlay>>,
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
    for mut n in &mut nate_q {
        n.recruited = true;
    }
    stats.nate_recruited = true;
    stats.hearts = (stats.hearts + 2).min(stats.max_hearts);
    stats.dash_energy = 100.0;
    stats.tendy_credits += 25;
    stats.hint_text = "NATE JOINED YOUR SQUAD! +2 Hearts, +25 Credits.".to_string();
    play_viking_chant_music();
}
