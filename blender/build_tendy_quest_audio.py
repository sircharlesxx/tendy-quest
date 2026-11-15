"""Generate background music and sound effects for Tendy Quest into assets/audio/."""

import math
import os
import subprocess
import tempfile
import wave
import numpy as np

SAMPLE_RATE = 44100
AUDIO_DIR = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "assets", "audio"))
VIDEO_PATH = os.path.abspath(
    os.path.join(os.path.dirname(__file__), "..", "assets", "video", "gemini_generated_video_23f12819.mp4")
)


def midi_to_hz(midi: float) -> float:
    return 440.0 * (2.0 ** ((midi - 69.0) / 12.0))


def write_wav(path: str, stereo: np.ndarray, sr: int = SAMPLE_RATE) -> None:
    """Write float32 stereo array in [-1, 1] of shape (N, 2) to 16-bit PCM WAV."""
    os.makedirs(os.path.dirname(path), exist_ok=True)
    peak = float(np.max(np.abs(stereo))) if stereo.size else 0.0
    if peak > 0.95:
        stereo = stereo * (0.92 / peak)
    pcm = np.clip(stereo, -1.0, 1.0)
    pcm_i16 = (pcm * 32767.0).astype(np.int16)
    with wave.open(path, "wb") as wf:
        wf.setnchannels(2)
        wf.setsampwidth(2)
        wf.setframerate(sr)
        wf.writeframes(pcm_i16.tobytes())


def encode_mp3(wav_path: str, mp3_path: str, bitrate: str = "160k") -> None:
    subprocess.run(
        ["ffmpeg", "-y", "-v", "error", "-i", wav_path, "-codec:a", "libmp3lame", "-b:a", bitrate, mp3_path],
        check=True,
    )


def mono_to_stereo(mono: np.ndarray, pan: float = 0.0) -> np.ndarray:
    """Equal-power pan in [-1, 1]."""
    angle = (pan + 1.0) * (math.pi * 0.25)
    return np.stack([mono * math.cos(angle), mono * math.sin(angle)], axis=-1)


def soft_adsr(n: int, a: float = 0.01, d: float = 0.08, s: float = 0.65, r: float = 0.05) -> np.ndarray:
    sr = SAMPLE_RATE
    na = min(n, max(1, int(a * sr)))
    nd = min(max(0, n - na), int(d * sr))
    nr = min(max(0, n - na - nd), int(r * sr))
    ns = max(0, n - na - nd - nr)
    parts = [
        np.linspace(0.0, 1.0, na, endpoint=False, dtype=np.float32),
        np.linspace(1.0, s, nd, endpoint=False, dtype=np.float32) if nd > 0 else np.empty(0, np.float32),
        np.full(ns, s, dtype=np.float32),
        np.linspace(s, 0.0, nr, endpoint=True, dtype=np.float32) if nr > 0 else np.empty(0, np.float32),
    ]
    env = np.concatenate(parts)[:n]
    return env


def add_at(buf: np.ndarray, start_idx: int, clip: np.ndarray, wrap: bool = False) -> None:
    n = len(clip)
    total = len(buf)
    if wrap:
        idx = (np.arange(n) + start_idx) % total
        np.add.at(buf, idx, clip)
        return
    if start_idx >= total or start_idx + n <= 0:
        return
    s0 = max(0, start_idx)
    c0 = s0 - start_idx
    length = min(total - s0, n - c0)
    buf[s0 : s0 + length] += clip[c0 : c0 + length]


