//! Browser JS bridge (`window.tendyAudio`, `window.tendyInput`, `window.tendySave`, `window.tendyTelemetry`).
//!
//! Audio, Web Speech TTS, mobile touch, W3C Gamepad polling, and `localStorage` checkpoints live
//! in `rust/index.html` rather than Bevy plugins so WebAudio unlocks reliably on the first
//! click/tap/button press and HTML overlays (Uzumaki spirals, touch controls) stay crisp at native
//! browser DPI. Non-`wasm32` targets use no-op stubs below so native `cargo check` compiles cleanly.

#[cfg(target_arch = "wasm32")]
mod imp {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = ["window", "tendyAudio"], js_name = playBgm)]
        fn js_play_bgm();

        #[wasm_bindgen(js_namespace = ["window", "tendyAudio"], js_name = playStartMenuMusic)]
        fn js_play_start_menu_music();

        #[wasm_bindgen(js_namespace = ["window", "tendyAudio"], js_name = playCutsceneMusic)]
        fn js_play_cutscene_music();

        #[wasm_bindgen(js_namespace = ["window", "tendyAudio"], js_name = playHeadphoneMusic)]
        fn js_play_headphone_music(level: u8);

        #[wasm_bindgen(js_namespace = ["window", "tendyAudio"], js_name = playBossMusic)]
        fn js_play_boss_music();

        #[wasm_bindgen(js_namespace = ["window", "tendyAudio"], js_name = playVikingChantMusic)]
        fn js_play_viking_chant_music();

        #[wasm_bindgen(js_namespace = ["window", "tendyAudio"], js_name = playHackingMusic)]
        fn js_play_hacking_music();

        #[wasm_bindgen(js_namespace = ["window", "tendyAudio"], js_name = playScooterMusic)]
        fn js_play_scooter_music();

        #[wasm_bindgen(js_namespace = ["window", "tendyAudio"], js_name = playCasinoJazzMusic)]
        fn js_play_casino_jazz_music();

        #[wasm_bindgen(js_namespace = ["window", "tendyAudio"], js_name = playWonkaTripMusic)]
        fn js_play_wonka_trip_music();

        #[wasm_bindgen(js_namespace = ["window", "tendyAudio"], js_name = playClubDiscoMusic)]
        fn js_play_club_disco_music();

        #[wasm_bindgen(js_namespace = ["window", "tendyAudio"], js_name = playLexieRpgBossMusic)]
        fn js_play_lexie_rpg_boss_music();

        #[wasm_bindgen(js_namespace = ["window", "tendyAudio"], js_name = speakVoice)]
        fn js_speak_voice(text: &str, role: &str);

        #[wasm_bindgen(js_namespace = ["window", "tendyAudio"], js_name = isVoiceSpeaking)]
        fn js_is_voice_speaking() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyAudio"], js_name = stopVoice)]
        fn js_stop_voice();

        #[wasm_bindgen(js_namespace = ["window", "tendyAudio"], js_name = playSfx)]
        fn js_play_sfx(name: &str);

        #[wasm_bindgen(js_namespace = ["window", "tendyAudio"], js_name = toggleMute)]
        fn js_toggle_mute() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyAudio"], js_name = isMuted)]
        fn js_is_muted() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyAudio"], js_name = setVolume)]
        fn js_set_volume(v: f32) -> f32;

        #[wasm_bindgen(js_namespace = ["window", "tendyAudio"], js_name = getVolume)]
        fn js_get_volume() -> f32;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = moveX)]
        fn js_move_x() -> f32;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = moveY)]
        fn js_move_y() -> f32;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = takeLookDX)]
        fn js_take_look_dx() -> f32;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = takeLookDY)]
        fn js_take_look_dy() -> f32;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = takeZoomDelta)]
        fn js_take_zoom_delta() -> f32;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = takeJump)]
        fn js_take_jump() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = takeDash)]
        fn js_take_dash() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = takeAttack)]
        fn js_take_attack() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = blockHeld)]
        fn js_block_held() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = attackHeld)]
        fn js_attack_held() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = takeGrenade)]
        fn js_take_grenade() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = takeWeaponCycle)]
        fn js_take_weapon_cycle() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = setCityWeapons)]
        fn js_set_city_weapons(active: bool, weapon_slot: u8, grenades: u32, grenades_unlocked: bool);

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = setCraftMode)]
        fn js_set_craft_mode(active: bool);

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = setCraftShapeLabel)]
        fn js_set_craft_shape_label(label: &str);

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = takeCraftBench)]
        fn js_take_craft_bench() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = takeCraftShape)]
        fn js_take_craft_shape() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = takeCraftView)]
        fn js_take_craft_view() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyTutorialVideo"], js_name = open)]
        fn js_open_derek_tutorial_video();

        #[wasm_bindgen(js_namespace = ["window", "tendyTutorialVideo"], js_name = close)]
        fn js_close_derek_tutorial_video();

        #[wasm_bindgen(js_namespace = ["window", "tendyTutorialVideo"], js_name = isOpen)]
        fn js_is_derek_tutorial_open() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyTutorialVideo"], js_name = takeJustClosed)]
        fn js_take_derek_tutorial_closed() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = combatDemo)]
        fn js_combat_demo() -> String;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = cityStart)]
        fn js_city_start() -> String;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = takeActionTap)]
        fn js_take_action_tap() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = takeHelp)]
        fn js_take_help() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = setPlaying)]
        fn js_set_playing(active: bool);

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = setBossMode)]
        fn js_set_boss_mode(active: bool);

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = setUzumakiMode)]
        fn js_set_uzumaki_mode(level: u8);

        // Why: toggles the crisp browser-DPI *They Live* CSS contrast filter and subliminal placard
        // overlay during Chillary's 1-second reptile flash (`index.html`). Gameplay impact: visual only.
        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = setTheyLiveMode)]
        fn js_set_they_live_mode(active: bool);

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = setTutorialVisible)]
        fn js_set_tutorial_visible(active: bool);

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = initMode)]
        fn js_init_mode() -> u32;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = debugKeys)]
        fn js_debug_keys() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = takeCutsceneStartTime)]
        fn js_take_cutscene_start_time() -> f32;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = skipHeld)]
        fn js_skip_held() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = skipTouchUi)]
        fn js_skip_touch_ui() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = setCutsceneSkip)]
        fn js_set_cutscene_skip(active: bool, progress: f32);

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = autoSkip)]
        fn js_auto_skip() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = takeMenuNav)]
        fn js_take_menu_nav() -> i32;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = usingGamepad)]
        fn js_using_gamepad() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = takePointerLockLost)]
        fn js_take_pointer_lock_lost() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = takeDebugPause)]
        fn js_take_debug_pause() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = wheelDemo)]
        fn js_wheel_demo() -> String;

        #[wasm_bindgen(js_namespace = ["window", "tendyTelemetry"], js_name = setWheel)]
        fn js_set_wheel_telemetry(line: &str);

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = atBeer)]
        fn js_at_beer() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = fastClub)]
        fn js_fast_club() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = fentanylMode)]
        fn js_fentanyl_mode() -> i32;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = overdoseSecs)]
        fn js_overdose_secs() -> f32;

        #[wasm_bindgen(js_namespace = Math, js_name = random)]
        fn js_math_random() -> f64;

        #[wasm_bindgen(js_namespace = ["window", "tendyTelemetry"], js_name = setClub)]
        fn js_set_club_telemetry(line: &str);

        #[wasm_bindgen(js_namespace = ["window", "tendyTelemetry"], js_name = setLexie)]
        fn js_set_lexie_telemetry(line: &str);

        #[wasm_bindgen(js_namespace = ["window", "tendySave"], js_name = saveCheckpoint)]
        fn js_save_checkpoint(json: &str);

        #[wasm_bindgen(js_namespace = ["window", "tendySave"], js_name = loadCheckpoint)]
        fn js_load_checkpoint() -> String;

        #[wasm_bindgen(js_namespace = ["window", "tendySave"], js_name = clearCheckpoint)]
        fn js_clear_checkpoint();

        #[wasm_bindgen(js_namespace = ["window", "tendySave"], js_name = markMomDefeated)]
        fn js_mark_mom_defeated();

        #[wasm_bindgen(js_namespace = ["window", "tendySave"], js_name = hasDefeatedMom)]
        fn js_has_defeated_mom() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendySave"], js_name = clearAllProgress)]
        fn js_clear_all_progress();

        #[wasm_bindgen(js_namespace = ["window", "tendySave"], js_name = clearCacheAndReload)]
        fn js_clear_cache_and_reload();

        #[wasm_bindgen(js_namespace = ["window", "tendySave"], js_name = saveTendyWendies)]
        fn js_save_tendy_wendies(amount: u32);

        #[wasm_bindgen(js_namespace = ["window", "tendySave"], js_name = loadTendyWendies)]
        fn js_load_tendy_wendies() -> u32;

        #[wasm_bindgen(js_namespace = ["window", "tendyBoot"], js_name = ready)]
        fn js_boot_ready();

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = parkDemo)]
        fn js_park_demo() -> String;

        #[wasm_bindgen(js_namespace = ["window", "tendyInput"], js_name = jumpHeld)]
        fn js_jump_held() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyTelemetry"], js_name = parkLog)]
        fn js_park_log(line: &str);

        #[wasm_bindgen(js_namespace = ["window", "tendyTelemetry"], js_name = update)]
        fn js_update_telemetry(
            state: &str,
            hearts: i32,
            tendies: u32,
            stamina: f32,
            boss_hp: f32,
            credits: u32,
            in_city: bool,
        );
    }

    pub fn play_bgm() {
        js_play_bgm();
    }

    pub fn play_start_menu_music() {
        js_play_start_menu_music();
    }

    pub fn play_cutscene_music() {
        js_play_cutscene_music();
    }

    pub fn play_headphone_music(level: u8) {
        js_play_headphone_music(level);
    }

    pub fn play_boss_music() {
        js_play_boss_music();
    }

    pub fn play_viking_chant_music() {
        js_play_viking_chant_music();
    }

    pub fn play_hacking_music() {
        js_play_hacking_music();
    }

    pub fn play_scooter_music() {
        js_play_scooter_music();
    }

    pub fn play_casino_jazz_music() {
        js_play_casino_jazz_music();
    }

    pub fn play_wonka_trip_music() {
        js_play_wonka_trip_music();
    }

    pub fn play_club_disco_music() {
        js_play_club_disco_music();
    }

    pub fn play_lexie_rpg_music() {
        js_play_lexie_rpg_boss_music();
    }

    pub fn speak_voice(text: &str, role: &str) {
        js_speak_voice(text, role);
    }

    pub fn is_voice_speaking() -> bool {
        js_is_voice_speaking()
    }

    /// Cancels the current TTS line so `is_voice_speaking` reads false right away.
    pub fn stop_voice() {
        js_stop_voice();
    }

    /// Gamepad Start or the touch SKIP button is held (keyboard Enter is read in Bevy).
    pub fn skip_input_held() -> bool {
        js_skip_held()
    }

    /// Touch layout is active, so the HTML SKIP button replaces the keyboard prompt.
    pub fn skip_touch_ui() -> bool {
        js_skip_touch_ui()
    }

    /// Shows/hides the touch SKIP button and sets its hold fill (0..1); also tells the gamepad
    /// poller that Start means skip, not help, while a cutscene runs.
    pub fn set_cutscene_skip(active: bool, progress: f32) {
        js_set_cutscene_skip(active, progress);
    }

    /// `?debug=1&autoskip=1`: skip each cutscene automatically (headless tests).
    pub fn debug_autoskip_enabled() -> bool {
        js_auto_skip()
    }

    /// Pause menu gamepad navigation since the last call: -1 up, 1 down, 3 back (B), 0 none.
    pub fn take_menu_nav() -> i32 {
        js_take_menu_nav()
    }

    /// True when the most recent input came from a gamepad (controls strip shows pad labels).
    pub fn last_input_gamepad() -> bool {
        js_using_gamepad()
    }

    /// The browser dropped pointer lock (Esc, focus loss) during gameplay since the last call.
    pub fn take_pointer_lock_lost() -> bool {
        js_take_pointer_lock_lost()
    }

    /// `?debug=1&pause_menu=1`: open the pause menu once on load (headless screenshots).
    pub fn take_debug_pause() -> bool {
        js_take_debug_pause()
    }

    /// `?debug=1&at_beer=1`: start Stan inside Pub 217 facing the beer tower (headless shots).
    pub fn debug_at_beer() -> bool {
        js_at_beer()
    }

    /// `?debug=1&wheel_demo=...` (ride, fast, exit, spin, titan): Ferris wheel debug driver.
    pub fn debug_wheel_demo() -> String {
        js_wheel_demo()
    }

    /// Debug ride state for headless checks (`window.tendyTelemetry.wheel`).
    pub fn set_wheel_telemetry(line: &str) {
        js_set_wheel_telemetry(line);
    }

    pub fn play_sfx(name: &str) {
        js_play_sfx(name);
    }

    pub fn toggle_mute() -> bool {
        js_toggle_mute()
    }

    pub fn is_muted() -> bool {
        js_is_muted()
    }

    /// Sets the master volume (0..1, remembered by the page) and returns the applied value.
    pub fn set_volume(v: f32) -> f32 {
        js_set_volume(v)
    }

    pub fn volume() -> f32 {
        js_get_volume()
    }

    pub fn touch_move_x() -> f32 {
        js_move_x()
    }

    pub fn touch_move_y() -> f32 {
        js_move_y()
    }

    pub fn take_touch_look_dx() -> f32 {
        js_take_look_dx()
    }

    pub fn take_touch_look_dy() -> f32 {
        js_take_look_dy()
    }

    pub fn take_touch_zoom_delta() -> f32 {
        js_take_zoom_delta()
    }

    pub fn take_touch_jump() -> bool {
        js_take_jump()
    }

    pub fn take_touch_dash() -> bool {
        js_take_dash()
    }

    pub fn take_touch_attack() -> bool {
        js_take_attack()
    }

    pub fn take_touch_action_tap() -> bool {
        js_take_action_tap()
    }

    /// Touch BLOCK button is being held (boss mode saber guard).
    pub fn touch_block_held() -> bool {
        js_block_held()
    }

    /// Touch SABER button or a gamepad attack button (RB / RT) is held down (heavy charge).
    pub fn touch_attack_held() -> bool {
        js_attack_held()
    }

    pub fn take_touch_grenade() -> bool {
        js_take_grenade()
    }

    pub fn take_touch_weapon_cycle() -> bool {
        js_take_weapon_cycle()
    }

    pub fn set_touch_city_weapons(active: bool, weapon_slot: u8, grenades: u32, grenades_unlocked: bool) {
        js_set_city_weapons(active, weapon_slot, grenades, grenades_unlocked);
    }

    pub fn set_touch_craft_mode(active: bool) {
        js_set_craft_mode(active);
    }

    pub fn set_touch_craft_shape(label: &str) {
        js_set_craft_shape_label(label);
    }

    pub fn take_touch_craft_bench() -> bool {
        js_take_craft_bench()
    }

    pub fn take_touch_craft_shape() -> bool {
        js_take_craft_shape()
    }

    pub fn take_touch_craft_view() -> bool {
        js_take_craft_view()
    }

    pub fn open_derek_tutorial_video() {
        js_open_derek_tutorial_video();
    }

    pub fn close_derek_tutorial_video() {
        js_close_derek_tutorial_video();
    }

    pub fn is_derek_tutorial_open() -> bool {
        js_is_derek_tutorial_open()
    }

    pub fn take_derek_tutorial_closed() -> bool {
        js_take_derek_tutorial_closed()
    }

    /// `combat_demo=` URL value (empty unless `debug=1`), for the debug combat motion demo.
    pub fn debug_combat_demo() -> String {
        js_combat_demo()
    }

    /// `city_start=` URL value (empty unless `debug=1`): where a fresh city entry drops Stan.
    pub fn debug_city_start() -> String {
        js_city_start()
    }

    /// `park_demo=` URL value (empty unless `debug=1`): scripted skate park run (skatepark.rs).
    pub fn debug_park_demo() -> String {
        js_park_demo()
    }

    /// Touch JUMP button is held down (backflips on the skate park need a hold).
    pub fn touch_jump_held() -> bool {
        js_jump_held()
    }

    /// Appends a line to `window.tendyParkLog` (debug telemetry for the skate park demos).
    pub fn park_log(line: &str) {
        js_park_log(line);
    }

    pub fn take_touch_help() -> bool {
        js_take_help()
    }

    pub fn set_touch_playing(active: bool) {
        js_set_playing(active);
    }

    pub fn set_touch_boss_mode(active: bool) {
        js_set_boss_mode(active);
    }

    pub fn set_uzumaki_mode(level: u8) {
        js_set_uzumaki_mode(level);
    }

    /// Toggles crisp HTML/CSS *They Live* contrast grading and subliminal placards (`index.html`).
    /// Gameplay impact: purely visual overlay (`pointer-events: none`), zero effect on controls.
    pub fn set_they_live_mode(active: bool) {
        js_set_they_live_mode(active);
    }

    pub fn set_tutorial_visible(active: bool) {
        js_set_tutorial_visible(active);
    }

    pub fn init_test_mode() -> u32 {
        js_init_mode()
    }

    pub fn debug_keys_enabled() -> bool {
        js_debug_keys()
    }

    pub fn take_cutscene_start_time() -> f32 {
        js_take_cutscene_start_time()
    }

    pub fn save_checkpoint(data: &str) {
        js_save_checkpoint(data);
    }

    pub fn load_checkpoint() -> String {
        js_load_checkpoint()
    }

    #[allow(dead_code)]
    pub fn clear_checkpoint() {
        js_clear_checkpoint();
    }

    pub fn mark_mom_defeated() {
        js_mark_mom_defeated();
    }

    #[allow(dead_code)]
    pub fn has_defeated_mom() -> bool {
        js_has_defeated_mom()
    }

    /// Pause-menu Clear save: drops both localStorage keys (checkpoint + Mom-defeated).
    pub fn clear_all_progress() {
        js_clear_all_progress();
    }

    /// Pause-menu Clear cache: unregisters SW/CacheStorage and reloads without touching localStorage saves.
    pub fn clear_cache_and_reload() {
        js_clear_cache_and_reload();
    }

    /// Persists the player's Tendy Wendies currency wallet for casino & future features.
    pub fn save_tendy_wendies(amount: u32) {
        js_save_tendy_wendies(amount);
    }

    /// Loads the persisted Tendy Wendies balance (defaults to 100 TW for new players).
    pub fn load_tendy_wendies() -> u32 {
        js_load_tendy_wendies()
    }

    /// Startup scenes are loaded: lets `index.html` fade its loading screen out.
    pub fn notify_boot_ready() {
        js_boot_ready();
    }

    pub fn update_telemetry(
        state: &str,
        hearts: i32,
        tendies: u32,
        stamina: f32,
        boss_hp: f32,
        credits: u32,
        in_city: bool,
    ) {
        js_update_telemetry(state, hearts, tendies, stamina, boss_hp, credits, in_city);
    }

    // Distributed free-roam multiplayer (`window.tendyNet`, net.rs, docs/MULTIPLAYER.md).
    // Why: keeps networking lifecycle & Online/Solo toggles event-driven in JS outside Rust's frame loop
    // while giving Free Mode full control over activating discovery and choosing Solo vs Online.
    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = ["window", "tendyNet"], js_name = enabled)]
        fn js_net_enabled() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyNet"], js_name = setFreeRoam)]
        fn js_net_set_free_roam(active: bool);

        #[wasm_bindgen(js_namespace = ["window", "tendyNet"], js_name = setOnline)]
        fn js_net_set_online(online: bool);

        #[wasm_bindgen(js_namespace = ["window", "tendyNet"], js_name = isOnline)]
        fn js_net_is_online() -> bool;

        #[wasm_bindgen(js_namespace = ["window", "tendyNet"], js_name = setScene)]
        fn js_net_set_scene(scene_id: u32, level1_ready: bool);

        #[wasm_bindgen(js_namespace = ["window", "tendyNet"], js_name = openCharSelect)]
        fn js_net_open_char_select(notice: &str);

        #[wasm_bindgen(js_namespace = ["window", "tendyNet"], js_name = lockChar)]
        fn js_net_lock_char(char_id: u32);

        #[wasm_bindgen(js_namespace = ["window", "tendyNet"], js_name = squadInfo)]
        fn js_net_squad_info() -> Vec<u32>;

        #[wasm_bindgen(js_namespace = ["window", "tendyNet"], js_name = publish)]
        fn js_net_publish(state: &[f32]);

        #[wasm_bindgen(js_namespace = ["window", "tendyNet"], js_name = publishMom)]
        fn js_net_publish_mom(state: &[f32], reliable: bool);

        #[wasm_bindgen(js_namespace = ["window", "tendyNet"], js_name = event)]
        fn js_net_event(code: u32, value: u32);

        #[wasm_bindgen(js_namespace = ["window", "tendyNet"], js_name = drain)]
        fn js_net_drain() -> Vec<f64>;

        #[wasm_bindgen(js_namespace = ["window", "tendyNet"], js_name = telemetry)]
        fn js_net_telemetry(json: &str);
    }

    /// True whenever the browser multiplayer bridge is loaded (stays inert outside Free Mode).
    pub fn net_enabled() -> bool {
        js_net_enabled()
    }

    /// Notifies JS when entering/leaving Cedar Rapids Free Mode so P2P discovery only runs there.
    pub fn net_set_free_roam(active: bool) {
        js_net_set_free_roam(active);
    }

    /// Toggles distributed Free Mode P2P networking vs Solo mode (persisted in localStorage).
    pub fn net_set_online(online: bool) {
        js_net_set_online(online);
    }

    /// Whether Online P2P multiplayer is preferred for Free Mode.
    pub fn net_is_online() -> bool {
        js_net_is_online()
    }

    pub fn net_set_scene(scene_id: u32, level1_ready: bool) {
        js_net_set_scene(scene_id, level1_ready);
    }

    pub fn net_open_char_select(notice: &str) {
        js_net_open_char_select(notice);
    }

    pub fn net_lock_char(char_id: u32) {
        js_net_lock_char(char_id);
    }

    pub fn net_squad_info() -> [u32; 8] {
        let v = js_net_squad_info();
        [
            v.first().copied().unwrap_or(1),
            v.get(1).copied().unwrap_or(0),
            v.get(2).copied().unwrap_or(1),
            v.get(3).copied().unwrap_or(0),
            v.get(4).copied().unwrap_or(0),
            v.get(5).copied().unwrap_or(0),
            v.get(6).copied().unwrap_or(0),
            v.get(7).copied().unwrap_or(1),
        ]
    }

    /// Local hero state for the wire (see net.rs `publish_local_state` for the field order).
    pub fn net_publish(state: &[f32]) {
        js_net_publish(state);
    }

    /// Authoritative Mom boss snapshot for `BossFight` peers.
    pub fn net_publish_mom(state: &[f32], reliable: bool) {
        js_net_publish_mom(state, reliable);
    }

    /// One-off event for peers (1 trick landed with points, 2 drank a beer).
    pub fn net_event(code: u32, value: u32) {
        js_net_event(code, value);
    }

    /// `[now_ms, peers, n, local_char, flags_mask, avg_rtt_ms, shard_idx, n * 20]` received since the last call.
    pub fn net_drain() -> Vec<f64> {
        js_net_drain()
    }

    /// Remote avatar positions as drawn, for headless checks (`window.tendyTelemetry.mp`).
    pub fn net_telemetry(json: &str) {
        js_net_telemetry(json);
    }

    /// `fast_club=1` URL flag (debug=1): shortens TJ chair approach interval in Velvet Disco Club.
    pub fn debug_fast_club() -> bool {
        js_fast_club()
    }

    /// Updates `window.tendyTelemetry.club` for headless Velvet Disco Club checks.
    pub fn set_club_telemetry(line: &str) {
        js_set_club_telemetry(line);
    }

    /// Updates `window.tendyTelemetry.lexie` for headless turn-based Lexie battle checks.
    pub fn set_lexie_telemetry(line: &str) {
        js_set_lexie_telemetry(line);
    }

    /// Debug override for 50% fentanyl roll (`1` = force overdose, `0` = force safe, `-1` = random).
    pub fn debug_fentanyl_mode() -> i32 {
        js_fentanyl_mode()
    }

    /// Debug override for fentanyl Narcan rescue countdown (defaults to `30.0` seconds).
    pub fn debug_overdose_secs() -> f32 {
        let v = js_overdose_secs();
        if v > 0.0 { v } else { 30.0 }
    }

    /// Uniform random float in `[0.0, 1.0)`.
    pub fn rand_unit() -> f32 {
        (js_math_random() as f32).clamp(0.0, 0.99999)
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    pub fn play_bgm() {}

    pub fn play_start_menu_music() {}

    pub fn play_cutscene_music() {}

    pub fn play_headphone_music(_level: u8) {}

    pub fn play_boss_music() {}

    pub fn play_viking_chant_music() {}

    pub fn play_hacking_music() {}

    pub fn play_scooter_music() {}

    pub fn play_casino_jazz_music() {}

    pub fn play_wonka_trip_music() {}

    pub fn play_club_disco_music() {}

    pub fn play_lexie_rpg_music() {}

    pub fn speak_voice(_text: &str, _role: &str) {}

    pub fn is_voice_speaking() -> bool {
        false
    }

    pub fn stop_voice() {}

    pub fn skip_input_held() -> bool {
        false
    }

    pub fn skip_touch_ui() -> bool {
        false
    }

    pub fn set_cutscene_skip(_active: bool, _progress: f32) {}

    pub fn debug_autoskip_enabled() -> bool {
        false
    }

    pub fn take_menu_nav() -> i32 {
        0
    }

    pub fn last_input_gamepad() -> bool {
        false
    }

    pub fn take_pointer_lock_lost() -> bool {
        false
    }

    pub fn take_debug_pause() -> bool {
        false
    }

    pub fn debug_at_beer() -> bool {
        false
    }

    pub fn debug_fast_club() -> bool {
        false
    }

    pub fn debug_fentanyl_mode() -> i32 {
        -1
    }

    pub fn debug_overdose_secs() -> f32 {
        30.0
    }

    pub fn rand_unit() -> f32 {
        0.25
    }

    pub fn set_club_telemetry(_line: &str) {}

    pub fn debug_wheel_demo() -> String {
        String::new()
    }

    pub fn set_wheel_telemetry(_line: &str) {}

    pub fn set_lexie_telemetry(_line: &str) {}

    pub fn play_sfx(_name: &str) {}

    pub fn toggle_mute() -> bool {
        false
    }

    pub fn is_muted() -> bool {
        false
    }

    pub fn set_volume(v: f32) -> f32 {
        v.clamp(0.0, 1.0)
    }

    pub fn volume() -> f32 {
        1.0
    }

    pub fn touch_move_x() -> f32 {
        0.0
    }

    pub fn touch_move_y() -> f32 {
        0.0
    }

    pub fn take_touch_look_dx() -> f32 {
        0.0
    }

    pub fn take_touch_look_dy() -> f32 {
        0.0
    }

    pub fn take_touch_zoom_delta() -> f32 {
        0.0
    }

    pub fn take_touch_jump() -> bool {
        false
    }

    pub fn take_touch_dash() -> bool {
        false
    }

    pub fn take_touch_attack() -> bool {
        false
    }

    pub fn take_touch_action_tap() -> bool {
        false
    }

    pub fn touch_block_held() -> bool {
        false
    }

    pub fn touch_attack_held() -> bool {
        false
    }

    pub fn take_touch_grenade() -> bool {
        false
    }

    pub fn take_touch_weapon_cycle() -> bool {
        false
    }

    pub fn set_touch_city_weapons(_active: bool, _weapon_slot: u8, _grenades: u32, _grenades_unlocked: bool) {}

    pub fn set_touch_craft_mode(_active: bool) {}

    pub fn set_touch_craft_shape(_label: &str) {}

    pub fn take_touch_craft_bench() -> bool {
        false
    }

    pub fn take_touch_craft_shape() -> bool {
        false
    }

    pub fn take_touch_craft_view() -> bool {
        false
    }

    pub fn open_derek_tutorial_video() {}

    pub fn close_derek_tutorial_video() {}

    pub fn is_derek_tutorial_open() -> bool {
        false
    }

    pub fn take_derek_tutorial_closed() -> bool {
        false
    }

    pub fn debug_combat_demo() -> String {
        String::new()
    }

    pub fn debug_city_start() -> String {
        String::new()
    }

    pub fn debug_park_demo() -> String {
        String::new()
    }

    pub fn touch_jump_held() -> bool {
        false
    }

    pub fn park_log(_line: &str) {}

    pub fn take_touch_help() -> bool {
        false
    }

    pub fn set_touch_playing(_active: bool) {}

    pub fn set_touch_boss_mode(_active: bool) {}

    pub fn set_uzumaki_mode(_level: u8) {}

    /// Native check stub mirroring the wasm32 *They Live* DOM overlay toggle.
    pub fn set_they_live_mode(_active: bool) {}

    pub fn set_tutorial_visible(_active: bool) {}

    pub fn init_test_mode() -> u32 {
        0
    }

    pub fn debug_keys_enabled() -> bool {
        false
    }

    pub fn take_cutscene_start_time() -> f32 {
        -1.0
    }

    pub fn save_checkpoint(_data: &str) {}

    pub fn load_checkpoint() -> String {
        String::new()
    }

    #[allow(dead_code)]
    pub fn clear_checkpoint() {}

    pub fn mark_mom_defeated() {}

    #[allow(dead_code)]
    pub fn has_defeated_mom() -> bool {
        false
    }

    pub fn clear_all_progress() {}

    pub fn clear_cache_and_reload() {}

    pub fn save_tendy_wendies(_amount: u32) {}

    pub fn load_tendy_wendies() -> u32 {
        100
    }

    pub fn notify_boot_ready() {}

    pub fn update_telemetry(
        _state: &str,
        _hearts: i32,
        _tendies: u32,
        _stamina: f32,
        _boss_hp: f32,
        _credits: u32,
        _in_city: bool,
    ) {
    }

    pub fn net_enabled() -> bool {
        false
    }

    pub fn net_set_free_roam(_active: bool) {}

    pub fn net_set_online(_online: bool) {}

    pub fn net_is_online() -> bool {
        true
    }

    pub fn net_set_scene(_scene_id: u32, _level1_ready: bool) {}

    pub fn net_open_char_select(_notice: &str) {}

    pub fn net_lock_char(_char_id: u32) {}

    pub fn net_squad_info() -> [u32; 8] {
        [1, 0, 1, 0, 0, 0, 0, 1]
    }

    pub fn net_publish(_state: &[f32]) {}

    pub fn net_publish_mom(_state: &[f32], _reliable: bool) {}

    pub fn net_event(_code: u32, _value: u32) {}

    pub fn net_drain() -> Vec<f64> {
        Vec::new()
    }

    pub fn net_telemetry(_json: &str) {}
}

