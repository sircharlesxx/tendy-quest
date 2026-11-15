#!/usr/bin/env python3
"""Generate friendly, playful README graphics and animated GIF for Tendy Quest."""

from pathlib import Path
import subprocess
import sys
import tempfile

from PIL import Image, ImageDraw, ImageFilter, ImageFont


ROOT = Path(__file__).resolve().parent.parent
OUT_DIR = ROOT / "assets" / "readme"
VIDEO_PATH = ROOT / "assets" / "video" / "gemini_generated_video_23f12819.mp4"

SHOT_DESKTOP = Path("/home/tpeters/.gemini/antigravity-cli/brain/a57795ae-ab44-4094-bfb9-bebac1762ce7/scratch/desktop_playing.png")
SHOT_WALKING = Path("/home/tpeters/.gemini/antigravity-cli/brain/a57795ae-ab44-4094-bfb9-bebac1762ce7/scratch/desktop_apartment_floor.png")
SHOT_PORTRAIT = Path("/home/tpeters/.gemini/antigravity-cli/brain/a57795ae-ab44-4094-bfb9-bebac1762ce7/scratch/mobile_pixel11_playing.png")
SHOT_CITY = Path("/home/tpeters/.gemini/antigravity-cli/brain/a57795ae-ab44-4094-bfb9-bebac1762ce7/scratch/desktop_city_aerial.png")

FONT_DISPLAY = "/usr/share/fonts/aajohan-comfortaa-fonts/Comfortaa-Bold.otf"
FONT_EXTRA = "/usr/share/fonts/abattis-cantarell-fonts/Cantarell-ExtraBold.otf"
FONT_BOLD = "/usr/share/fonts/abattis-cantarell-fonts/Cantarell-Bold.otf"
FONT_REG = "/usr/share/fonts/abattis-cantarell-fonts/Cantarell-Regular.otf"


def font(path: str, size: int) -> ImageFont.FreeTypeFont:
    return ImageFont.truetype(path, size)


def verify_inputs() -> None:
    missing = [
        str(p)
        for p in [VIDEO_PATH, SHOT_DESKTOP, SHOT_WALKING, SHOT_PORTRAIT, SHOT_CITY]
        if not p.exists()
    ]
    if missing:
        raise FileNotFoundError(
            "Missing required media inputs for README generation:\n  "
            + "\n  ".join(missing)
        )


def extract_video_frame(timestamp: str, dest: Path) -> Image.Image:
    subprocess.run(
        [
            "ffmpeg",
            "-y",
            "-ss",
            timestamp,
            "-i",
            str(VIDEO_PATH),
            "-frames:v",
            "1",
            "-loglevel",
            "error",
            str(dest),
        ],
        check=True,
    )
    return Image.open(dest).convert("RGBA")


def build_kitchen_gif() -> None:
    out_gif = OUT_DIR / "kitchen_dance.gif"
    # 270px wide portrait GIF (~270x480) stays sharp for README embeds and stays under 2.5 MB
    subprocess.run(
        [
            "ffmpeg",
            "-y",
            "-ss",
            "0.0",
            "-t",
            "3.8",
            "-i",
            str(VIDEO_PATH),
            "-vf",
            (
                "fps=10,scale=270:-1:flags=lanczos,"
                "split[s0][s1];[s0]palettegen=max_colors=80:stats_mode=diff[p];"
                "[s1][p]paletteuse=dither=bayer:bayer_scale=3"
            ),
            "-loop",
            "0",
            "-loglevel",
            "error",
            str(out_gif),
        ],
        check=True,
    )
    size_mb = out_gif.stat().st_size / (1024 * 1024)
    print(f"Generated {out_gif.name} ({size_mb:.2f} MB)")


