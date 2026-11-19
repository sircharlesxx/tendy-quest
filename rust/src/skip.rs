//! Hold-to-skip for every cutscene state.
//!
//! Holding Enter (keyboard), Start (gamepad, polled in `index.html`) or the touch SKIP button for
//! [`HOLD_SECS`] raises a skip flag and stops any speaking voice. Each cutscene's own update system
//! reads [`skipping`] and jumps its timer/beat to the end, so its normal completion code (flags,
//! rewards, next state, OnExit cleanup and music) runs exactly once, the same as watching it out.
//!
//! The flag is a plain atomic rather than a resource so the cutscene systems only need one call,
//! not an extra system parameter. It is cleared on entering any cutscene state, so a skipped
//! cutscene that chains into another one (Mom victory -> city intro) doesn't skip the second too.
//!
//! Debug: `?debug=1&autoskip=1` triggers the skip a few seconds into each cutscene (headless tests).

use std::sync::atomic::{AtomicBool, Ordering};

use bevy::prelude::*;

use crate::state::GameState;
use crate::transition::SceneFade;

/// Hold time before a skip fires, long enough that a stray tap doesn't skip a scene.
const HOLD_SECS: f32 = 0.6;
/// Debug `autoskip=1`: seconds into a cutscene before the skip triggers itself.
const AUTOSKIP_AFTER_SECS: f32 = 3.0;

static SKIP_REQUESTED: AtomicBool = AtomicBool::new(false);

pub const CUTSCENE_STATES: [GameState; 11] = [
    GameState::HeadphoneCutscene,
    GameState::Cutscene,
    GameState::MomVictoryCutscene,
    GameState::CityIntroCutscene,
    GameState::TjCutscene,
    GameState::MikeCutscene,
    GameState::DerekCutscene,
    GameState::NateCutscene,
    GameState::GlennCutscene,
    // Registered so hold-to-skip jumps cleanly out of the optional boardwalk cutscene and restores
    // CityPlay position, audio, and DOM filters via `exit_chillary_cutscene`.
    GameState::ChillaryCutscene,
    GameState::LexieVictoryCutscene,
];

/// True once the player has skipped the current cutscene. Cutscene systems treat this as
/// "time is up and no voice is speaking".
pub fn skipping() -> bool {
    SKIP_REQUESTED.load(Ordering::Relaxed)
}

#[derive(Resource, Default)]
struct SkipHold {
    held_secs: f32,
    /// False until the skip input has been released once in this cutscene, so a key or button
    /// still held from gameplay can't start the hold.
    armed: bool,
    in_scene_secs: f32,
    /// Last value pushed to the JS shell, to avoid calling it every frame.
    js_active: bool,
    js_progress_pct: u8,
}

#[derive(Component)]
struct SkipPrompt;

#[derive(Component)]
struct SkipPromptFill;

pub struct CutsceneSkipPlugin;

impl Plugin for CutsceneSkipPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SkipHold>()
            .add_systems(Startup, spawn_skip_prompt)
            .add_systems(Update, update_cutscene_skip);
        for s in CUTSCENE_STATES {
            app.add_systems(OnEnter(s), reset_cutscene_skip);
        }
    }
}

fn reset_cutscene_skip(mut hold: ResMut<SkipHold>) {
    SKIP_REQUESTED.store(false, Ordering::Relaxed);
    hold.held_secs = 0.0;
    hold.armed = false;
    hold.in_scene_secs = 0.0;
}

fn spawn_skip_prompt(mut commands: Commands) {
    commands
        .spawn((
            SkipPrompt,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(16.0),
                bottom: Val::Px(14.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)),
            BorderRadius::all(Val::Px(8.0)),
            // Above cutscene letterbox bars and subtitle cards, below the fade overlay.
            GlobalZIndex(i32::MAX - 2),
            Visibility::Hidden,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("Hold ENTER / START to skip"),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.82)),
            ));
            p.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(3.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.18)),
            ))
            .with_children(|track| {
                track.spawn((
                    SkipPromptFill,
                    Node {
                        width: Val::Percent(0.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(1.0, 0.84, 0.25)),
                ));
            });
        });
}

fn update_cutscene_skip(
    real: Res<Time<Real>>,
    state: Res<State<GameState>>,
    keys: Res<ButtonInput<KeyCode>>,
    fade: Res<SceneFade>,
    mut hold: ResMut<SkipHold>,
    mut prompt_q: Query<&mut Visibility, With<SkipPrompt>>,
    mut fill_q: Query<&mut Node, With<SkipPromptFill>>,
) {
    let in_cutscene = CUTSCENE_STATES.contains(state.get());
    let dt = real.delta_secs().min(0.05);
    let skipped = skipping();

    if in_cutscene && !skipped {
        // JS side covers gamepad Start and the touch SKIP button.
        let held = keys.pressed(KeyCode::Enter)
            || keys.pressed(KeyCode::NumpadEnter)
            || crate::audio::skip_input_held();
        if !held {
            hold.armed = true;
        }
        if fade.is_pending() {
            // A transition is already under way; don't build up a skip behind the fade.
            hold.held_secs = 0.0;
        } else {
            // Wall clock (unclamped) so the debug autoskip fires on slow software renderers too.
            hold.in_scene_secs += real.delta_secs();
            if held && hold.armed {
                hold.held_secs += dt;
            } else {
                hold.held_secs = 0.0;
            }
            let auto = crate::audio::debug_autoskip_enabled()
                && hold.in_scene_secs >= AUTOSKIP_AFTER_SECS;
            if hold.held_secs >= HOLD_SECS || auto {
                SKIP_REQUESTED.store(true, Ordering::Relaxed);
                hold.held_secs = 0.0;
                crate::audio::stop_voice();
            }
        }
    } else {
        hold.held_secs = 0.0;
    }

    let show = in_cutscene && !skipping();
    let progress = (hold.held_secs / HOLD_SECS).clamp(0.0, 1.0);
    // Touch devices get the HTML SKIP button (with its own fill) instead of the keyboard prompt.
    let touch_ui = crate::audio::skip_touch_ui();
    for mut vis in &mut prompt_q {
        let want = if show && !touch_ui {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
    }
    for mut node in &mut fill_q {
        node.width = Val::Percent(progress * 100.0);
    }
    let pct = (progress * 100.0) as u8;
    if show != hold.js_active || pct != hold.js_progress_pct {
        hold.js_active = show;
        hold.js_progress_pct = pct;
        crate::audio::set_cutscene_skip(show, progress);
    }
}
