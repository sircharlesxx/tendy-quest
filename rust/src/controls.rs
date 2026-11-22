//! On-screen controls strip and the pause menu.
//!
//! * The strip is a row of small key chips in the bottom-left corner during Playing, BossFight and
//!   CityPlay. It lists only the controls for the current context (Level 1, saber combat, city on
//!   foot, scooter, car), switches to gamepad labels when the last input came from a pad, and is
//!   hidden on touch layouts (the touch buttons label themselves), in cutscenes, while paused and
//!   during fades.
//! * The pause menu replaces the old help card. Esc, H, touch HELP and gamepad Start open it in the
//!   gameplay states (and so does the browser dropping pointer lock); it pauses exactly like the old
//!   card did (`HelpPause` + `Time<Virtual>`, see `ui::gameplay_running`). Buttons: Resume, Sound,
//!   Restart (press twice), Clear save (press twice). Restart re-enters the current state through `SceneTransition`, and the
//!   identity transition runs that state's OnExit, the world reset and its OnEnter while black
//!   (see [`run_pause_restart`]).

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};

use crate::state::{CameraOrbit, GameState, LevelEntity, PlayerStats};
use crate::transition::{FadePhase, SceneFade, SceneTransition, sim_running};
use crate::ui::{EndOverlay, HelpOverlay, HelpPause};

/// Which set of controls applies right now.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ControlContext {
    Level1,
    Saber,
    CityFoot,
    Scooter,
    Car,
    Wheel,
    Casino,
}

impl ControlContext {
    pub fn current(state: &GameState, stats: &PlayerStats) -> Option<Self> {
        match state {
            GameState::Playing if stats.boss_mode => Some(Self::Saber),
            GameState::Playing => Some(Self::Level1),
            GameState::BossFight => Some(Self::Saber),
            GameState::CityPlay if stats.riding_wheel => Some(Self::Wheel),
            GameState::CityPlay if stats.riding_scooter.is_some() => Some(Self::Scooter),
            GameState::CityPlay if stats.driving_car.is_some() => Some(Self::Car),
            GameState::CityPlay => Some(Self::CityFoot),
            GameState::CasinoPlay => Some(Self::Casino),
            _ => None,
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Level1 => "Level 1, the kitchen",
            Self::Saber => "Saber combat",
            Self::CityFoot => "Cedar Rapids on foot",
            Self::Scooter => "Riding a scooter",
            Self::Car => "Driving a car",
            Self::Wheel => "Riding the Ferris wheel",
            Self::Casino => "Tendy Palace Casino",
        }
    }
}

/// One binding: keyboard label, gamepad label ("" = no pad binding), action, shown in the strip.
#[derive(Clone, Copy)]
struct Bind {
    kb: &'static str,
    pad: &'static str,
    action: &'static str,
    strip: bool,
}

const fn bind(kb: &'static str, pad: &'static str, action: &'static str, strip: bool) -> Bind {
    Bind { kb, pad, action, strip }
}