def build_play_button_svg() -> None:
    svg = """<svg xmlns="http://www.w3.org/2000/svg" width="560" height="74" viewBox="0 0 560 74" role="img" aria-label="Play Tendy Quest in Your Browser">
  <defs>
    <linearGradient id="btnGrad" x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" stop-color="#fbbf24"/>
      <stop offset="55%" stop-color="#f59e0b"/>
      <stop offset="100%" stop-color="#d97706"/>
    </linearGradient>
    <linearGradient id="rimGrad" x1="0%" y1="0%" x2="100%" y2="0%">
      <stop offset="0%" stop-color="#b81d24"/>
      <stop offset="50%" stop-color="#fde68a"/>
      <stop offset="100%" stop-color="#b81d24"/>
    </linearGradient>
    <filter id="softShadow" x="-4%" y="-8%" width="108%" height="124%">
      <feDropShadow dx="0" dy="4" stdDeviation="3" flood-color="#1a0d04" flood-opacity="0.45"/>
    </filter>
  </defs>
  <rect x="4" y="4" width="552" height="64" rx="18" fill="url(#btnGrad)" stroke="url(#rimGrad)" stroke-width="3" filter="url(#softShadow)"/>
  <rect x="14" y="14" width="12" height="44" rx="5" fill="#b81d24" opacity="0.88"/>
  <rect x="14" y="28" width="12" height="16" fill="#18181b" opacity="0.85"/>
  <rect x="534" y="14" width="12" height="44" rx="5" fill="#b81d24" opacity="0.88"/>
  <rect x="534" y="28" width="12" height="16" fill="#18181b" opacity="0.85"/>
  <polygon points="48,22 48,48 70,35" fill="#fffbeb" stroke="#78350f" stroke-width="2" stroke-linejoin="round"/>
  <text x="290" y="34" text-anchor="middle" font-family="system-ui, -apple-system, 'Segoe UI', sans-serif" font-size="20" font-weight="800" letter-spacing="0.8" fill="#1f1106">PLAY TENDY QUEST IN YOUR BROWSER!</text>
  <text x="290" y="54" text-anchor="middle" font-family="system-ui, -apple-system, 'Segoe UI', sans-serif" font-size="12.5" font-weight="700" fill="#451a03">Instant WebGL2 &#8226; Desktop Mouse/Keyboard &amp; Mobile Touch Controls</text>
</svg>
"""
    out_svg = OUT_DIR / "play_button.svg"
    out_svg.write_text(svg, encoding="utf-8")
    print(f"Generated {out_svg.name}")


