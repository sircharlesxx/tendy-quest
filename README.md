<p align="center">
  <img src="assets/readme/hero_banner.png" alt="Tendy Quest: Giant Kitchen Adventure" width="100%" />
</p>

<p align="center">
  <a href="https://sircharlesxx.github.io/tendy-quest/">
    <img src="assets/readme/play_button.svg" alt="Play Tendy Quest in Your Browser" width="480" />
  </a>
</p>

<p align="center">
  <a href="https://sircharlesxx.github.io/tendy-quest/"><img src="https://img.shields.io/badge/Play%20Live-GitHub%20Pages-f59e0b?style=for-the-badge&logo=googlechrome&logoColor=white" alt="Play Live on GitHub Pages"/></a>
  <img src="https://img.shields.io/badge/Rust-Bevy%200.15-ea580c?style=for-the-badge&logo=rust&logoColor=white" alt="Rust + Bevy 0.15"/>
  <img src="https://img.shields.io/badge/Target-WASM%20%2B%20WebGL2-6366f1?style=for-the-badge&logo=webassembly&logoColor=white" alt="WebAssembly + WebGL2"/>
  <img src="https://img.shields.io/badge/Assets-Blender%20PBR%203D-0d9488?style=for-the-badge&logo=blender&logoColor=white" alt="Blender PBR 3D Assets"/>
  <img src="https://img.shields.io/badge/Input-Desktop%20%2B%20Touch-be185d?style=for-the-badge" alt="Desktop and Mobile Touch Support"/>
</p>

---

## About the Game

Shrink down to miniature size and sprint across a towering **Giant Kitchen** countertop as the **Plaid Hero**! Armed with a red-and-black buffalo plaid onesie, acrobatic **Double-Jumps**, and an invulnerable **Belly Dash**, your mission is to grab all **10 Golden Tendies** and reach the stacked **Pancake Tower** before hungry waves of healthy-food monsters catch up with you.

<table>
  <tr>
    <td valign="top">
      <h3>Core Gameplay Loop</h3>
      <ul>
        <li><strong>Collect All 10 Golden Tendies:</strong> Hop across giant kitchen sponges, toaster rims, and maple syrup lanes to snag floating crispy tenders (<code>+100 pts</code> each, plus a <code>+5,000 pts</code> finisher bonus at the Pancake Stack!).</li>
        <li><strong>Survive 3 Escalating Waves:</strong> Reinforcements arrive every 16 seconds as Broccoli Brutes, fork-wielding Carrot Lancers, and sludgy Kale Slimes patrol the countertop.</li>
        <li><strong>Articulated 3D Animation:</strong> Stride, sprint, jump, belly-dash, and break into a victory dance with custom glTF joint-pivot limb animations—no static gliding!</li>
        <li><strong>Instant Browser Play:</strong> Compiles to WebAssembly + WebGL2 with automatic multi-touch controls on mobile devices.</li>
      </ul>
    </td>
    <td width="250" align="center" valign="middle">
      <img src="assets/readme/kitchen_dance.gif" alt="Plaid Hero dashing past Broccoli Brutes and Carrot Lancers in the Giant Kitchen" width="230" />
      <br/>
      <sub><em>Inspired by the Giant Kitchen concept video</em></sub>
    </td>
  </tr>
</table>

---

## Screenshots — Desktop & Mobile Touch

Play seamlessly on a widescreen desktop browser or straight from your phone's touchscreen with zero downloads:

<p align="center">
  <img src="assets/readme/gameplay_showcase.png" alt="Tendy Quest running on Desktop Browser, Mobile Landscape, and Mobile Portrait" width="100%" />
</p>

---

## Characters, Power-Ups & Veggie Monsters

<p align="center">
  <img src="assets/readme/cast_and_powerups.png" alt="Meet the Kitchen Crew, Power-Ups & Veggie Monsters" width="100%" />
</p>

| Entity | Role | Mechanics & Stats |
| :--- | :--- | :--- |
| **Plaid Hero** | Playable Hero | Starts with **5 Hearts**. Features camera-relative movement, **Double-Jump**, a regenerating **Belly Dash** (16.5 speed + 0.35s invulnerability), and a celebratory **Victory Dance**. |
| **Golden Tendy** | Primary Collectible | **10 total** across the kitchen arena (`+100 pts` each). Reaching the 10th final tendy at the Pancake Stack triggers **Stage Clear** (`+5,000 pts`). |
| **Honey Mustard Cup** | Speed Power-Up | Grants **10 seconds** of **1.75× movement speed** for zooming past hazards and grabbing out-of-reach tendies. |
| **Ranch Shield** | Defense Power-Up | Surrounds you in a creamy **1-hit protective bubble** that absorbs incoming veggie damage and knocks attackers back. |
| **Broccoli Brute** | Heavy Tank | Patrols key routes at 2.2 speed and charges aggressively (3.8 speed) whenever you enter its **9.0m** range. |
| **Carrot Lancer** | Fast Charger | Locks onto your line of sight within **10.5m**, **telegraphs for 0.8s** with raised fork, then **lunges at 11.5 speed** before a brief stun recovery. |
| **Kale Slime** | Area Denial Hazard | Glides along the countertop dropping temporary **Fiber Puddles** every 1.4s that slow your footsteps by **40%**. |

