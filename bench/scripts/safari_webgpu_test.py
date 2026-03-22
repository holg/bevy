"""Test Bevy WASM in Safari/WebKit via Playwright.

Usage: python3 bench/scripts/safari_webgpu_test.py
Requires: pip install playwright && playwright install webkit
"""
from playwright.sync_api import sync_playwright
import time

with sync_playwright() as p:
    browser = p.webkit.launch(headless=False)
    page = browser.new_page(viewport={"width": 1280, "height": 720})

    console_logs = []
    page.on("console", lambda msg: console_logs.append(f"[{msg.type}] {msg.text}"))
    page.on("pageerror", lambda err: print(f"  PAGE ERROR: {err}"))

    print("Opening Bistro in Safari/WebKit...")
    page.goto("http://localhost:8080/bistro_safari/", wait_until="domcontentloaded", timeout=60000)

    print("Waiting 30s for scene to load...")
    time.sleep(30)

    # Screenshot
    page.screenshot(path="/tmp/bevy_wasm_screenshots/safari_bistro.png")
    print("Screenshot: /tmp/bevy_wasm_screenshots/safari_bistro.png")

    # Check WebGPU
    gpu_check = page.evaluate("""async () => {
        try {
            if (!navigator.gpu) return { error: 'WebGPU not available' };
            const adapter = await navigator.gpu.requestAdapter();
            if (!adapter) return { error: 'No adapter' };
            const features = [];
            adapter.features.forEach(f => features.push(f));
            return { features: features.sort(), available: true };
        } catch(e) { return { error: e.toString() }; }
    }""")

    print(f"\nWebGPU: {gpu_check}")

    print(f"\nConsole logs ({len(console_logs)} total):")
    for log in console_logs[:30]:
        print(f"  {log[:200]}")

    time.sleep(5)
    browser.close()
