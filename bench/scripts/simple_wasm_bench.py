"""Bevy WASM bindless benchmark - measure FPS via requestAnimationFrame."""
from playwright.sync_api import sync_playwright
import time
import os

SCREENSHOTS_DIR = "/tmp/bevy_wasm_screenshots"
os.makedirs(SCREENSHOTS_DIR, exist_ok=True)

FPS_MEASURE_JS = """() => {
    return new Promise(resolve => {
        let frames = 0;
        let start = performance.now();
        function count() {
            frames++;
            if (performance.now() - start < 5000) {
                requestAnimationFrame(count);
            } else {
                let elapsed = (performance.now() - start) / 1000;
                resolve({ fps: frames / elapsed, frames: frames, seconds: elapsed });
            }
        }
        requestAnimationFrame(count);
    });
}"""

GPU_FEATURES_JS = """async () => {
    try {
        const adapter = await navigator.gpu?.requestAdapter();
        if (!adapter) return { error: 'No WebGPU adapter' };
        const features = [...adapter.features];
        const limits = {};
        for (const [k, v] of Object.entries(Object.getOwnPropertyDescriptors(GPUSupportedLimits.prototype))) {
            if (v.get && k !== 'constructor') {
                try { limits[k] = adapter.limits[k]; } catch(e) {}
            }
        }
        return { features, limits };
    } catch(e) { return { error: e.message }; }
}"""

def bench(page, url, label, wait_load=12, measure_sec=5):
    console_logs = []
    page.on("console", lambda msg: console_logs.append(f"[{msg.type}] {msg.text}"))

    print(f"\n{'='*60}")
    print(f"  {label}")
    print(f"{'='*60}")

    page.goto(url, wait_until="domcontentloaded")
    print(f"  Loading scene ({wait_load}s)...")
    time.sleep(wait_load)

    # Measure FPS
    print(f"  Measuring FPS over {measure_sec}s...")
    fps_data = page.evaluate(FPS_MEASURE_JS)
    print(f"  FPS: {fps_data['fps']:.1f} ({fps_data['frames']} frames / {fps_data['seconds']:.1f}s)")

    # Screenshot
    path = f"{SCREENSHOTS_DIR}/{label.replace(' ', '_').lower()}.png"
    page.screenshot(path=path)
    print(f"  Screenshot: {path}")

    # Memory
    mem = page.evaluate("() => performance.memory ? performance.memory.usedJSHeapSize / 1024 / 1024 : null")
    if mem:
        print(f"  JS Heap: {mem:.1f} MB")

    # Bevy console logs
    bevy_logs = [l for l in console_logs if 'crates/' in l or 'bevy' in l.lower()]
    for log in bevy_logs[:10]:
        # Strip ANSI-like formatting
        clean = log.split('%c')[-1] if '%c' in log else log
        print(f"  {clean[:120]}")

    return fps_data

with sync_playwright() as p:
    browser = p.chromium.launch(
        headless=False,
        args=["--enable-unsafe-webgpu", "--use-angle=metal"]
    )
    context = browser.new_context(viewport={"width": 1280, "height": 720})

    # Check WebGPU features first
    check_page = context.new_page()
    check_page.goto("about:blank")
    gpu = check_page.evaluate(GPU_FEATURES_JS)
    check_page.close()

    print("\n" + "="*60)
    print("  WebGPU Adapter Features")
    print("="*60)
    if 'features' in gpu:
        for f in sorted(gpu['features']):
            print(f"  {f}")
        bindless = [f for f in gpu['features'] if 'texture' in f or 'sampler' in f or 'binding' in f]
        print(f"\n  Bindless-related: {bindless if bindless else 'NONE'}")
    else:
        print(f"  {gpu}")

    # Benchmark both versions
    page1 = context.new_page()
    r1 = bench(page1, "http://localhost:8080/no_bindless/", "WITHOUT FIX")

    page2 = context.new_page()
    r2 = bench(page2, "http://localhost:8080/bindless/", "WITH FIX")

    print(f"\n{'='*60}")
    print(f"  RESULTS")
    print(f"{'='*60}")
    print(f"  WITHOUT fix: {r1['fps']:.1f} FPS")
    print(f"  WITH fix:    {r2['fps']:.1f} FPS")
    if r1['fps'] > 0:
        ratio = r2['fps'] / r1['fps']
        print(f"  Ratio:       {ratio:.2f}x")

    time.sleep(5)
    browser.close()