/// Bindings for a context, in strip priority order (the strip keeps the first few on narrow
/// screens). Labels mirror the real bindings in player.rs / scooter.rs / city.rs / stunts.rs and
/// the gamepad mapping in index.html (`pollGamepad`).
fn binds(ctx: ControlContext) -> Vec<Bind> {
    const PAUSE: Bind = bind("ESC", "START", "Pause", true);
    const LOOK: Bind = bind("MOUSE", "R-STICK", "Look", false);
    match ctx {
        ControlContext::Level1 => vec![
            bind("WASD", "L-STICK", "Move", true),
            bind("SPACE", "A", "Jump (x2)", true),
            bind("SHIFT", "B", "Dash", true),
            bind("Q", "Y", "Heal", true),
            PAUSE,
            LOOK,
            bind("WHEEL", "", "Zoom", false),
        ],
        ControlContext::Saber | ControlContext::CityFoot => {
            let mut v = vec![
                // Tap = light swing, hold to charge and release = heavy (no separate heavy key).
                bind("CLICK / J", "RB", "Attack (hold: heavy)", true),
                bind("R-CLICK / R", "LT", "Block (hold)", true),
                bind("SHIFT", "B", "Roll", true),
            ];
            if ctx == ControlContext::CityFoot {
                v.push(bind("F", "X", "Ride / drive / drink / wheel", true));
                v.push(bind("P", "LB", "Phone", true));
            }
            v.extend([
                bind("V", "L3", "Strafe (hold)", true),
                bind("TAB", "R3", "Lock-on", true),
                bind("Q", "Y", "Heal", true),
                PAUSE,
                bind("WASD", "L-STICK", "Move", false),
                LOOK,
                bind("SPACE", "A", "Jump", false),
            ]);
            v
        }
        ControlContext::Scooter => vec![
            bind("W / S", "L-STICK", "Go / brake", true),
            bind("MOUSE or A / D", "R-STICK", "Steer", true),
            bind("SHIFT", "B", "Turbo", true),
            bind("SPACE", "A", "Jump / grind (air: hold to flip)", true),
            bind("F", "X", "Hop off", true),
            PAUSE,
            LOOK,
            // Skate park extras (skatepark.rs), listed in the pause menu.
            bind("SPACE", "A", "Park air: tap for a tailwhip", false),
            bind("A / D", "L-STICK", "In the air: spin (360s)", false),
            bind("A / D", "L-STICK", "Park: grind balance", false),
            bind("W", "L-STICK", "Park: hold dropping in to pump", false),
        ],
        ControlContext::Car => vec![
            bind("WASD", "L-STICK", "Drive", true),
            bind("SHIFT", "B", "Nitro", true),
            bind("SPACE", "A", "Handbrake", true),
            bind("F", "X", "Exit car", true),
            PAUSE,
            LOOK,
        ],
        ControlContext::Wheel => vec![
            bind("MOUSE", "R-STICK", "Look around", true),
            bind("F", "X", "Hop off at the bottom", true),
            bind("WHEEL", "", "Gondola / outside view", true),
            PAUSE,
        ],
        ControlContext::Casino => vec![
            bind("WASD", "L-STICK", "Move", true),
            bind("SPACE", "A", "Jump", true),
            bind("SHIFT", "B", "Dash", true),
            bind("F / E", "X", "Play / Return Portal", true),
            PAUSE,
            LOOK,
            bind("WHEEL", "", "Zoom", false),
        ],
    }
}

fn label(b: &Bind, pad: bool) -> Option<&'static str> {
    let k = if pad { b.pad } else { b.kb };
    (!k.is_empty()).then_some(k)
}

const KEY_COLOR: Color = Color::srgba(1.0, 0.86, 0.32, 0.95);
const KEY_BG: Color = Color::srgba(1.0, 1.0, 1.0, 0.13);
const ACTION_COLOR: Color = Color::srgba(1.0, 1.0, 1.0, 0.86);

/// Spawns `[KEY] action` as two text nodes into `parent`.
fn spawn_key_action(parent: &mut ChildBuilder, key: &str, action: &str, size: f32, key_min_w: f32) {
    parent
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(4.0), Val::Px(1.0)),
                min_width: Val::Px(key_min_w),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(KEY_BG),
            BorderRadius::all(Val::Px(4.0)),
        ))
        .with_children(|k| {
            k.spawn((
                Text::new(key),
                TextLayout::new_with_no_wrap(),
                TextFont {
                    font_size: size,
                    ..default()
                },
                TextColor(KEY_COLOR),
            ));
        });
    parent.spawn((
        Text::new(action),
        TextLayout::new_with_no_wrap(),
        TextFont {
            font_size: size,
            ..default()
        },
        TextColor(ACTION_COLOR),
    ));
}

// ---------------------------------------------------------------------------------------------
// Controls strip
// ---------------------------------------------------------------------------------------------

/// Root of the bottom-left controls strip (positioned by `ui::adapt_ui_to_viewport`).
#[derive(Component)]
pub struct ControlStrip;