def synth_bgm_loop() -> np.ndarray:
    """Synthesize a seamless 16-bar (30.72s) 125 BPM Bubblegum Pop track."""
    bpm = 125.0
    beat_sec = 60.0 / bpm  # 0.48s
    bar_sec = beat_sec * 4.0  # 1.92s
    bars = 16
    total_sec = bars * bar_sec  # 30.72s
    total_samples = int(round(total_sec * SAMPLE_RATE))  # 1354752
    mix = np.zeros((total_samples, 2), dtype=np.float32)

    rng = np.random.default_rng(125)

    # Chord progression in C Major / A Minor (I - V - vi - IV repeated with bridge variation)
    bar_roots_a = [60, 67, 69, 65]  # C, G, Am, F
    bar_triads_a = [
        [60, 64, 67],
        [59, 62, 67],
        [60, 64, 69],
        [60, 65, 69],
    ]
    bar_roots_b = [62, 67, 64, 69]  # Dm, G, Em, Am
    bar_triads_b = [
        [62, 65, 69],
        [59, 62, 67],
        [59, 64, 67],
        [60, 64, 69],
    ]

    # Drums on 16th-note grid per bar
    sixteenth = int(round((beat_sec / 4.0) * SAMPLE_RATE))
    kick_len = int(0.18 * SAMPLE_RATE)
    tk = np.arange(kick_len, dtype=np.float32) / SAMPLE_RATE
    f_kick = 46.0 + 115.0 * np.exp(-tk * 32.0)
    kick_phase = 2.0 * math.pi * np.cumsum(f_kick) / SAMPLE_RATE
    kick_mono = (np.sin(kick_phase) * np.exp(-tk * 14.0) + np.sin(tk * 220.0 * math.tau) * np.exp(-tk * 70.0) * 0.25) * 0.42
    kick = mono_to_stereo(kick_mono, 0.0)

    snare_len = int(0.15 * SAMPLE_RATE)
    ts = np.arange(snare_len, dtype=np.float32) / SAMPLE_RATE
    snare_noise = rng.uniform(-1.0, 1.0, snare_len).astype(np.float32)
    snare_tone = np.sin(2.0 * math.pi * 195.0 * ts * np.exp(-ts * 4.0))
    snare_mono = (snare_noise * np.exp(-ts * 22.0) * 0.55 + snare_tone * np.exp(-ts * 26.0) * 0.45) * 0.24
    snare = mono_to_stereo(snare_mono, 0.08)

    hat_len = int(0.055 * SAMPLE_RATE)
    th = np.arange(hat_len, dtype=np.float32) / SAMPLE_RATE
    hat_raw = rng.uniform(-1.0, 1.0, hat_len).astype(np.float32)
    hat_hp = np.concatenate([[0.0], np.diff(hat_raw)])
    hat_mono = hat_hp * np.exp(-th * 65.0) * 0.14
    hat_r = mono_to_stereo(hat_mono, 0.25)
    hat_l = mono_to_stereo(hat_mono * 0.85, -0.25)

    # Catchy 4-bar melody motif in C major (offset in 8th notes, midi note, duration in 8th notes)
    melody_motif_1 = [
        (0, 72, 1.0), (1, 74, 1.0), (2, 76, 1.5), (4, 79, 1.5), (6, 76, 1.0),
        (8, 74, 1.0), (9, 76, 1.0), (10, 74, 1.5), (12, 71, 1.5), (14, 74, 1.0),
        (16, 72, 1.0), (18, 76, 1.5), (20, 81, 1.5), (22, 79, 1.0),
        (24, 77, 1.5), (26, 76, 1.0), (27, 74, 1.0), (28, 72, 2.0),
    ]
    melody_motif_2 = [
        (0, 74, 1.0), (1, 77, 1.0), (2, 81, 1.5), (4, 79, 1.5), (6, 77, 1.0),
        (8, 76, 1.0), (10, 74, 1.0), (11, 76, 1.0), (12, 79, 2.0),
        (16, 76, 1.0), (17, 79, 1.0), (18, 83, 1.5), (20, 81, 1.5), (22, 79, 1.0),
        (24, 81, 1.5), (26, 79, 1.0), (28, 84, 2.5),
    ]

    eighth = int(round((beat_sec / 2.0) * SAMPLE_RATE))

    for bar in range(bars):
        bar_start = bar * 16 * sixteenth
        is_bridge = 8 <= bar < 12
        roots = bar_roots_b if is_bridge else bar_roots_a
        triads = bar_triads_b if is_bridge else bar_triads_a
        b_idx = bar % 4
        root_midi = roots[b_idx] - 24  # bass register
        chord = triads[b_idx]

        # Percussion (4-on-the-floor kick, crisp backbeat snare, offbeat hats)
        for step in range(16):
            pos = bar_start + step * sixteenth
            if step in (0, 4, 8, 12):
                add_at(mix, pos, kick, wrap=True)
            if step in (4, 12):
                add_at(mix, pos, snare, wrap=True)
            if step % 2 == 1:
                add_at(mix, pos, hat_r if (step % 4 == 1) else hat_l, wrap=True)
            elif bar % 4 == 3 and step in (14, 15):
                add_at(mix, pos, snare * 0.6, wrap=True)

        # Bouncy octave bass (8th notes)
        bass_pattern = [0, 12, 0, 12, 7, 12, 0, 12]
        for step8, semi in enumerate(bass_pattern):
            pos = bar_start + step8 * eighth
            n_samp = int(0.20 * SAMPLE_RATE)
            t = np.arange(n_samp, dtype=np.float32) / SAMPLE_RATE
            f = midi_to_hz(root_midi + semi)
            wave_b = (
                0.65 * np.sin(2.0 * math.pi * f * t)
                + 0.25 * np.sin(2.0 * math.pi * (2.0 * f) * t)
                + 0.12 * np.tanh(1.8 * np.sin(2.0 * math.pi * f * t))
            )
            env = soft_adsr(n_samp, a=0.006, d=0.07, s=0.4, r=0.05)
            add_at(mix, pos, mono_to_stereo(wave_b * env * 0.22, -0.04), wrap=True)

        # Warm stereo chord plucks on upbeat/quarter accents
        for step4 in range(4):
            pos = bar_start + step4 * 4 * sixteenth
            n_samp = int(0.38 * SAMPLE_RATE)
            t = np.arange(n_samp, dtype=np.float32) / SAMPLE_RATE
            env = soft_adsr(n_samp, a=0.012, d=0.14, s=0.35, r=0.12)
            chord_l = np.zeros(n_samp, dtype=np.float32)
            chord_r = np.zeros(n_samp, dtype=np.float32)
            for ni, m in enumerate(chord):
                f = midi_to_hz(m)
                det = 1.003 if ni % 2 == 0 else 0.997
                s1 = np.sin(2.0 * math.pi * f * t) + 0.28 * np.sin(2.0 * math.pi * 2.0 * f * t)
                s2 = np.sin(2.0 * math.pi * (f * det) * t) + 0.28 * np.sin(2.0 * math.pi * 2.0 * (f * det) * t)
                chord_l += s1
                chord_r += s2
            acc = 0.075 if step4 in (1, 3) else 0.055
            add_at(mix, pos, np.stack([chord_l * env * acc, chord_r * env * acc], axis=-1), wrap=True)

        # Lead melody rendered in 4-bar phrases
        if bar % 4 == 0:
            phrase = melody_motif_2 if is_bridge else melody_motif_1
            for off8, m_note, dur8 in phrase:
                pos = bar_start + int(off8 * eighth)
                n_samp = max(256, int(dur8 * beat_sec * 0.5 * 0.92 * SAMPLE_RATE))
                t = np.arange(n_samp, dtype=np.float32) / SAMPLE_RATE
                f = midi_to_hz(m_note)
                vib = 1.0 + 0.003 * np.sin(2.0 * math.pi * 5.5 * t)
                ph = 2.0 * math.pi * np.cumsum(f * vib) / SAMPLE_RATE
                lead = (
                    0.55 * np.sin(ph)
                    + 0.25 * np.sin(2.0 * ph)
                    + 0.14 * np.sign(np.sin(ph)) * 0.5
                    + 0.08 * np.sin(3.0 * ph)
                )
                env = soft_adsr(n_samp, a=0.01, d=0.08, s=0.68, r=0.06)
                pan = 0.18 * math.sin(bar * 0.7 + off8 * 0.2)
                add_at(mix, pos, mono_to_stereo(lead * env * 0.16, pan), wrap=True)

    # Gentle soft limiter
    mix = np.tanh(mix * 1.05) * 0.88
    return mix.astype(np.float32)