---

## Controls Guide

| Desktop (Mouse + Keyboard) | Mobile / Tablet (Touch Screen) |
| :--- | :--- |
| <ul><li>**Move:** `W A S D` or Arrow Keys</li><li>**3D Camera Look:** Move Mouse (`Click` locks cursor, `Esc` unlocks)</li><li>**Camera Zoom:** Mouse Scroll Wheel</li><li>**Jump / Double-Jump:** `Space`</li><li>**Belly Dash:** `Shift` (Left or Right)</li><li>**Controls / Tutorial:** `H`</li><li>**Mute / Unmute Audio:** `M`</li><li>**Start / Play Again:** `Space`, `Enter`, or `Click` button</li></ul> | <ul><li>**Move:** Bottom-left **Virtual Thumbstick** (full 360° analog speed)</li><li>**3D Camera Look:** **Swipe / Drag** anywhere on the viewport (simultaneous with walking!)</li><li>**Jump / Double-Jump:** Tap the golden **JUMP** button</li><li>**Belly Dash:** Tap the cyan **DASH** button</li><li>**Controls / Tutorial:** Tap **HELP** pill</li><li>**Mute / Unmute Audio:** Tap **SOUND** pill</li><li>**Start / Play Again:** Tap **anywhere** on the overlay</li></ul> |

---

## Music & Sound Effects

Tendy Quest features a custom WebAudio soundtrack and positional SFX pipeline wired from Rust via `wasm-bindgen` (automatically unlocked on your first click, key press, or touch):

- **Background Music:** [`assets/audio/tendies_from_the_sun.mp3`](assets/audio/tendies_from_the_sun.mp3) — upbeat 16-bar **125 BPM Bubblegum Pop** synth loop.
- **Kitchen Ambience:** [`assets/audio/kitchen_ambience.mp3`](assets/audio/kitchen_ambience.mp3) — crossfade-looped room ambience extracted from the reference kitchen clip.
- **11 Stereo Sound Effects (`.wav`):** `jump`, `double_jump`, `dash`, `tendy`, `powerup`, `hit`, `shield`, `step`, `lunge`, `win`, and `lose`.

---

## Tech Stack & Building from Source

- **Engine:** [Rust](https://www.rust-lang.org/) + [Bevy 0.15](https://bevyengine.org/) (`webgl2`, `bevy_gltf`, `bevy_pbr`, `bevy_ui`)
- **Web Packaging:** [`trunk`](https://trunkrs.dev/) + `wasm-bindgen` deploying static artifacts from `dist/web/` to GitHub Pages
- **3D & Media Pipeline:** Python-scripted Blender 5.x procedural PBR `.glb` models ([`blender/build_tendy_quest_assets.py`](blender/build_tendy_quest_assets.py)), NumPy/`ffmpeg` audio synthesis ([`blender/build_tendy_quest_audio.py`](blender/build_tendy_quest_audio.py)), and Pillow/`ffmpeg` README graphics ([`blender/build_readme_graphics.py`](blender/build_readme_graphics.py))

### Prerequisites

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk
```

### Run Locally in Your Browser

```bash
cd rust
trunk serve
```

### Build Production GitHub Pages Bundle (`dist/web/`)

```bash
cd rust
trunk build --release --public-url ./ --dist ../dist/web
```

### Regenerate 3D Models, Audio & README Graphics (Optional)

```bash
# Regenerate all 13 articulated PBR .glb models in assets/models/
blender --background --python blender/build_tendy_quest_assets.py

# Regenerate background music & 11 stereo SFX in assets/audio/
python3 blender/build_tendy_quest_audio.py

# Regenerate README banners, showcase panels & GIF in assets/readme/
python3 blender/build_readme_graphics.py
```

---

<p align="center">
  <strong>Grab a golden tendy and watch out for steamed veggies!</strong><br/>
  <a href="https://sircharlesxx.github.io/tendy-quest/">Play Tendy Quest Live &rarr;</a>
</p>
