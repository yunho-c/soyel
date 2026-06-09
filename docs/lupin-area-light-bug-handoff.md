# Lupin Area Light Bug Handoff

Current Soyel commit with focused tests:

```text
3a4cd6b9aeedb07045772272b305653d22df343e test: isolate lupin area light rendering
```

Soyel depends on the local Lupin fork:

```toml
lupin_pt = { path = "../../LupinPathTracer-fork/lupin", package = "lupin_pt" }
```

Original Soyel render path:

```text
src-tauri/src/lib.rs:935
```

Soyel called `lupin_pt::pathtrace_scene(..., Default::default(), ...)`, where `Default::default()` is `PathtraceType::Standard`. The preview render also sets:

```rust
force_software_bvh: true
```

## Resolution

Soyel now routes both sync and streaming Lupin previews through `preview_pathtrace_type()`, which returns `lupin_pt::PathtraceType::Direct`.

Rationale: CPU-side conversion already proves emissive material preservation and Lupin light registration. Directly visible emissive geometry also renders. The missing behavior is receiver illumination from sampled lights, so Soyel should use Lupin's direct-light integrator instead of the default standard integrator for viewport previews.

The local sandbox used for this fix cannot acquire a WGPU adapter, so ignored GPU render tests still fail before rendering here. The non-GPU regression locks the selected integrator, and default Rust tests pass.

## Observed Problem

Imported glTF/GLB scenes that have no emissive material get a synthetic `Default Area Light`. Raster/Three preview shows the imported scene, but Lupin RT preview is black. The initial suspicion was that `Default Area Light` was not emissive or not registered as a light.

The focused smoke tests show the problem is narrower: the light is emissive and registered, but its sampled contribution does not illuminate non-emissive receiver geometry.

## What Is Proven

CPU-side scene conversion preserves the emissive material and registers the light:

```text
src-tauri/src/lib.rs:2114
emissive_plane_snapshot_registers_lupin_light

src-tauri/src/lib.rs:2134
emissive_plane_receiver_snapshot_registers_lupin_light
```

These assert:

- emissive material values survive Soyel scene conversion
- `lupin_pt::build_lights(&scene, &[])` returns exactly one light
- receiver scene has two instances: receiver plus emissive plane
- receiver material remains non-emissive

GPU-side direct visibility also works:

```text
src-tauri/src/lib.rs:2195
emissive_plane_only_scene_renders_nonzero_pixels

src-tauri/src/lib.rs:2225
emissive_triangle_mesh_only_scene_renders_nonzero_pixels
```

When the camera directly sees emissive geometry, Lupin renders non-black pixels.

## Original Receiver-Lighting Failure

With Soyel's original `PathtraceType::Standard` preview path, GPU-side receiver illumination failed:

```text
src-tauri/src/lib.rs:2255
emissive_plane_lights_triangle_mesh_receiver

src-tauri/src/lib.rs:2289
emissive_plane_lights_procedural_receiver
```

These scenes have:

- a visible non-emissive receiver facing the camera
- an emissive plane light outside direct camera view
- exactly one registered Lupin area light

Expected: receiver gets lit, image has non-black pixels.

Actual: fully black output:

```text
Rgba8Stats { non_black_pixels: 0, max_luma: 0, unique_rgb_count: 1 }
```

Historical reproduction from Soyel:

```sh
cd src-tauri
env RUSTC_WRAPPER= CARGO_BUILD_RUSTC_WRAPPER= cargo test
env RUSTC_WRAPPER= CARGO_BUILD_RUSTC_WRAPPER= cargo test emissive_ -- --ignored --nocapture
```

Default tests passed. Before the direct-light preview fix, the ignored GPU command passed the directly visible emissive tests and failed the receiver-lighting tests. After the fix, those ignored receiver-lighting tests exercise `PathtraceType::Direct`; they still require a supported WGPU adapter to run.

## Lupin Failure Boundary

The original `PathtraceType::Standard` failure was probably not in:

- glTF importer fallback light brightness
- Soyel material emission conversion
- `build_lights` CPU registration
- direct rendering of emissive geometry
- raster/Three scene reset

The remaining Lupin-level question is why `PathtraceType::Standard` did not contribute sampled area-light illumination for this scene. If that path is repaired in Lupin later, useful places to inspect include:

- `pathtrace_standard`
- `pathtrace_direct`
- `sample_lights`
- `sample_lights_pdf`
- `compute_instance_lights_pdf`
- `sample_instance_alias_table`
- packed software-BVH intersection used by light rays
- normal or BSDF rejection in `eval_matte` / `eval_gltfpbr`

Important Lupin files:

```text
/Users/yunhocho/GitHub/LupinPathTracer-fork/lupin/src/shaders/pathtracer_packed.wgsl
/Users/yunhocho/GitHub/LupinPathTracer-fork/lupin/src/shaders/bvh_packed.wgsl
/Users/yunhocho/GitHub/LupinPathTracer-fork/lupin/src/data_structures.rs
/Users/yunhocho/GitHub/LupinPathTracer-fork/lupin/src/renderer.rs
```

Relevant symbols:

```text
pathtrace_standard: pathtracer_packed.wgsl:591
pathtrace_direct: pathtracer_packed.wgsl:1065
eval_matte: pathtracer_packed.wgsl:1973
sample_lights: pathtracer_packed.wgsl:2471
sample_lights_pdf: pathtracer_packed.wgsl:2519
compute_tri_geom_normal: pathtracer_packed.wgsl:2564
sample_instance_alias_table: pathtracer_packed.wgsl:2628
compute_instance_lights_pdf: bvh_packed.wgsl:119
build_lights: data_structures.rs:18
build_packed_scene_cpu: data_structures.rs:1065
build_packed_accel_structures_and_upload: data_structures.rs:1186
```

## Optional Lupin Follow-Up

Soyel's viewport-preview fix is to use the direct-light integrator. A deeper Lupin fix would still benefit from Lupin-side packed GPU tests that render the same "vertical receiver plus off-camera area light" scene outside Soyel.

If that fails in Lupin directly, fix the issue in Lupin's packed light sampling / direct illumination path.

If that passes in Lupin directly, compare Soyel's `SceneCPU` fields, transforms, indices, material types, and uploaded packed scene buffers against the passing Lupin test scene.