pub use imp::{
    debug_autoskip_enabled, debug_city_start, debug_combat_demo, debug_keys_enabled, init_test_mode, is_muted, is_voice_speaking, load_checkpoint,
    mark_mom_defeated, notify_boot_ready, play_bgm, play_boss_music, play_casino_jazz_music, play_club_disco_music, play_cutscene_music, play_hacking_music,
    play_headphone_music, play_lexie_rpg_music, play_scooter_music, play_sfx, play_start_menu_music,
    play_viking_chant_music, play_wonka_trip_music, save_checkpoint,
    set_cutscene_skip, set_touch_boss_mode, set_touch_playing, set_tutorial_visible, skip_input_held,
    skip_touch_ui, stop_voice, set_they_live_mode, set_uzumaki_mode, speak_voice, take_cutscene_start_time,
    take_touch_action_tap, take_touch_attack, take_touch_dash, take_touch_grenade, take_touch_help, take_touch_jump,
    take_touch_look_dx, take_touch_look_dy, take_touch_weapon_cycle, take_touch_zoom_delta, set_touch_city_weapons,
    set_touch_craft_mode, set_touch_craft_shape, take_touch_craft_bench, take_touch_craft_shape,
    take_touch_craft_view, open_derek_tutorial_video, close_derek_tutorial_video,
    is_derek_tutorial_open, take_derek_tutorial_closed, toggle_mute, set_volume, volume,
    touch_attack_held, touch_block_held, touch_move_x,
    touch_move_y, update_telemetry,
};
#[allow(unused_imports)]
pub use imp::has_defeated_mom;
// Pause menu / controls strip bridge (controls.rs).
pub use imp::{
    clear_all_progress, clear_cache_and_reload, last_input_gamepad, take_debug_pause, take_menu_nav,
    take_pointer_lock_lost,
};
// Tendy Wendies persistent currency wallet (state.rs, casino.rs, ui.rs).
pub use imp::{load_tendy_wendies, save_tendy_wendies};
// Debug start at the Pub 217 beer tower (beer_fx.rs).
pub use imp::debug_at_beer;
// Skate park demo driver, touch jump hold and park telemetry (skatepark.rs).
pub use imp::{debug_park_demo, park_log, touch_jump_held};
// Ferris wheel debug driver and telemetry (ferris.rs).
pub use imp::{debug_wheel_demo, set_wheel_telemetry};
// Velvet Disco Club debug fast-timer, fentanyl test flags & telemetry hooks (club.rs, fentanyl.rs).
pub use imp::{debug_fast_club, debug_fentanyl_mode, debug_overdose_secs, rand_unit, set_club_telemetry};
// Turn-based Lexie showdown telemetry hook (lexie_battle.rs).
pub use imp::set_lexie_telemetry;
// Distributed multiplayer bridge (net.rs & controls.rs).
pub use imp::{net_drain, net_enabled, net_event, net_is_online, net_lock_char, net_open_char_select, net_publish, net_publish_mom, net_set_free_roam, net_set_online, net_set_scene, net_squad_info, net_telemetry};
