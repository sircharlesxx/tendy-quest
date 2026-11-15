# Tendy Quest — Game Design & Antigravity Context

## 1. Project Overview
- **Title:** Tendy Quest
- **Genre:** 3D Third-Person Arcade Action-Platformer / Collectathon
- **Target Engine:** Godot 4.3+ (GDScript) with `.glb` assets generated via Blender 4.x Python scripting
- **Core Premise:** Play as a cheerful hero in a red-and-black buffalo plaid onesie navigating oversized domestic environments (Giant Kitchen, Pantry Canyon, Freezer Fortress) to collect golden crispy Chicken Tendies while evading and outsmarting anthropomorphic healthy-food monsters.

## 2. Protagonist ("Plaid Hero")
- **Visual Identity:** Stocky, upbeat man with messy light-brown hair, a short beard/mustache, wearing a cozy **red-and-black buffalo plaid onesie** (`#B81D24` red / `#141414` black check pattern) and dark grey slip-on shoes.
- **Signature Moves:**
  - **Run & Jump:** Snappy third-person movement with double-jump and coyote time.
  - **Belly Slide / Plaid Dash:** Short burst of speed with invulnerability frames to dodge charging veggies.
  - **Victory Dance:** Emote triggered upon collecting golden tendies (inspired by the original dance video clip).
- **Health / Craving Meter:** Starts at 100%. Taking hits from healthy vegetables reduces the meter; eating Tendies restores it and increases the score multiplier.

## 3. Collectibles & Power-Ups
- **Golden Chicken Tendy (Primary Collectible):** Floating, rotating crispy golden tender (`+100 pts`, `+15% Craving`).
- **Honey Mustard Cup (Speed & Jump Boost):** Grants 10 seconds of double movement speed and golden trail particles.
- **Ranch Shield (Defense):** Surrounds the player in a creamy bubble that absorbs one veggie hit and knocks the enemy back.

## 4. Anthropomorphic Healthy Food Monsters
1. **Broccoli Brute (Heavy Tank):**
   - *Appearance:* Thick green stalk body, wide muscular arms, dense dark-green floret crown, grumpy brow.
   - *Behavior:* Patrols between waypoints; charges slowly when player enters 10m vision cone; performs a ground-pound AoE slam at close range.
2. **Carrot Lancer (Fast Charger):**
   - *Appearance:* Bright orange tapered cone body, leafy green hair tuft, pointed tip.
   - *Behavior:* Locks onto player line-of-sight, telegraphs for 0.8s, then lunges forward in a straight line (stuns itself briefly if it hits a wall).
3. **Kale Slime (Area Denial):**
   - *Appearance:* Low, undulating mound of dark leafy greens.
   - *Behavior:* Leaves a temporary "Fiber Trail" on the floor that slows player movement by 40%.

## 5. Audio & Media Assets Carried Over
- Drop your exported files into:
  - `assets/sprites/hero_turnaround.png` (3D T-Pose turnaround reference sheet)
  - `assets/sprites/items_and_enemies.png` (Collectibles & Veggie Monsters sheet)
  - `assets/audio/tendies_from_the_sun.mp3` (Generated 125 BPM Bubblegum Pop theme track)
  - `assets/video/reference_dance.mp4` (Original isolated dance clip for animation reference)