def extract_kitchen_ambience(mp3_path: str) -> None:
    """Extract and seamlessly loop audio from the kitchen reference video."""
    if not os.path.exists(VIDEO_PATH):
        return
    with tempfile.TemporaryDirectory() as tmp:
        raw_wav = os.path.join(tmp, "video_raw.wav")
        loop_wav = os.path.join(tmp, "ambience_loop.wav")
        subprocess.run(
            ["ffmpeg", "-y", "-v", "error", "-i", VIDEO_PATH, "-vn", "-ar", str(SAMPLE_RATE), "-ac", "2", raw_wav],
            check=True,
        )
        with wave.open(raw_wav, "rb") as wf:
            frames = wf.readframes(wf.getnframes())
            arr = np.frombuffer(frames, dtype=np.int16).astype(np.float32) / 32767.0
            arr = arr.reshape(-1, 2)
        fade_len = min(int(0.8 * SAMPLE_RATE), len(arr) // 4)
        if fade_len > 0 and len(arr) > fade_len * 2:
            t = np.linspace(0.0, 1.0, fade_len, dtype=np.float32)[:, None]
            head = arr[:fade_len] * t + arr[-fade_len:] * (1.0 - t)
            body = arr[fade_len:-fade_len]
            seamless = np.vstack([head, body])
        else:
            seamless = arr
        write_wav(loop_wav, seamless)
        encode_mp3(loop_wav, mp3_path, bitrate="128k")


def generate_sfx() -> None:
    """Synthesize punchy arcade SFX into assets/audio/*.wav."""
    rng = np.random.default_rng(2026)

    def save(name: str, mono: np.ndarray, pan: float = 0.0) -> None:
        write_wav(os.path.join(AUDIO_DIR, f"{name}.wav"), mono_to_stereo(mono.astype(np.float32), pan))

    # Jump: buoyant upward chirp
    t = np.arange(int(0.22 * SAMPLE_RATE), dtype=np.float32) / SAMPLE_RATE
    f = 240.0 + 440.0 * (1.0 - np.exp(-t * 14.0))
    ph = 2.0 * math.pi * np.cumsum(f) / SAMPLE_RATE
    jump = (0.7 * np.sin(ph) + 0.3 * np.sin(2.0 * ph)) * np.exp(-t * 11.0) * 0.55
    save("jump", jump)

    # Double jump: crisp higher airy twinkle
    t = np.arange(int(0.26 * SAMPLE_RATE), dtype=np.float32) / SAMPLE_RATE
    f = 420.0 + 620.0 * (1.0 - np.exp(-t * 16.0))
    ph = 2.0 * math.pi * np.cumsum(f) / SAMPLE_RATE
    djump = (0.65 * np.sin(ph) + 0.35 * np.sin(2.5 * ph)) * np.exp(-t * 10.0) * 0.50
    save("double_jump", djump)

    # Belly dash: filtered noise whoosh + low punch
    t = np.arange(int(0.24 * SAMPLE_RATE), dtype=np.float32) / SAMPLE_RATE
    noise = rng.uniform(-1.0, 1.0, len(t)).astype(np.float32)
    whoosh = np.convolve(noise, np.ones(14, dtype=np.float32) / 14.0, mode="same")
    punch = np.sin(2.0 * math.pi * np.cumsum(190.0 * np.exp(-t * 12.0)) / SAMPLE_RATE) * np.exp(-t * 16.0)
    dash = (whoosh * np.sin(math.pi * np.clip(t / 0.24, 0.0, 1.0)) * 0.65 + punch * 0.55) * 0.55
    save("dash", dash)

    # Golden Tendy pickup: crisp crunch + bright major triad chime (C6-E6-G6)
    n_tendy = int(0.42 * SAMPLE_RATE)
    t = np.arange(n_tendy, dtype=np.float32) / SAMPLE_RATE
    crunch = rng.uniform(-1.0, 1.0, int(0.06 * SAMPLE_RATE)).astype(np.float32) * np.exp(-t[: int(0.06 * SAMPLE_RATE)] * 48.0) * 0.35
    tendy = np.zeros(n_tendy, dtype=np.float32)
    tendy[: len(crunch)] += crunch
    for idx, freq in enumerate([1046.5, 1318.5, 1568.0]):
        s0 = int(idx * 0.055 * SAMPLE_RATE)
        nt = n_tendy - s0
        tt = np.arange(nt, dtype=np.float32) / SAMPLE_RATE
        tone = (np.sin(2.0 * math.pi * freq * tt) + 0.25 * np.sin(4.0 * math.pi * freq * tt)) * np.exp(-tt * 9.5) * 0.36
        tendy[s0:] += tone
    save("tendy", tendy)

    # Powerup: rising sparkle fanfare
    n_pow = int(0.50 * SAMPLE_RATE)
    power = np.zeros(n_pow, dtype=np.float32)
    for idx, freq in enumerate([523.25, 659.25, 783.99, 1046.5]):
        s0 = int(idx * 0.075 * SAMPLE_RATE)
        nt = n_pow - s0
        tt = np.arange(nt, dtype=np.float32) / SAMPLE_RATE
        tone = (0.65 * np.sin(2.0 * math.pi * freq * tt) + 0.35 * np.sin(3.0 * math.pi * freq * tt)) * np.exp(-tt * 7.5) * 0.35
        power[s0:] += tone
    save("powerup", power)

    # Hit: thud + buzzer
    t = np.arange(int(0.32 * SAMPLE_RATE), dtype=np.float32) / SAMPLE_RATE
    f_drop = 175.0 * np.exp(-t * 5.0) + 55.0
    ph = 2.0 * math.pi * np.cumsum(f_drop) / SAMPLE_RATE
    buzz = np.tanh(2.5 * np.sin(ph)) * np.exp(-t * 8.5)
    splat = rng.uniform(-1.0, 1.0, len(t)).astype(np.float32) * np.exp(-t * 24.0) * 0.35
    save("hit", (buzz * 0.55 + splat) * 0.55)

    # Shield deflect: glassy bubble bounce
    t = np.arange(int(0.30 * SAMPLE_RATE), dtype=np.float32) / SAMPLE_RATE
    f_sh = 580.0 + 360.0 * np.sin(t * 28.0) * np.exp(-t * 6.0)
    ph = 2.0 * math.pi * np.cumsum(f_sh) / SAMPLE_RATE
    shield = np.sin(ph) * np.exp(-t * 9.0) * 0.55
    save("shield", shield)

    # Soft sneaker step on wooden countertop
    t = np.arange(int(0.08 * SAMPLE_RATE), dtype=np.float32) / SAMPLE_RATE
    wood = np.sin(2.0 * math.pi * 135.0 * t * np.exp(-t * 18.0)) * np.exp(-t * 55.0)
    tap = rng.uniform(-1.0, 1.0, len(t)).astype(np.float32) * np.exp(-t * 85.0) * 0.25
    save("step", (wood * 0.45 + tap) * 0.35)

    # Carrot lancer fork lunge whoosh
    t = np.arange(int(0.28 * SAMPLE_RATE), dtype=np.float32) / SAMPLE_RATE
    f_ln = 320.0 + 480.0 * np.exp(-t * 9.0)
    ph = 2.0 * math.pi * np.cumsum(f_ln) / SAMPLE_RATE
    swoosh = rng.uniform(-1.0, 1.0, len(t)).astype(np.float32) * np.exp(-t * 14.0) * 0.22
    save("lunge", (np.sin(ph) * np.exp(-t * 11.0) * 0.40 + swoosh) * 0.55)

    # Stage clear celebratory fanfare
    n_win = int(1.35 * SAMPLE_RATE)
    win = np.zeros(n_win, dtype=np.float32)
    win_notes = [(0.0, 523.25, 0.14), (0.13, 659.25, 0.14), (0.26, 783.99, 0.14), (0.40, 1046.5, 0.65)]
    for t0, freq, dur in win_notes:
        s0 = int(t0 * SAMPLE_RATE)
        nt = min(n_win - s0, int((dur + 0.35) * SAMPLE_RATE))
        tt = np.arange(nt, dtype=np.float32) / SAMPLE_RATE
        env = soft_adsr(nt, a=0.01, d=0.08, s=0.7, r=0.25)
        tone = (0.65 * np.sin(2.0 * math.pi * freq * tt) + 0.28 * np.sin(4.0 * math.pi * freq * tt)) * env * 0.38
        win[s0 : s0 + nt] += tone
    save("win", win)

    # Game over descending sad trombone/synth
    n_lose = int(1.10 * SAMPLE_RATE)
    lose = np.zeros(n_lose, dtype=np.float32)
    lose_notes = [(0.0, 392.0, 0.22), (0.24, 369.99, 0.22), (0.48, 349.23, 0.22), (0.72, 311.13, 0.35)]
    for t0, freq, dur in lose_notes:
        s0 = int(t0 * SAMPLE_RATE)
        nt = min(n_lose - s0, int((dur + 0.08) * SAMPLE_RATE))
        tt = np.arange(nt, dtype=np.float32) / SAMPLE_RATE
        env = soft_adsr(nt, a=0.015, d=0.08, s=0.65, r=0.08)
        tone = np.tanh(1.5 * np.sin(2.0 * math.pi * freq * tt)) * env * 0.32
        lose[s0 : s0 + nt] += tone
    save("lose", lose)


if __name__ == "__main__":
    os.makedirs(AUDIO_DIR, exist_ok=True)
    with tempfile.TemporaryDirectory() as tmp:
        bgm_wav = os.path.join(tmp, "tendies_from_the_sun.wav")
        bgm = synth_bgm_loop()
        write_wav(bgm_wav, bgm)
        encode_mp3(bgm_wav, os.path.join(AUDIO_DIR, "tendies_from_the_sun.mp3"), bitrate="160k")
    extract_kitchen_ambience(os.path.join(AUDIO_DIR, "kitchen_ambience.mp3"))
    generate_sfx()
    print("Generated audio assets in:", AUDIO_DIR)
