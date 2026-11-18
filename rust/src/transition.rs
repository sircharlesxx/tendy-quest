//! Scene transitions: every `GameState` change fades through black instead of hard-cutting.
//!
//! Call sites keep writing `next_state.set(GameState::Foo)`; the parameter type is
//! [`SceneTransition`] instead of `ResMut<NextState<GameState>>`. A request is queued in
//! [`SceneFade`] and the real `NextState` is only written once the full-screen overlay is opaque:
//!
//! `Idle -> FadeOut (~0.25 s) -> apply NextState -> Hold (a few frames, black) -> FadeIn (~0.35 s)`
//!
//! Rules:
//! * Only one transition can be pending. Requests made during `FadeOut`/`Apply` are ignored, so
//!   cutscene systems that re-request every frame are harmless and the first request wins.
//! * A request during `Hold` (already black) applies immediately; a request during `FadeIn`
//!   reverses into `FadeOut` from the current opacity.
//! * While a transition is pending (`FadeOut`/`Apply`) every state-scoped Update set is frozen
//!   through [`sim_running`] (wired in `main.rs`). The last frame fades to black as a still, so the
//!   hero cannot die, collect, or trigger a second transition mid-fade, and one-shot end-of-scene
//!   code in cutscene systems still runs exactly once.
//! * The heavy `OnEnter` spawn frame (e.g. the whole city) happens during `Hold`, behind black.
//! * Timing uses `Time<Real>` with delta clamped to 50 ms, so a long spawn frame or the help pause
//!   (which pauses `Time<Virtual>`) never skips or stalls a fade.
//! * The app boots black (`Hold` + `waiting_boot`) and only fades in once the startup glTF scenes
//!   (hero + Level 1) report loaded or failed, or after [`BOOT_TIMEOUT_SECS`].

use bevy::asset::RecursiveDependencyLoadState;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use crate::state::GameState;

const FADE_OUT_SECS: f32 = 0.25;
const FADE_IN_SECS: f32 = 0.35;
/// Death gets a slightly heavier, slower fade.
const GAME_OVER_FADE_OUT_SECS: f32 = 0.45;
const GAME_OVER_FADE_IN_SECS: f32 = 0.55;
/// Frames kept fully black after the state is applied so the spawn frame and the first GPU
/// upload/pipeline-compile frames are hidden.
const HOLD_FRAMES: u8 = 3;
const HOLD_MIN_SECS: f32 = 0.05;
/// Clamp for real delta so a 500 ms hitch doesn't jump straight to the end of a fade.
const MAX_FADE_DT: f32 = 0.05;
/// Give up waiting for startup scenes after this long (a failed/slow asset must not block play).
pub const BOOT_TIMEOUT_SECS: f32 = 6.0;
/// Minimum time after entering GameOver / StageClear before restart input is accepted.
pub const RESTART_LOCKOUT_SECS: f32 = 0.6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FadePhase {
    Idle,
    FadeOut,
    /// Opaque; the queued state is written to `NextState` this frame (used to skip the fade-out).
    Apply,
    Hold,
    FadeIn,
}

#[derive(Resource)]
pub struct SceneFade {
    pub phase: FadePhase,
    /// Overlay opacity, 0 = transparent, 1 = black.
    pub alpha: f32,
    pending: Option<GameState>,
    fade_out_secs: f32,
    fade_in_secs: f32,
    hold_frames: u8,
    hold_secs: f32,
    /// True until the startup scenes are loaded (or the boot timeout passes).
    waiting_boot: bool,
    boot_elapsed: f32,
    /// Real seconds since the last state change was applied by this module.
    since_apply: f32,
}

impl Default for SceneFade {
    fn default() -> Self {
        // Boot black: the first frames are a Hold that waits for startup assets, then fades in.
        Self {
            phase: FadePhase::Hold,
            alpha: 1.0,
            pending: None,
            fade_out_secs: FADE_OUT_SECS,
            fade_in_secs: FADE_IN_SECS,
            hold_frames: HOLD_FRAMES,
            hold_secs: 0.0,
            waiting_boot: true,
            boot_elapsed: 0.0,
            since_apply: 0.0,
        }
    }
}

impl SceneFade {
    fn request(&mut self, target: GameState, from_black: bool) {
        if self.pending.is_some() || matches!(self.phase, FadePhase::FadeOut | FadePhase::Apply) {
            return;
        }
        let slow = target == GameState::GameOver;
        self.fade_out_secs = if slow { GAME_OVER_FADE_OUT_SECS } else { FADE_OUT_SECS };
        self.fade_in_secs = if slow { GAME_OVER_FADE_IN_SECS } else { FADE_IN_SECS };
        self.pending = Some(target);
        if from_black || self.phase == FadePhase::Hold || self.alpha >= 0.999 {
            self.alpha = 1.0;
            self.phase = FadePhase::Apply;
        } else {
            // From Idle, or reversing a FadeIn from its current opacity.
            self.phase = FadePhase::FadeOut;
        }
    }