fn spawn_control_strip(mut commands: Commands) {
    commands.spawn((
        ControlStrip,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            bottom: Val::Px(10.0),
            width: Val::Px(440.0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            align_items: AlignItems::Center,
            column_gap: Val::Px(5.0),
            row_gap: Val::Px(5.0),
            ..default()
        },
        Visibility::Hidden,
    ));
}

/// Shows/hides the strip and rebuilds its chips when the context, label set or width class changes.
#[allow(clippy::too_many_arguments)]
fn update_control_strip(
    mut commands: Commands,
    state: Res<State<GameState>>,
    stats: Res<PlayerStats>,
    help: Res<HelpPause>,
    fade: Res<SceneFade>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    mut strip_q: Query<(Entity, &mut Visibility), With<ControlStrip>>,
    mut built: Local<Option<(ControlContext, bool, usize)>>,
) {
    let Ok((strip, mut vis)) = strip_q.get_single_mut() else {
        return;
    };
    let ctx = ControlContext::current(state.get(), &stats);
    let show = ctx.is_some()
        && !help.open
        && fade.phase == FadePhase::Idle
        && !stats.phone_open
        && !stats.casino_open
        && !stats.bartender_open
        && !stats.char_select_open
        && !crate::audio::skip_touch_ui();
    let want = if show { Visibility::Inherited } else { Visibility::Hidden };
    if *vis != want {
        *vis = want;
    }
    let (Some(ctx), true) = (ctx, show) else {
        return;
    };
    let pad = crate::audio::last_input_gamepad();
    let w = window_q.get_single().map(|w| w.width()).unwrap_or(1280.0);
    // Narrow screens keep only the most important few chips.
    let max = if w >= 1100.0 {
        usize::MAX
    } else if w >= 760.0 {
        6
    } else {
        4
    };
    if *built == Some((ctx, pad, max)) {
        return;
    }
    *built = Some((ctx, pad, max));
    commands.entity(strip).despawn_descendants();
    commands.entity(strip).with_children(|row| {
        for b in binds(ctx)
            .iter()
            .filter(|b| b.strip && label(b, pad).is_some())
            .take(max)
        {
            row.spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(5.0),
                    // Keep each chip on one line; the strip wraps whole chips into rows instead.
                    flex_shrink: 0.0,
                    padding: UiRect {
                        left: Val::Px(3.0),
                        right: Val::Px(7.0),
                        top: Val::Px(3.0),
                        bottom: Val::Px(3.0),
                    },
                    ..default()
                },
                BackgroundColor(Color::srgba(0.04, 0.05, 0.09, 0.60)),
                BorderRadius::all(Val::Px(7.0)),
            ))
            .with_children(|chip| {
                spawn_key_action(chip, label(b, pad).unwrap_or(""), b.action, 10.5, 0.0);
            });
        }
    });
}

// ---------------------------------------------------------------------------------------------
// Pause menu
// ---------------------------------------------------------------------------------------------

const BTN_RESUME: usize = 0;
const BTN_SOUND: usize = 1;
const BTN_RESTART: usize = 2;
const BTN_CLEAR_SAVE: usize = 3;
const BTN_MULTIPLAYER: usize = 4;
const BTN_CLEAR_CACHE: usize = 5;
/// Real seconds the "press again to restart" (or "to clear save" / "to clear cache") confirmation stays armed.
const RESTART_CONFIRM_SECS: f32 = 3.0;
/// Ignore Esc-to-close this long after opening, so the Esc that dropped pointer lock (which can
/// arrive as a key event a frame after the lock change) doesn't close the menu again.
const ESC_CLOSE_GRACE_SECS: f64 = 0.35;

fn pause_buttons(_state: &GameState, _stats: &PlayerStats) -> Vec<usize> {
    vec![BTN_RESUME, BTN_SOUND, BTN_RESTART, BTN_CLEAR_CACHE, BTN_CLEAR_SAVE]
}

