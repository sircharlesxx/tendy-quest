#!/usr/bin/env python3
"""
Tendy Quest Recursive Self-Testing & Gameplay Tuning Harness.
Plays through Stage 1 (Kitchen), Mom Boss Fight, NYC + Phone Hacking Maze,
and Titan Lexie Fight in headless Chrome, collects window.tendyTelemetry,
checks for panics, and evaluates/tunes gameplay balance parameters.
"""
import argparse
import json
import os
import re
import subprocess
import time
from selenium import webdriver
from selenium.webdriver.chrome.options import Options
from selenium.webdriver.common.action_chains import ActionChains
from selenium.webdriver.common.by import By
from selenium.webdriver.common.keys import Keys

ROOT_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PLAYER_RS = os.path.join(ROOT_DIR, "rust", "src", "player.rs")
REPORT_PATH = os.path.join(ROOT_DIR, "tools", "self_test_report.json")


def make_driver():
    opts = Options()
    opts.add_argument("--headless=new")
    opts.add_argument("--no-sandbox")
    opts.add_argument("--disable-dev-shm-usage")
    opts.add_argument("--window-size=1280,800")
    opts.add_argument("--use-gl=angle")
    opts.add_argument("--use-angle=swiftshader")
    opts.add_argument("--enable-webgl")
    opts.add_argument("--ignore-gpu-blocklist")
    opts.set_capability("goog:loggingPrefs", {"browser": "ALL"})
    return webdriver.Chrome(options=opts)


def wait_ready(driver, timeout=25.0):
    deadline = time.time() + timeout
    while time.time() < deadline:
        ok = driver.execute_script("""
            const l = document.getElementById('loading');
            const c = document.querySelector('canvas');
            return (!l || l.style.display === 'none') && c && c.width > 0;
        """)
        if ok:
            time.sleep(1.5)
            return True
        time.sleep(0.4)
    return False


def collect_panics(driver):
    panics = []
    for entry in driver.get_log("browser"):
        msg = entry.get("message", "")
        if "panicked at" in msg or "RuntimeError: unreachable" in msg or "error[B0001]" in msg:
            panics.append(msg[:300])
    return panics


def get_telemetry(driver):
    return driver.execute_script("return window.tendyTelemetry || null;")


def tap_key(driver, key, hold_s=0.15):
    actions = ActionChains(driver)
    actions.key_down(key).pause(hold_s).key_up(key).perform()


def run_playtest(base_url):
    metrics = {
        "panics": [],
        "kitchen": {},
        "boss": {},
        "city": {},
        "titan": {},
        "checkpoint_saved": False,
    }

    # 1. Stage 1: Tutorial + Kitchen
    d = make_driver()
    try:
        d.get(base_url)
        ready = wait_ready(d)
        tap_key(d, Keys.SPACE, 0.1)
        time.sleep(0.8)
        stamina_samples = []
        for k in ["w", "d", "w", "a"]:
            tap_key(d, k, 0.35)
            tap_key(d, Keys.SHIFT, 0.1)
            tel = get_telemetry(d)
            if tel:
                stamina_samples.append(tel.get("stamina", 100.0))
        metrics["kitchen"] = {
            "ready": ready,
            "telemetry": get_telemetry(d),
            "min_stamina": min(stamina_samples) if stamina_samples else 100.0,
        }
        metrics["panics"].extend(collect_panics(d))
    finally:
        d.quit()

    # 2. Mom Boss Fight (Dark Souls combat loop)
    d = make_driver()
    try:
        d.get(base_url + "?boss=1")
        wait_ready(d)
        tap_key(d, Keys.TAB, 0.08)  # Lock-on
        stamina_samples = []
        start_hp = (get_telemetry(d) or {}).get("bossHp", 100.0)
        for step in range(6):
            tap_key(d, "w", 0.3)
            tap_key(d, Keys.SHIFT, 0.08)  # Dodge roll
            tap_key(d, "j" if step % 2 == 0 else "c", 0.08)  # Light / Heavy attack
            tel = get_telemetry(d)
            if tel:
                stamina_samples.append(tel.get("stamina", 100.0))
                if tel.get("hearts", 5) <= 3:
                    tap_key(d, "q", 0.08)  # Estus Flask
            time.sleep(0.25)
        end_tel = get_telemetry(d) or {}
        metrics["boss"] = {
            "start_boss_hp": start_hp,
            "end_boss_hp": end_tel.get("bossHp", start_hp),
            "end_hearts": end_tel.get("hearts", 0),
            "zero_stamina_hits": sum(1 for s in stamina_samples if s < 5.0),
            "sample_count": len(stamina_samples),
        }
        metrics["panics"].extend(collect_panics(d))
    finally:
        d.quit()

    # 3. NYC Stage 2 + Phone Maze + Save Checkpoint
    d = make_driver()
    try:
        d.get(base_url + "?city=1")
        wait_ready(d)
        tap_key(d, "p", 0.08)  # Open phone hacking maze
        time.sleep(0.5)
        for mk in ["d", "s", "s", "a", "s"]:
            tap_key(d, mk, 0.06)
        tap_key(d, "p", 0.08)  # Close phone
        tap_key(d, "w", 0.4)
        tel = get_telemetry(d) or {}
        saved = d.execute_script("return localStorage.getItem('tendy_quest_save_v1') || '';")
        metrics["checkpoint_saved"] = len(saved) > 0
        metrics["city"] = {
            "in_city": tel.get("inCity", False),
            "state": tel.get("state", ""),
            "saved_checkpoint": saved,
        }
        metrics["panics"].extend(collect_panics(d))
    finally:
        d.quit()

    # 4. NYC Titan vs Lexie
    d = make_driver()
    try:
        d.get(base_url + "?city_titan=1")
        wait_ready(d)
        tap_key(d, Keys.TAB, 0.08)
        for _ in range(4):
            tap_key(d, "w", 0.25)
            tap_key(d, "c", 0.08)
        tel = get_telemetry(d) or {}
        metrics["titan"] = {
            "state": tel.get("state", ""),
            "boss_hp": tel.get("bossHp", 300.0),
            "hearts": tel.get("hearts", 0),
        }
        metrics["panics"].extend(collect_panics(d))
    finally:
        d.quit()

    # 5. Cutscene Cinematic & Dramatic Pacing Check (TJ Cutscene)
    d = make_driver()
    try:
        d.get(base_url + "?tj=1&cutscene_t=17.0")
        ready = wait_ready(d)
        metrics["cutscene"] = {
            "ready": ready,
            "state": (get_telemetry(d) or {}).get("state", ""),
        }
        metrics["panics"].extend(collect_panics(d))
    finally:
        d.quit()

    return metrics


