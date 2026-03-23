# RFC: Native Photometric Lighting for Bevy

## Summary

A proposal to add per-fragment photometric light profiles to Bevy — evaluating real measured luminaire distributions (IES/LDT/TM-33) directly in the PBR shader, rather than approximating with cubemap cookies or multiple spotlights.

**Live demo** (WebGPU required): https://iesna.eu/photometric_wip.html
Controls: 1-5 switch modes, H toggles heatmap, [/] switch LDT profiles, WASD/arrows camera

## The Problem

No real-time engine properly supports photometric light profiles:

- **Unity/Unreal**: Bake IES into a cubemap cookie — projective, not angular. Loses accuracy at grazing angles, breaks for area lights, doesn't support LDT or TM-33.
- **Current Bevy workaround** ([eulumdat-bevy](https://github.com/holg/eulumdat-rs)): Approximates with 5+ SpotLights per luminaire. Works for previews, but fundamentally wrong for asymmetric distributions (road lights, wall washers).

Architectural visualization, digital twins, and lighting design tools need accurate photometric rendering. Bevy's ECS architecture and WebGPU backend are well-suited for this.

## Proposed Approach

### Per-fragment angular intensity lookup

Instead of projecting from the light's perspective (cubemap cookie) or approximating with multiple spots, sample a 2D intensity texture at every fragment:

1. **CPU (load time)**: Parse IES/LDT file → resolve symmetry, C-plane rotation, Type B→C conversion → generate R16Float equirectangular texture (C-plane × gamma)
2. **GPU (every frame)**: In `point_light()`, convert light-to-fragment direction to (C, gamma) in luminaire local space → sample the texture → modulate intensity

```wgsl
// In point_light() — the core addition
let frag_dir = normalize(P - light.position);
let local_dir = inverse_rotation * frag_dir;
let gamma = acos(-local_dir.y);                    // 0 = nadir
let c_angle = atan2(local_dir.x, local_dir.z);     // azimuthal
let uv = vec2(c_angle / TAU + 0.5, gamma / PI);
let intensity = textureSample(photometric_texture, sampler, uv).r;
```

### Format support

| Format | Standard | Status |
|--------|----------|--------|
| **IES** (LM-63) | ANSI/IES, all versions 1991-2019 | WIP prototype |
| **LDT** (EULUMDAT) | European standard | WIP prototype |
| **TM-33** (ATLA S001) | Modern XML/JSON replacement | Planned |

### ECS design

Compositional — not a new light type:

```rust
commands.spawn((
    PointLight { intensity: 15000.0, ..default() },
    PhotometricLight { profile: asset_server.load("luminaire.ldt") },
    ColorTemperature::new(3000.0),  // Kelvin → sRGB on CPU
));
```

## Comparison Table

| Feature | Unity | Unreal | Bevy (current) | Bevy (proposed) |
|---------|-------|--------|-----------------|-----------------|
| IES profiles | Cubemap cookie | Cubemap cookie | Multi-spot approx | Per-fragment lookup |
| LDT (EULUMDAT) | No | No | Multi-spot approx | Per-fragment lookup |
| Evaluation method | Projective | Projective | N spots | Angular, per-fragment |
| Area light + photometric | No | No | No | LTC + angular modulation |
| Color temperature | Component | Property | Manual RGB | `ColorTemperature` component |
| Lights per luminaire | 1 | 1 | 5+ | 1 |

## Implementation Status

### WIP Prototype
- `ColorTemperature` component (Kelvin → linear RGB, Planckian locus approximation)
- `PhotometricProfile` asset type (R16Float equirectangular texture)
- `IesLoader` / `LdtLoader` — basic parsing, symmetry expansion, needs validation
- GPU pipeline scaffolding (descriptor buffer, bind group entries, shader code) — not yet validated end-to-end
- Extraction wiring (photometric index packed into `GpuClusteredLight::flags` upper 16 bits)
- Feature flag: `pbr_photometric_lights`

The heatmap visualization in the demo shows the correct CPU-side sampling. The GPU shader path needs further work to match.

### Prerequisites (PRs open)
- #23439 — Skip dynamic cluster resizing when GPU clustering is active
- #23436 — Enable partial bindless on Metal, reduce bind group overhead

### Composes with
- #23400 — LTC PointLight (merged into our experimental branch — photometric modulation × LTC area light evaluation)

## Phased Roadmap

1. **ColorTemperature** — standalone, small PR
2. **Asset loaders** (IES/LDT) — medium PR, no rendering changes
3. **GPU photometric sampling** — core PR, shader + bind groups + extraction
4. **LTC area lights + photometric** — builds on #23400
5. **Spectral pipeline** — long-term (multi-band rendering, SPD textures)

## Questions for Discussion

1. **Should photometric profiles reuse the existing light texture (cookie/decal) system, or be separate?** We chose separate — cookies are projective (`local_from_world * pos`), photometric is angular (`inverse_rotation * normalize(dir)`). Different parameterization.

2. **Where should format parsing live?** Currently in `bevy_light`. Could be a separate `bevy_photometric` crate.

3. **How to handle the C-plane orientation ambiguity?** IES and LDT define C0 differently, and manufacturers don't always follow the convention. Our current approach: no automatic rotation, user rotates via `Transform`.

4. **Binding budget on Metal/WebGPU** — photometric adds 3 bindings to group 1 (slots 8-10). Our #23436 PR frees headroom for this.

## References

- [Design document](https://github.com/holg/eulumdat-rs) (full technical detail in the eulumdat-rs repo)
- [ANSI/IES LM-63-2019](https://www.ies.org/) — IES file format standard
- [EULUMDAT specification](https://web.archive.org/web/20190716173235/http://www.helios32.com/Eulumdat.htm) — European luminaire data format
- [LTC paper](https://eheitzresearch.wordpress.com/415-2/) — Heitz, Hill, McGuire 2016
- [Live demo](https://iesna.eu/photometric_wip.html) (WebGPU required)