#[derive(Resource, Default)]
pub struct PauseMenu {
    selected: usize,
    buttons: Vec<usize>,
    /// > 0 while the Restart button is armed (real seconds left).
    confirm_restart: f32,
    opened_at: f64,
    /// State to restart, set when Restart is confirmed; consumed by [`run_pause_restart`].
    restart_pending: Option<GameState>,
    /// > 0 while the Clear save button is armed (real seconds left).
    confirm_clear: f32,
    /// Set when Clear save is confirmed; consumed by [`run_pause_clear_save`].
    clear_pending: bool,
    /// > 0 while the Clear cache button is armed (real seconds left).
    confirm_clear_cache: f32,
    /// Set once Clear cache is confirmed and a browser reload is in progress.
    cache_reloading: bool,
}

#[derive(Component)]
struct PauseButton(usize);

#[derive(Component)]
struct PauseButtonText(usize);

fn spawn_pause_menu(
    commands: &mut Commands,
    ctx: ControlContext,
    pad: bool,
    w: f32,
    h: f32,
    buttons: &[usize],
) {
    let narrow = w < 520.0;
    let short = h < 560.0;
    let k = if short || narrow { 0.88 } else { 1.0 };
    let row_w = if narrow {
        100.0
    } else if short && w >= 760.0 {
        33.33
    } else {
        50.0
    };
    let mut rows: Vec<(&'static str, &'static str)> = binds(ctx)
        .iter()
        .filter_map(|b| label(b, pad).map(|l| (l, b.action)))
        .collect();
    if !pad {
        rows.push(("M", "Sound on/off"));
    }
    rows.push((if pad { "HOLD START" } else { "HOLD ENTER" }, "Skip cutscene"));

    commands
        .spawn((
            HelpOverlay,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.02, 0.05, 0.62)),
            // Over the city boss bar / minimap, which are spawned later than the HUD.
            GlobalZIndex(1000),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(if short { 5.0 } else { 10.0 }),
                    padding: UiRect::axes(Val::Px(20.0), Val::Px(if short { 12.0 } else { 18.0 })),
                    width: Val::Percent(94.0),
                    max_width: Val::Px(if short { 680.0 } else { 580.0 }),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.06, 0.10, 0.94)),
                BorderColor(Color::srgba(1.0, 0.84, 0.32, 0.45)),
                BorderRadius::all(Val::Px(14.0)),
            ))
            .with_children(|card| {
                card.spawn((
                    Text::new("PAUSED"),
                    TextFont {
                        font_size: 28.0 * k,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.82, 0.22)),
                ));
                card.spawn((
                    Text::new(format!(
                        "Controls: {}{}",
                        ctx.title(),
                        if pad { " (gamepad)" } else { "" }
                    )),
                    TextFont {
                        font_size: 12.5 * k,
                        ..default()
                    },
                    TextColor(Color::srgb(0.85, 0.92, 1.0)),
                ));
                card.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    width: Val::Percent(100.0),
                    row_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|grid| {
                    for (key, action) in rows {
                        grid.spawn(Node {
                            width: Val::Percent(row_w),
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(8.0),
                            ..default()
                        })
                        .with_children(|row| {
                            spawn_key_action(row, key, action, 12.0 * k, 92.0 * k);
                        });
                    }
                });
                card.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Stretch,
                    row_gap: Val::Px(if short { 4.0 } else { 8.0 }),
                    width: Val::Percent(100.0),
                    max_width: Val::Px(300.0),
                    margin: UiRect::top(Val::Px(4.0)),
                    ..default()
                })
                .with_children(|col| {
                    for &btn_id in buttons {
                        col.spawn((
                            PauseButton(btn_id),
                            Button,
                            Node {
                                padding: UiRect::axes(
                                    Val::Px(16.0),
                                    Val::Px(if short { 5.0 } else { 9.0 }),
                                ),
                                justify_content: JustifyContent::Center,
                                border: UiRect::all(Val::Px(2.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.08)),
                            BorderColor(Color::NONE),
                            BorderRadius::all(Val::Px(10.0)),
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                PauseButtonText(btn_id),
                                Text::new(""),
                                TextFont {
                                    font_size: 16.0 * k,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));
                        });
                    }
                });
                card.spawn((
                    Text::new(if pad {
                        "D-PAD select  |  A choose  |  START or B resume"
                    } else {
                        "UP / DOWN select  |  ENTER choose  |  ESC resume"
                    }),
                    TextFont {
                        font_size: 11.5 * k,
                        ..default()
                    },
                    TextColor(Color::srgba(1.0, 1.0, 1.0, 0.78)),
                ));
            });
        });
}

