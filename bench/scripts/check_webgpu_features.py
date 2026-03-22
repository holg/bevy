"""Check Chrome WebGPU features to verify bindless support.

Usage: python3 bench/scripts/check_webgpu_features.py

Requires: pip install playwright && playwright install chrome
Also requires a running HTTP server for WebGPU secure context.
Start one with: cd examples/wasm/dist && python3 -m http.server 8080
"""
from playwright.sync_api import sync_playwright
import time

with sync_playwright() as p:
    # Use installed Chrome (not bundled Chromium) for WebGPU support
    browser = p.chromium.launch(
        headless=False,
        channel="chrome",
        args=["--enable-unsafe-webgpu"],
    )
    page = browser.new_page()
    # WebGPU requires http:// (secure context)
    page.goto("http://localhost:8080/", wait_until="domcontentloaded")
    time.sleep(2)

    result = page.evaluate("""async () => {
        try {
            if (!navigator.gpu)
                return { error: 'WebGPU not available (navigator.gpu is ' + typeof navigator.gpu + ')' };
            const adapter = await navigator.gpu.requestAdapter();
            if (!adapter) return { error: 'No adapter returned' };
            const features = [];
            adapter.features.forEach(f => features.push(f));
            const limits = {};
            const limitNames = [
                'maxTextureDimension2D', 'maxBindGroups', 'maxBindingsPerBindGroup',
                'maxSampledTexturesPerShaderStage', 'maxSamplersPerShaderStage',
                'maxStorageBuffersPerShaderStage', 'maxStorageTexturesPerShaderStage',
            ];
            for (const name of limitNames) {
                if (name in adapter.limits) limits[name] = adapter.limits[name];
            }
            return { features: features.sort(), limits };
        } catch(e) { return { error: e.toString() }; }
    }""")

    if 'error' in result:
        print(f"Error: {result['error']}")
    else:
        print("Chrome WebGPU features:")
        for f in result['features']:
            print(f"  {f}")
        print("\nLimits:")
        for k, v in result.get('limits', {}).items():
            print(f"  {k}: {v}")
        bindless = [f for f in result['features']
                    if 'texture' in f or 'binding' in f or 'sampler' in f]
        print(f"\nBindless-related: {bindless if bindless else 'NONE - bindless not available in WebGPU'}")

    browser.close()