def make_gradient_bg(w: int, h: int, top_rgb: tuple, bot_rgb: tuple) -> Image.Image:
    base = Image.new("RGBA", (w, h))
    px = base.load()
    for y in range(h):
        t = y / max(1, h - 1)
        r = int(top_rgb[0] + (bot_rgb[0] - top_rgb[0]) * t)
        g = int(top_rgb[1] + (bot_rgb[1] - top_rgb[1]) * t)
        b = int(top_rgb[2] + (bot_rgb[2] - top_rgb[2]) * t)
        for x in range(w):
            # Subtle countertop wood-grain planks horizontally
            plank = ((y // 48) % 2) * 4 - 2
            px[x, y] = (
                max(0, min(255, r + plank)),
                max(0, min(255, g + plank)),
                max(0, min(255, b + plank)),
                255,
            )
    return base


def draw_plaid_ribbon(draw: ImageDraw.ImageDraw, w: int, y0: int, h: int) -> None:
    sq = h
    for x in range(0, w, sq):
        col = (184, 29, 36, 255) if ((x // sq) % 2 == 0) else (24, 24, 28, 255)
        draw.rectangle([x, y0, x + sq - 1, y0 + h - 1], fill=col)
    # Center accent line
    draw.rectangle([0, y0 + h // 2 - 1, w, y0 + h // 2 + 1], fill=(220, 38, 38, 120))


def wrap_lines(
    draw: ImageDraw.ImageDraw, text: str, fnt: ImageFont.FreeTypeFont, max_w: int
) -> list[str]:
    words = text.split()
    lines: list[str] = []
    cur = ""
    for word in words:
        cand = f"{cur} {word}".strip()
        bbox = draw.textbbox((0, 0), cand, font=fnt)
        if bbox[2] - bbox[0] <= max_w or not cur:
            cur = cand
        else:
            lines.append(cur)
            cur = word
    if cur:
        lines.append(cur)
    return lines


def paste_rounded_card(
    canvas: Image.Image,
    img: Image.Image,
    pos: tuple[int, int],
    size: tuple[int, int],
    radius: int = 16,
    border_col: tuple = (251, 191, 36, 255),
    border_w: int = 5,
    angle: float = 0.0,
) -> None:
    tw, th = size
    fitted = img.resize((tw, th), Image.Resampling.LANCZOS)

    mask = Image.new("L", (tw, th), 0)
    mdraw = ImageDraw.Draw(mask)
    mdraw.rounded_rectangle([0, 0, tw - 1, th - 1], radius=radius, fill=255)

    card_w = tw + border_w * 2
    card_h = th + border_w * 2
    card = Image.new("RGBA", (card_w, card_h), (0, 0, 0, 0))
    cdraw = ImageDraw.Draw(card)
    cdraw.rounded_rectangle(
        [0, 0, card_w - 1, card_h - 1],
        radius=radius + border_w // 2,
        fill=border_col,
    )
    card.paste(fitted, (border_w, border_w), mask)

    # Drop shadow
    pad = 18
    sh = Image.new("RGBA", (card_w + pad * 2, card_h + pad * 2), (0, 0, 0, 0))
    sdraw = ImageDraw.Draw(sh)
    sdraw.rounded_rectangle(
        [pad, pad + 6, pad + card_w - 1, pad + card_h + 5],
        radius=radius + border_w // 2,
        fill=(0, 0, 0, 150),
    )
    sh = sh.filter(ImageFilter.GaussianBlur(7))
    sh.paste(card, (pad, pad), card)

    if angle != 0.0:
        sh = sh.rotate(angle, resample=Image.Resampling.BICUBIC, expand=True)

    canvas.alpha_composite(sh, dest=(pos[0] - pad, pos[1] - pad))


def draw_pill(
    draw: ImageDraw.ImageDraw,
    x: int,
    y: int,
    text: str,
    fnt: ImageFont.FreeTypeFont,
    bg_col: tuple,
    fg_col: tuple,
    pad_x: int = 14,
    pad_y: int = 7,
) -> int:
    bbox = draw.textbbox((0, 0), text, font=fnt)
    tw = bbox[2] - bbox[0]
    th = bbox[3] - bbox[1]
    pw = tw + pad_x * 2
    ph = th + pad_y * 2 + 2
    draw.rounded_rectangle([x, y, x + pw, y + ph], radius=ph // 2, fill=bg_col)
    tx = x + pad_x - bbox[0]
    ty = y + pad_y - bbox[1]
    draw.text((tx, ty), text, font=fnt, fill=fg_col)
    return pw


def build_hero_banner(v_hero: Image.Image) -> None:
    W, H = 1280, 440
    canvas = make_gradient_bg(W, H, (38, 22, 13), (22, 13, 8))

    # Warm spotlight glow on the right behind cards
    glow = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    gdraw = ImageDraw.Draw(glow)
    gdraw.ellipse([650, -60, 1290, 500], fill=(245, 158, 11, 52))
    gdraw.ellipse([760, 40, 1200, 410], fill=(251, 191, 36, 42))
    glow = glow.filter(ImageFilter.GaussianBlur(40))
    canvas.alpha_composite(glow)

    draw = ImageDraw.Draw(canvas)
    draw_plaid_ribbon(draw, W, 0, 12)
    draw_plaid_ribbon(draw, W, H - 12, 12)
    draw.rectangle([0, 12, W, 14], fill=(245, 158, 11, 220))
    draw.rectangle([0, H - 14, W, H - 12], fill=(245, 158, 11, 220))

    # Left column typography
    f_badge = font(FONT_EXTRA, 14)
    f_title = font(FONT_DISPLAY, 66)
    f_sub = font(FONT_EXTRA, 25)
    f_body = font(FONT_REG, 20)
    f_pill = font(FONT_BOLD, 14)

    draw_pill(
        draw,
        50,
        40,
        "3D WEBGL ARCADE COLLECTATHON",
        f_badge,
        (251, 191, 36, 255),
        (38, 20, 6, 255),
        pad_x=16,
        pad_y=7,
    )

    # Title with warm crisp drop shadow
    draw.text((53, 91), "TENDY QUEST", font=f_title, fill=(140, 35, 12, 255))
    draw.text((50, 88), "TENDY QUEST", font=f_title, fill=(254, 240, 138, 255))
    draw.text((52, 166), "1,200 SQ FT APARTMENT ADVENTURE", font=f_sub, fill=(251, 146, 60, 255))

    tagline = (
        "Start as a tiny 17cm guy on the breakfast island, eat all 12 Golden Tendies "
        "across a giant 1,200 sq ft apartment to grow into a 180cm Big Boy, and open "
        "the Giant Refrigerator!"
    )
    ty = 208
    for line in wrap_lines(draw, tagline, f_body, 625):
        draw.text((52, ty), line, font=f_body, fill=(254, 243, 199, 245))
        ty += 28

    # Feature pills cleanly arranged in 1 row
    pills = [
        ("Rust + Bevy 0.15", (194, 65, 12, 245)),
        ("Blender PBR 3D", (15, 118, 110, 245)),
        ("Eat-to-Grow", (190, 24, 93, 245)),
        ("Mobile Touch Ready", (67, 56, 202, 245)),
    ]
    px_cur = 52
    py_cur = 344
    for label, col in pills:
        pw = draw_pill(
            draw,
            px_cur,
            py_cur,
            label,
            f_pill,
            col,
            (255, 255, 255, 255),
            pad_x=14,
            pad_y=8,
        )
        px_cur += pw + 10

    # Right showcase polaroids: waving cheerful Plaid Hero + live browser walking gameplay
    hero_crop = v_hero.crop((140, 395, 580, 915))
    paste_rounded_card(
        canvas,
        hero_crop,
        pos=(730, 46),
        size=(215, 254),
        radius=16,
        border_col=(254, 240, 138, 255),
        border_w=6,
        angle=4.5,
    )

    walk_img = Image.open(SHOT_WALKING).convert("RGBA")
    paste_rounded_card(
        canvas,
        walk_img,
        pos=(865, 184),
        size=(328, 152),
        radius=14,
        border_col=(36, 36, 42, 255),
        border_w=9,
        angle=-3.2,
    )

    # Small cheerful badge on top-right
    draw = ImageDraw.Draw(canvas)
    draw_pill(
        draw,
        988,
        52,
        "DESKTOP + TOUCHSCREEN",
        font(FONT_EXTRA, 13),
        (184, 29, 36, 255),
        (254, 243, 199, 255),
        pad_x=14,
        pad_y=7,
    )

    out = OUT_DIR / "hero_banner.png"
    canvas.convert("RGB").save(out, quality=95)
    print(f"Generated {out.name}")


def build_gameplay_showcase() -> None:
    W, H = 1280, 520
    canvas = make_gradient_bg(W, H, (34, 22, 13), (20, 13, 8))
    draw = ImageDraw.Draw(canvas)

    draw.rounded_rectangle(
        [10, 10, W - 11, H - 11],
        radius=22,
        outline=(217, 119, 6, 180),
        width=2,
    )

    f_hdr = font(FONT_DISPLAY, 28)
    f_sub = font(FONT_REG, 17)
    f_cap_title = font(FONT_BOLD, 18)
    f_cap_sub = font(FONT_REG, 14)

    title = "PLAY ANYWHERE — DESKTOP OR MOBILE TOUCH"
    tb = draw.textbbox((0, 0), title, font=f_hdr)
    draw.text(((W - (tb[2] - tb[0])) // 2, 24), title, font=f_hdr, fill=(253, 230, 138, 255))

    sub = "Zero installs needed • Automatic virtual controls & touch camera look on phones and tablets"
    sb = draw.textbbox((0, 0), sub, font=f_sub)
    draw.text(((W - (sb[2] - sb[0])) // 2, 60), sub, font=f_sub, fill=(254, 215, 170, 230))

    im_desk = Image.open(SHOT_DESKTOP).convert("RGBA")
    im_land = Image.open(SHOT_WALKING).convert("RGBA")
    im_port = Image.open(SHOT_PORTRAIT).convert("RGBA")

    # Mathematically spaced columns:
    # Panel 1 (Desktop): x=48..500 (cx=274)
    # Panel 2 (Mobile Landscape): x=548..1016 (cx=782)
    # Panel 3 (Mobile Portrait): x=1064..1232 (cx=1148)
    desk_w, desk_h = 432, 243
    paste_rounded_card(
        canvas,
        im_desk,
        pos=(48, 128),
        size=(desk_w, desk_h),
        radius=12,
        border_col=(42, 42, 48, 255),
        border_w=10,
    )
    draw = ImageDraw.Draw(canvas)
    # Desktop stand under bezel
    draw.rectangle([250, 391, 298, 418], fill=(72, 72, 82, 255))
    draw.rounded_rectangle([208, 416, 340, 426], radius=5, fill=(95, 95, 108, 255))

    # Panel 2: Landscape smartphone
    land_w, land_h = 442, 204
    paste_rounded_card(
        canvas,
        im_land,
        pos=(548, 152),
        size=(land_w, land_h),
        radius=16,
        border_col=(32, 34, 40, 255),
        border_w=13,
    )
    draw = ImageDraw.Draw(canvas)
    # Speaker/camera pill on left phone bezel
    draw.rounded_rectangle([553, 244, 558, 292], radius=3, fill=(82, 82, 92, 255))

    # Panel 3: Portrait smartphone
    port_w, port_h = 148, 320
    paste_rounded_card(
        canvas,
        im_port,
        pos=(1064, 96),
        size=(port_w, port_h),
        radius=16,
        border_col=(32, 34, 40, 255),
        border_w=11,
    )
    draw = ImageDraw.Draw(canvas)
    draw.rounded_rectangle([1130, 101, 1168, 106], radius=3, fill=(82, 82, 92, 255))

    captions = [
        (274, "Desktop Apartment View", "Mouse 3D Look, Scroll Zoom & Belly Dash"),
        (782, "1,200 sq ft Open Floor", "Hunt Tendies Around Cheese Cups & Controllers"),
        (1140, "Mobile Touch Portrait", "Compact HUD & Virtual Touch Controls"),
    ]
    for cx, c_title, c_sub in captions:
        b1 = draw.textbbox((0, 0), c_title, font=f_cap_title)
        draw.text(
            (cx - (b1[2] - b1[0]) // 2, 444),
            c_title,
            font=f_cap_title,
            fill=(254, 243, 199, 255),
        )
        b2 = draw.textbbox((0, 0), c_sub, font=f_cap_sub)
        draw.text(
            (cx - (b2[2] - b2[0]) // 2, 470),
            c_sub,
            font=f_cap_sub,
            fill=(214, 211, 209, 235),
        )

    out = OUT_DIR / "gameplay_showcase.png"
    canvas.convert("RGB").save(out, quality=95)
    print(f"Generated {out.name}")


def draw_powerup_illustration(size: tuple[int, int]) -> Image.Image:
    """Crisp 2x-supersampled icon art for Honey Mustard Cup & Ranch Bubble Shield."""
    w, h = size[0] * 2, size[1] * 2
    img = Image.new("RGBA", (w, h), (31, 20, 12, 255))
    d = ImageDraw.Draw(img)

    # Soft cyan ranch glow & warm golden mustard glow
    glow = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    gd = ImageDraw.Draw(glow)
    gd.ellipse([18, 22, 168, 172], fill=(234, 179, 8, 95))
    gd.ellipse([78, 146, 234, 302], fill=(56, 189, 248, 110))
    glow = glow.filter(ImageFilter.GaussianBlur(16))
    img.alpha_composite(glow)
    d = ImageDraw.Draw(img)

    # Honey Mustard Dipping Cup (top-left)
    d.polygon([(46, 72), (144, 72), (128, 152), (62, 152)], fill=(250, 204, 21, 255))
    d.rounded_rectangle([38, 56, 152, 82], radius=12, fill=(245, 158, 11, 255), outline=(254, 249, 195, 255), width=3)
    d.ellipse([48, 60, 142, 76], fill=(253, 224, 71, 255))
    # Speed lines
    d.line([(20, 88), (44, 88)], fill=(253, 224, 71, 230), width=4)
    d.line([(14, 110), (48, 110)], fill=(253, 224, 71, 230), width=4)
    d.line([(24, 132), (52, 132)], fill=(253, 224, 71, 230), width=4)

    # Ranch Bubble Shield (bottom-right)
    cx, cy, r = 158, 224, 62
    d.ellipse([cx - r, cy - r, cx + r, cy + r], fill=(56, 189, 248, 70), outline=(125, 211, 252, 245), width=5)
    # Creamy ranch drop + herb flecks inside
    d.polygon([(cx, cy - 38), (cx - 28, cy + 8), (cx + 28, cy + 8)], fill=(248, 250, 252, 255))
    d.ellipse([cx - 28, cy - 10, cx + 28, cy + 36], fill=(248, 250, 252, 255))
    for fx, fy in [(cx - 10, cy + 12), (cx + 10, cy + 18), (cx, cy + 4)]:
        d.ellipse([fx - 3, fy - 3, fx + 3, fy + 3], fill=(34, 197, 94, 255))
    # Shield specular arc
    d.arc([cx - 46, cy - 46, cx + 46, cy + 46], start=205, end=275, fill=(255, 255, 255, 220), width=5)

    return img.resize(size, Image.Resampling.LANCZOS)


def build_cast_and_powerups(
    v_broccoli: Image.Image,
    v_carrot: Image.Image,
    v_tendy_slime: Image.Image,
    v_hero: Image.Image,
) -> None:
    W, H = 1280, 560
    canvas = make_gradient_bg(W, H, (34, 22, 13), (20, 13, 8))
    draw = ImageDraw.Draw(canvas)

    f_hdr = font(FONT_DISPLAY, 27)
    f_sub = font(FONT_REG, 16)
    f_role = font(FONT_EXTRA, 12)
    f_name = font(FONT_DISPLAY, 21)
    f_desc = font(FONT_REG, 15)

    title = "MEET STAN, SAUCE POWER-UPS & VEGGIE MONSTERS"
    tb = draw.textbbox((0, 0), title, font=f_hdr)
    draw.text(((W - (tb[2] - tb[0])) // 2, 20), title, font=f_hdr, fill=(253, 230, 138, 255))

    sub = "Eat crispy tenders to grow into a 180cm Big Boy while dodging grumpy greens"
    sb = draw.textbbox((0, 0), sub, font=f_sub)
    draw.text(((W - (sb[2] - sb[0])) // 2, 55), sub, font=f_sub, fill=(254, 215, 170, 225))

    cards = [
        {
            "name": "Stan (Plaid Hero)",
            "role": "HERO • PLAYABLE",
            "accent": (220, 38, 38, 255),
            "role_bg": (184, 29, 36, 255),
            "desc": "Starts as a tiny 17cm guy and eats tendies to grow into a 180cm Big Boy! Double-Jump, Belly Dash, and bowl veggies aside.",
            "img": v_hero.crop((185, 435, 505, 835)),
        },
        {
            "name": "Golden Tendy",
            "role": "COLLECTIBLE",
            "accent": (245, 158, 11, 255),
            "role_bg": (180, 83, 9, 255),
            "desc": "12 crispy tenders across the 1,200 sq ft apartment (+150 pts each). Eat them all to become a Big Boy & open the Giant Fridge!",
            "img": v_tendy_slime.crop((240, 175, 500, 445)),
        },
        {
            "name": "Sauce Power-Ups",
            "role": "BUFFS & SHIELD",
            "accent": (56, 189, 248, 255),
            "role_bg": (3, 105, 161, 255),
            "desc": "Honey Mustard gives 10s of 1.75x speed, while the icy Ranch Bubble absorbs one veggie hit & knocks foes back!",
            "img": draw_powerup_illustration((124, 168)),
        },
        {
            "name": "Broccoli Brute",
            "role": "HEAVY TANK",
            "accent": (34, 197, 94, 255),
            "role_bg": (21, 128, 61, 255),
            "desc": "Dense floret crown and beefy knit arms. Patrols countertops and charges fast when you step inside 9m!",
            "img": v_broccoli.crop((445, 330, 720, 650)),
        },
        {
            "name": "Carrot Lancer",
            "role": "FAST CHARGER",
            "accent": (249, 115, 22, 255),
            "role_bg": (194, 65, 12, 255),
            "desc": "Armed with a shiny 4-tine fork! Locks onto your line of sight, telegraphs for 0.8s, then lunges fast.",
            "img": v_carrot.crop((130, 440, 440, 795)),
        },
        {
            "name": "Kale Slime",
            "role": "AREA HAZARD",
            "accent": (132, 204, 22, 255),
            "role_bg": (77, 124, 15, 255),
            "desc": "An undulating leafy blob gliding across the wood counter that slows your footsteps by 40% on contact.",
            "img": v_tendy_slime.crop((0, 680, 172, 888)),
        },
    ]

    cols_x = [36, 445, 854]
    rows_y = [94, 322]
    cw, ch = 390, 210

    for idx, c in enumerate(cards):
        col = idx % 3
        row = idx // 3
        x = cols_x[col]
        y = rows_y[row]

        draw = ImageDraw.Draw(canvas)
        draw.rounded_rectangle(
            [x, y, x + cw, y + ch],
            radius=18,
            fill=(43, 28, 18, 255),
            outline=c["accent"],
            width=2,
        )

        # Left portrait
        art = c["img"].resize((124, 168), Image.Resampling.LANCZOS)
        pmask = Image.new("L", (124, 168), 0)
        ImageDraw.Draw(pmask).rounded_rectangle([0, 0, 123, 167], radius=14, fill=255)
        canvas.paste(art, (x + 16, y + 21), pmask)
        draw = ImageDraw.Draw(canvas)
        draw.rounded_rectangle(
            [x + 15, y + 20, x + 140, y + 189],
            radius=14,
            outline=c["accent"],
            width=2,
        )

        # Right card details
        tx = x + 156
        draw_pill(
            draw,
            tx,
            y + 20,
            c["role"],
            f_role,
            c["role_bg"],
            (255, 255, 255, 255),
            pad_x=11,
            pad_y=5,
        )
        draw.text((tx, y + 54), c["name"], font=f_name, fill=(255, 251, 235, 255))

        dy = y + 86
        for line in wrap_lines(draw, c["desc"], f_desc, cw - 172):
            draw.text((tx, dy), line, font=f_desc, fill=(231, 229, 228, 245))
            dy += 21

    out = OUT_DIR / "cast_and_powerups.png"
    canvas.convert("RGB").save(out, quality=95)
    print(f"Generated {out.name}")


def build_city_preview() -> None:
    """Crop a spoiler-free skyline preview from the city screenshot (no subtitle/UI spoilers)."""
    im_city = Image.open(SHOT_CITY).convert("RGB")
    cropped = im_city.crop((0, 0, min(1280, im_city.width), min(515, im_city.height)))
    out = OUT_DIR / "city_preview.png"
    cropped.save(out, quality=95)
    print(f"Generated {out.name} ({cropped.size[0]}x{cropped.size[1]})")


def main() -> int:
    verify_inputs()
    OUT_DIR.mkdir(parents=True, exist_ok=True)

    with tempfile.TemporaryDirectory() as tmp:
        tmp_dir = Path(tmp)
        v_broccoli = extract_video_frame("1.2", tmp_dir / "f_broccoli.png")
        v_carrot = extract_video_frame("3.5", tmp_dir / "f_carrot.png")
        v_tendy_slime = extract_video_frame("6.0", tmp_dir / "f_tendy_slime.png")
        v_hero = extract_video_frame("8.5", tmp_dir / "f_hero.png")

        build_kitchen_gif()
        build_play_button_svg()
        build_hero_banner(v_hero)
        build_gameplay_showcase()
        build_cast_and_powerups(v_broccoli, v_carrot, v_tendy_slime, v_hero)
        build_city_preview()

    print("All README assets generated successfully in", OUT_DIR)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as exc:
        print(f"Error: {exc}", file=sys.stderr)
        sys.exit(1)