/// Throws away queued touch / gamepad actions so the input that closes the menu (or anything
/// pressed while it was open) doesn't fire as a jump, attack or dash afterwards.
fn drain_queued_actions() {
    let _ = crate::audio::take_touch_jump();
    let _ = crate::audio::take_touch_action_tap();
    let _ = crate::audio::take_touch_attack();
    let _ = crate::audio::take_touch_dash();
}

/// Opens, drives and closes the pause menu (Playing / BossFight / CityPlay, no pending fade).
#[allow(clippy::too_many_arguments)]
fn update_pause_menu(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    touches: Res<Touches>,
    real: Res<Time<Real>>,
    state: Res<State<GameState>>,
    mut stats: ResMut<PlayerStats>,
    mut help: ResMut<HelpPause>,
    mut menu: ResMut<PauseMenu>,
    mut virtual_time: ResMut<Time<Virtual>>,
    mut window_q: Query<&mut Window, With<PrimaryWindow>>,
    btn_q: Query<(&Interaction, &PauseButton), Changed<Interaction>>,
    overlay_q: Query<Entity, With<HelpOverlay>>,
    mut next_state: SceneTransition,
) {
    let now = real.elapsed_secs_f64();
    if !help.open {
        // Always consume the Start / HELP request so one made mid-fade doesn't fire later.
        let requested = keys.just_pressed(KeyCode::Escape)
            || keys.just_pressed(KeyCode::KeyH)
            || crate::audio::take_touch_help();
        // Only open over settled gameplay: not during a fade-in (a pointer-lock loss or the debug
        // flag waits for it to finish instead of being dropped).
        // (`SceneTransition` owns `SceneFade`; `input_unlocked(0.0)` is "fade phase is Idle".)
        if !next_state.input_unlocked(0.0) || stats.casino_open || stats.bartender_open || stats.char_select_open || stats.casino_esc_grace {
            stats.casino_esc_grace = false;
            let _ = crate::audio::take_pointer_lock_lost();
            return;
        }
        let lock_lost = crate::audio::take_pointer_lock_lost();
        let debug_open = crate::audio::take_debug_pause();
        if !(requested || lock_lost || debug_open) {
            return;
        }
        let Some(ctx) = ControlContext::current(state.get(), &stats) else {
            return;
        };
        let buttons = pause_buttons(state.get(), &stats);
        help.open = true;
        virtual_time.pause();
        *menu = PauseMenu {
            opened_at: now,
            buttons: buttons.clone(),
            ..default()
        };
        let _ = crate::audio::take_menu_nav();
        drain_queued_actions();
        // Report "not playing" before freeing the pointer so the JS side doesn't treat our own
        // unlock as a lock loss; this also hides the touch stick and buttons.
        crate::audio::set_touch_playing(false);
        let (w, h) = window_q
            .get_single()
            .map(|w| (w.width(), w.height()))
            .unwrap_or((1280.0, 800.0));
        if let Ok(mut window) = window_q.get_single_mut() {
            window.cursor_options.grab_mode = CursorGrabMode::None;
            window.cursor_options.visible = true;
        }
        spawn_pause_menu(
            &mut commands,
            ctx,
            crate::audio::last_input_gamepad(),
            w,
            h,
            &buttons,
        );
        return;
    }

    let dt = real.delta_secs().min(0.05);
    if menu.confirm_restart > 0.0 {
        menu.confirm_restart = (menu.confirm_restart - dt).max(0.0);
    }
    if menu.confirm_clear > 0.0 {
        menu.confirm_clear = (menu.confirm_clear - dt).max(0.0);
    }
    if menu.confirm_clear_cache > 0.0 {
        menu.confirm_clear_cache = (menu.confirm_clear_cache - dt).max(0.0);
    }
    drain_queued_actions();
    // A lock change while paused is our own unlock (or a stale event); never act on it.
    let _ = crate::audio::take_pointer_lock_lost();
    let nav = crate::audio::take_menu_nav();
    let esc = keys.just_pressed(KeyCode::Escape) && now - menu.opened_at > ESC_CLOSE_GRACE_SECS;
    let close = esc || keys.just_pressed(KeyCode::KeyH) || crate::audio::take_touch_help() || nav == 3;

    let btn_count = menu.buttons.len().max(1);
    let prev_sel = menu.selected;
    if keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::KeyW) || nav == -1 {
        menu.selected = (menu.selected + btn_count - 1) % btn_count;
    }
    if keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::KeyS) || nav == 1 {
        menu.selected = (menu.selected + 1) % btn_count;
    }
    // (button semantic id, activated by a real mouse click)
    let mut activate: Option<(usize, bool)> = None;
    for (interaction, btn) in &btn_q {
        match interaction {
            Interaction::Pressed => {
                if let Some(idx) = menu.buttons.iter().position(|&b| b == btn.0) {
                    menu.selected = idx;
                }
                activate = Some((btn.0, touches.iter().next().is_none()));
            }
            Interaction::Hovered => {
                if let Some(idx) = menu.buttons.iter().position(|&b| b == btn.0) {
                    menu.selected = idx;
                }
            }
            Interaction::None => {}
        }
    }
    let selected_id = menu.buttons.get(menu.selected).copied().unwrap_or(BTN_RESUME);
    if menu.selected != prev_sel && selected_id != BTN_RESTART {
        menu.confirm_restart = 0.0;
    }
    if menu.selected != prev_sel && selected_id != BTN_CLEAR_SAVE {
        menu.confirm_clear = 0.0;
    }
    if menu.selected != prev_sel && selected_id != BTN_CLEAR_CACHE {
        menu.confirm_clear_cache = 0.0;
    }
    if activate.is_none() && (keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space)) {
        // Keyboard (or gamepad A, which arrives as a Space key event).
        activate = Some((selected_id, false));
    }

    let mut resume = close;
    // Re-lock the pointer only on a real mouse click (as the in-game click does) or a keyboard
    // resume that isn't Esc; Esc is not a user activation, so a lock request would be refused.
    let mut relock = !esc && keys.just_pressed(KeyCode::KeyH) && !crate::audio::last_input_gamepad();
    match activate {
        Some((BTN_RESUME, by_mouse)) => {
            resume = true;
            relock = by_mouse || !crate::audio::last_input_gamepad();
        }
        Some((BTN_SOUND, _)) => {
            stats.audio_muted = crate::audio::toggle_mute();
        }
        Some((BTN_MULTIPLAYER, _)) => {
            // Why: lets any Free Mode player opt in/out of distributed P2P multiplayer without
            // leaving their session or touching browser console flags.
            crate::audio::net_set_online(!crate::audio::net_is_online());
        }
        Some((BTN_RESTART, _)) => {
            if menu.confirm_restart > 0.0 {
                menu.confirm_restart = 0.0;
                let current = *state.get();
                menu.restart_pending = Some(current);
                // Same-state request: the fade runs, and `run_pause_restart` (OnTransition)
                // resets the world while the screen is black. The menu stays up until then.
                next_state.set(current);
                return;
            }
            menu.confirm_restart = RESTART_CONFIRM_SECS;
        }
        Some((BTN_CLEAR_CACHE, _)) => {
            if menu.cache_reloading {
                return;
            }
            if menu.confirm_clear_cache > 0.0 {
                menu.confirm_clear_cache = 0.0;
                menu.cache_reloading = true;
                if !stats.free_roam {
                    match state.get() {
                        GameState::CityPlay
                        | GameState::CasinoPlay
                        | GameState::LexieBattle
                        | GameState::LexieVictoryCutscene => {
                            crate::ui::save_game_checkpoint("city", &stats);
                        }
                        GameState::BossFight => {
                            crate::ui::save_game_checkpoint("boss", &stats);
                        }
                        _ => {}
                    }
                }
                crate::audio::clear_cache_and_reload();
                return;
            }
            menu.confirm_clear_cache = RESTART_CONFIRM_SECS;
        }
        Some((BTN_CLEAR_SAVE, _)) => {
            if menu.confirm_clear > 0.0 {
                menu.confirm_clear = 0.0;
                menu.clear_pending = true;
                // Wipe storage now (in case the tab closes mid-fade); `run_pause_clear_save`
                // wipes it again after OnExit and resets the world to a fresh title screen.
                crate::audio::clear_all_progress();
                next_state.set(GameState::Tutorial);
                return;
            }
            menu.confirm_clear = RESTART_CONFIRM_SECS;
        }
        _ => {}
    }
    if !resume {
        return;
    }
    crate::ui::close_help(&mut commands, &mut help, &mut virtual_time, &overlay_q);
    help.resume_grace = true;
    crate::audio::set_touch_playing(true);
    if relock && touches.iter().next().is_none() && !crate::audio::skip_touch_ui() {
        if let Ok(mut window) = window_q.get_single_mut() {
            window.cursor_options.grab_mode = CursorGrabMode::Locked;
            window.cursor_options.visible = false;
        }
    }
}