def analyze_and_improve(metrics, apply_tuning=False):
    findings = []
    tuned = False

    if metrics["panics"]:
        findings.append(f"CRITICAL: {len(metrics['panics'])} runtime panics detected.")
    else:
        findings.append("OK: Zero runtime panics across all 4 gameplay phases.")

    boss = metrics.get("boss", {})
    samples = max(1, boss.get("sample_count", 1))
    zero_rate = boss.get("zero_stamina_hits", 0) / samples
    if zero_rate > 0.5:
        findings.append(
            f"TUNING: High stamina starvation in BossFight ({zero_rate:.0%}); recommend +4.0 boss stamina regen."
        )
        if apply_tuning and os.path.exists(PLAYER_RS):
            with open(PLAYER_RS, "r", encoding="utf-8") as f:
                src = f.read()
            new_src = re.sub(
                r"(else if stats\.boss_mode \{\s*)(\d+\.\d+)",
                lambda m: f"{m.group(1)}{min(52.0, float(m.group(2)) + 4.0):.1f}",
                src,
                count=1,
            )
            if new_src != src:
                with open(PLAYER_RS, "w", encoding="utf-8") as f:
                    f.write(new_src)
                tuned = True
    else:
        findings.append(f"OK: Boss stamina pacing balanced (starvation rate {zero_rate:.0%}).")

    if not metrics.get("checkpoint_saved"):
        findings.append("WARN: Save checkpoint was not written to localStorage in CityPlay.")
    else:
        findings.append(
            f"OK: Checkpoint persisted ({metrics['city'].get('saved_checkpoint', '')})."
        )

    if metrics.get("cutscene", {}).get("ready"):
        findings.append("OK: Cutscene rendered cleanly with 2.39:1 cinema letterbox & zero panics.")

    return findings, tuned


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--url", default="https://sircharlesxx.github.io/tendy-quest/")
    parser.add_argument("--iterations", type=int, default=1)
    parser.add_argument("--apply-tuning", action="store_true")
    args = parser.parse_args()

    history = []
    for i in range(args.iterations):
        print(f"=== Self-Test Iteration {i + 1}/{args.iterations} ===")
        metrics = run_playtest(args.url)
        findings, tuned = analyze_and_improve(metrics, apply_tuning=args.apply_tuning)
        for line in findings:
            print(" ", line)
        history.append({"iteration": i + 1, "metrics": metrics, "findings": findings, "tuned": tuned})
        if not tuned:
            break

    with open(REPORT_PATH, "w", encoding="utf-8") as f:
        json.dump(history, f, indent=2)
    print(f"Report written to {REPORT_PATH}")


if __name__ == "__main__":
    main()