    /// True while a requested transition hasn't been applied yet.
    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }
}

/// Drop-in replacement for `ResMut<NextState<GameState>>` at transition call sites.
#[derive(SystemParam)]
pub struct SceneTransition<'w> {
    fade: ResMut<'w, SceneFade>,
}

impl SceneTransition<'_> {
    /// Fade out, switch to `state` while black, fade in. Ignored if a transition is pending.
    pub fn set(&mut self, state: GameState) {
        self.fade.request(state, false);
    }

    /// Switch while the screen is (or is forced) black, skipping the fade-out.
    pub fn set_from_black(&mut self, state: GameState) {
        self.fade.request(state, true);
    }

    /// True once the current state has been settled for `lockout` real seconds and no fade is
    /// running, so end-screen input isn't accepted mid-fade or from a held button.
    pub fn input_unlocked(&self, lockout: f32) -> bool {
        self.fade.phase == FadePhase::Idle && self.fade.since_apply >= lockout
    }
}

/// Run condition: false while a transition is pending (fade-out and the apply frame).
pub fn sim_running(fade: Res<SceneFade>) -> bool {
    !fade.is_pending()
}

/// Marker for the persistent black overlay.
#[derive(Component)]
pub struct FadeOverlay;

pub fn spawn_fade_overlay(mut commands: Commands) {
    commands.spawn((
        FadeOverlay,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(Color::BLACK),
        // Above HUD, letterbox bars, subtitles and end cards.
        GlobalZIndex(i32::MAX - 1),
        FocusPolicy::Pass,
    ));
}

/// Advances the fade state machine and writes the queued state into `NextState` once opaque.
/// Runs in `PostUpdate`, after every Update-system request of the frame.
pub fn drive_scene_fade(
    real: Res<Time<Real>>,
    mut fade: ResMut<SceneFade>,
    mut next_state: ResMut<NextState<GameState>>,
    mut overlay_q: Query<(&mut BackgroundColor, &mut Visibility), With<FadeOverlay>>,
) {
    let dt = real.delta_secs().min(MAX_FADE_DT);
    fade.since_apply += dt;

    if fade.phase == FadePhase::FadeOut {
        let step = dt / fade.fade_out_secs.max(0.01);
        fade.alpha = (fade.alpha + step).min(1.0);
        if fade.alpha >= 1.0 {
            fade.phase = FadePhase::Apply;
        }
    }
    match fade.phase {
        FadePhase::Apply => {
            if let Some(target) = fade.pending.take() {
                next_state.set(target);
            }
            fade.alpha = 1.0;
            fade.phase = FadePhase::Hold;
            fade.hold_frames = HOLD_FRAMES;
            fade.hold_secs = 0.0;
            fade.since_apply = 0.0;
        }
        FadePhase::Hold => {
            fade.alpha = 1.0;
            fade.hold_frames = fade.hold_frames.saturating_sub(1);
            fade.hold_secs += dt;
            if fade.hold_frames == 0 && fade.hold_secs >= HOLD_MIN_SECS && !fade.waiting_boot {
                fade.phase = FadePhase::FadeIn;
            }
        }
        FadePhase::FadeIn => {
            let step = dt / fade.fade_in_secs.max(0.01);
            fade.alpha = (fade.alpha - step).max(0.0);
            if fade.alpha <= 0.0 {
                fade.phase = FadePhase::Idle;
            }
        }
        FadePhase::Idle | FadePhase::FadeOut => {}
    }

    // Ease the visible ramp so the darkening/brightening feels even.
    let a = fade.alpha.clamp(0.0, 1.0);
    let shown = a * a * (3.0 - 2.0 * a);
    for (mut bg, mut vis) in &mut overlay_q {
        bg.0 = Color::srgba(0.0, 0.0, 0.0, shown);
        let want = if a <= 0.0 { Visibility::Hidden } else { Visibility::Visible };
        if *vis != want {
            *vis = want;
        }
    }
}

/// Holds the boot black screen until every startup glTF scene (hero, kitchen, enemies) has
/// finished loading or failed, or the timeout passes, then lets the fade-in start and tells the
/// HTML shell to fade its loading screen.
pub fn wait_for_boot_assets(
    real: Res<Time<Real>>,
    asset_server: Res<AssetServer>,
    mut fade: ResMut<SceneFade>,
    scenes: Query<&SceneRoot>,
) {
    if !fade.waiting_boot {
        return;
    }
    // Unclamped: the timeout is about wall-clock time.
    fade.boot_elapsed += real.delta_secs();
    let all_settled = scenes.iter().all(|s| {
        matches!(
            asset_server.get_recursive_dependency_load_state(s.0.id()),
            Some(RecursiveDependencyLoadState::Loaded | RecursiveDependencyLoadState::Failed(_))
        )
    });
    if all_settled || fade.boot_elapsed >= BOOT_TIMEOUT_SECS {
        fade.waiting_boot = false;
        crate::audio::notify_boot_ready();
    }
}