/// Button labels and selection highlight.
fn refresh_pause_menu(
    menu: Res<PauseMenu>,
    stats: Res<PlayerStats>,
    mut btn_q: Query<(&PauseButton, &mut BackgroundColor, &mut BorderColor)>,
    mut txt_q: Query<(&PauseButtonText, &mut Text)>,
) {
    if btn_q.is_empty() {
        return;
    }
    let armed = menu.confirm_restart > 0.0;
    let armed_clear = menu.confirm_clear > 0.0;
    let armed_cache = menu.confirm_clear_cache > 0.0;
    let reloading_cache = menu.cache_reloading;
    let selected_id = menu.buttons.get(menu.selected).copied().unwrap_or(BTN_RESUME);
    let online = crate::audio::net_is_online();
    for (btn, mut bg, mut border) in &mut btn_q {
        let selected = btn.0 == selected_id;
        let want_bg = match (btn.0, selected) {
            (BTN_RESTART, _) if armed => Color::srgb(0.70, 0.06, 0.06),
            (BTN_CLEAR_SAVE, _) if armed_clear => Color::srgb(0.70, 0.06, 0.06),
            (BTN_CLEAR_CACHE, _) if armed_cache || reloading_cache => Color::srgb(0.10, 0.45, 0.72),
            (_, true) => Color::srgb(0.82, 0.14, 0.16),
            _ => Color::srgba(1.0, 1.0, 1.0, 0.08),
        };
        let want_border = if selected {
            Color::srgba(1.0, 0.84, 0.32, 0.9)
        } else {
            Color::NONE
        };
        if bg.0 != want_bg {
            bg.0 = want_bg;
        }
        if border.0 != want_border {
            border.0 = want_border;
        }
    }
    for (t, mut text) in &mut txt_q {
        let want = match t.0 {
            BTN_RESUME => "RESUME",
            BTN_SOUND if stats.audio_muted => "SOUND: OFF",
            BTN_SOUND => "SOUND: ON",
            BTN_MULTIPLAYER if online => "FREE ROAM: ONLINE (P2P)",
            BTN_MULTIPLAYER => "FREE ROAM: SOLO",
            BTN_CLEAR_CACHE if reloading_cache => "CLEARING CACHE...",
            BTN_CLEAR_CACHE if armed_cache => "PRESS AGAIN TO CLEAR CACHE",
            BTN_CLEAR_CACHE => "CLEAR CACHE (KEEPS SAVE)",
            BTN_CLEAR_SAVE if armed_clear => "PRESS AGAIN TO CLEAR SAVE",
            BTN_CLEAR_SAVE => "CLEAR SAVE",
            _ if armed => "PRESS AGAIN TO RESTART LEVEL",
            _ => "RESTART LEVEL",
        };
        if text.0 != want {
            text.0 = want.to_string();
        }
    }
}

