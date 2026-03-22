"""Bevy Bistro WASM benchmark - visible browser, FPS measurement."""
from playwright.sync_api import sync_playwright
import time
import os

SCREENSHOTS_DIR = "/tmp/bevy_wasm_screenshots"
os.makedirs(SCREENSHOTS_DIR, exist_ok=True)

FPS_MEASURE_JS = """(seconds) => {
    return new Promise(resolve => {
        let frames = 0;
        let start = performance.now();
        let frameTimes = [];
        let lastTime = start;
        function count() {
            let now = performance.now();
            frameTimes.push(now - lastTime);
            lastTime = now;
            frames++;
            if (now - start < seconds * 1000) {
                requestAnimationFrame(count);
            } else {
                let elapsed = (now - start) / 1000;
                frameTimes.sort((a, b) => a - b);
                let p1 = frameTimes[Math.floor(frameTimes.length * 0.01)];
                let p50 = frameTimes[Math.floor(frameTimes.length * 0.50)];
                let p99 = frameTimes[Math.floor(frameTimes.length * 0.99)];
                let avg = frameTimes.reduce((a, b) => a + b, 0) / frameTimes.length;
                resolve({
                    fps: frames / elapsed,
                    frames: frames,
                    seconds: elapsed,
                    avg_ms: avg,
                    p1_ms: p1,
                    p50_ms: p50,
                    p99_ms: p99,
                });
            }
        }
        requestAnimationFrame(count);
    });
}"""

def bench(page, url, label, load_wait=25, measure_sec=10):
    console_logs = []
    page.on("console", lambda msg: console_logs.append(f"[{msg.type}] {msg.text}"))

    print(f"\n{'='*60}")
    print(f"  {label}")
    print(f"{'='*60}")

    page.goto(url, wait_until="domcontentloaded", timeout=60000)
    print(f"  Loading Bistro scene ({load_wait}s for 181MB GLB)...")
    time.sleep(load_wait)

    # Screenshot after loading
    path = f"{SCREENSHOTS_DIR}/bistro_{label.replace(' ', '_').lower()}.png"
    page.screenshot(path=path)
    print(f"  Screenshot: {path}")

    # Measure FPS
    print(f"  Measuring FPS over {measure_sec}s with orbiting camera...")
    fps_data = page.evaluate(FPS_MEASURE_JS, measure_sec)
    print(f"  FPS:     {fps_data['fps']:.1f}")
    print(f"  Avg:     {fps_data['avg_ms']:.2f}ms")
    print(f"  P1:      {fps_data['p1_ms']:.2f}ms (best 1%)")
    print(f"  P50:     {fps_data['p50_ms']:.2f}ms (median)")
    print(f"  P99:     {fps_data['p99_ms']:.2f}ms (worst 1%)")
    print(f"  Frames:  {fps_data['frames']}")

    # Memory
    mem = page.evaluate("() => performance.memory ? performance.memory.usedJSHeapSize / 1024 / 1024 : null")
    if mem:
        print(f"  JS Heap: {mem:.1f} MB")

    # Print bevy-related console logs
    bevy_logs = [l for l in console_logs if 'adapter' in l.lower() or 'error' in l.lower() or 'panic' in l.lower()]
    for log in bevy_logs[:5]:
        print(f"  {log[:150]}")

    return fps_data

with sync_playwright() as p:
    browser = p.chromium.launch(
        headless=False,
        args=["--enable-unsafe-webgpu", "--use-angle=metal"]
    )
    context = browser.new_context(viewport={"width": 1280, "height": 720})

    # Test WITHOUT fix first
    page1 = context.new_page()
    r1 = bench(page1, "http://localhost:8080/bistro_no_bindless/", "WITHOUT FIX", load_wait=30, measure_sec=10)

    # Test WITH fix
    page2 = context.new_page()
    r2 = bench(page2, "http://localhost:8080/bistro_bindless/", "WITH FIX", load_wait=30, measure_sec=10)

    # Final screenshots
    page1.screenshot(path=f"{SCREENSHOTS_DIR}/bistro_no_bindless_final.png")
    page2.screenshot(path=f"{SCREENSHOTS_DIR}/bistro_bindless_final.png")

    print(f"\n{'='*60}")
    print(f"  BISTRO EXTERIOR WASM BENCHMARK RESULTS")
    print(f"{'='*60}")
    print(f"  WITHOUT fix: {r1['fps']:.1f} FPS (avg {r1['avg_ms']:.1f}ms, p99 {r1['p99_ms']:.1f}ms)")
    print(f"  WITH fix:    {r2['fps']:.1f} FPS (avg {r2['avg_ms']:.1f}ms, p99 {r2['p99_ms']:.1f}ms)")
    if r1['fps'] > 0:
        print(f"  Speedup:     {r2['fps'] / r1['fps']:.2f}x")

    time.sleep(3)
    browser.close()