/// OnTransition(S -> S) for the gameplay states: a confirmed pause-menu Restart. Identity
/// transitions skip OnExit/OnEnter, so run them here around the same world reset GameOver uses
/// (`ui::reset_world_for_restart`), all while the fade overlay is black. Level 1 and the Mom fight
/// restart on a fresh kitchen (the Mom fight then re-enters BossFight, like the checkpoint
/// continue); the city restarts like a death retry.
fn run_pause_restart(world: &mut World) {
    let Some(state) = world.resource_mut::<PauseMenu>().restart_pending.take() else {
        return;
    };
    let _ = world.try_run_schedule(OnExit(state));
    if state != GameState::CasinoPlay {
        let _ = world.run_system_once_with((state == GameState::CityPlay, false), restart_world_for_pause);
    }
    let _ = world.try_run_schedule(OnEnter(state));
}

/// OnTransition(gameplay state -> Tutorial): a confirmed pause-menu Clear save. Runs after the
/// gameplay state's OnExit and before OnEnter(Tutorial) (so the title card sees no save and shows
/// no Continue), while the fade overlay is black. Both localStorage keys are wiped again here so
/// nothing written during the fade survives, then the world gets the same Level 1 reset as
/// "Play again", but without the sticky `game_beaten` flag: only the mute setting survives.
fn run_pause_clear_save(world: &mut World) {
    if !std::mem::take(&mut world.resource_mut::<PauseMenu>().clear_pending) {
        return;
    }
    crate::audio::clear_all_progress();
    let _ = world.run_system_once_with((false, true), restart_world_for_pause);
}

/// `(retry_city, wipe_progress)`: `wipe_progress` drops the sticky `game_beaten` flag too.
#[allow(clippy::too_many_arguments)]
fn restart_world_for_pause(
    In((retry_city, wipe_progress)): In<(bool, bool)>,
    commands: Commands,
    asset_server: Res<AssetServer>,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<StandardMaterial>>,
    mut stats: ResMut<PlayerStats>,
    orbit: ResMut<CameraOrbit>,
    level_q: Query<Entity, With<LevelEntity>>,
    end_q: Query<Entity, With<EndOverlay>>,
) {
    if wipe_progress {
        // `reset_world_for_restart` keeps `game_beaten` and resets everything else (license
        // included, as this is not a city retry).
        stats.game_beaten = false;
    }
    crate::ui::reset_world_for_restart(
        retry_city, commands, asset_server, meshes, materials, stats, orbit, level_q, end_q,
    );
}

pub struct ControlsPlugin;

impl Plugin for ControlsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PauseMenu>()
            .add_systems(Startup, spawn_control_strip.after(crate::ui::spawn_hud))
            .add_systems(
                Update,
                update_pause_menu
                    .run_if(
                        in_state(GameState::Playing)
                            .or(in_state(GameState::BossFight))
                            .or(in_state(GameState::CityPlay))
                            .or(in_state(GameState::CasinoPlay)),
                    )
                    .run_if(sim_running),
            )
            .add_systems(Update, (update_control_strip, refresh_pause_menu));
        for s in [
            GameState::Playing,
            GameState::BossFight,
            GameState::CityPlay,
            GameState::CasinoPlay,
        ] {
            app.add_systems(
                OnTransition {
                    exited: s,
                    entered: s,
                },
                run_pause_restart,
            );
            app.add_systems(
                OnTransition {
                    exited: s,
                    entered: GameState::Tutorial,
                },
                run_pause_clear_save,
            );
        }
    }
}
