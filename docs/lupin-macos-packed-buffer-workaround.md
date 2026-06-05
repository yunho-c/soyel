# Lupin macOS packed-buffer WGPU workaround

## Summary

Lupin currently cannot initialize its WGPU renderer on Apple Silicon through the
Metal backend because it unconditionally requests
`wgpu::Features::BUFFER_BINDING_ARRAY`. WGPU's Metal backend does not expose
that feature on the Apple M4 Pro adapter, so device creation fails before any
scene can render.

This plan targets a Lupin fork whose first macOS workaround is a portable WGPU
software-BVH path based on packed aggregate storage buffers. It is not a native
MetalRT backend. MetalRT can remain a later backend project once the scene data
model is less dependent on Vulkan-style buffer binding arrays.

## Current blocker

The failure is rooted in Lupin's resource model:

- `request_device_for_lupin` requires `BUFFER_BINDING_ARRAY` along with texture
  and storage-resource binding arrays.
- `Scene` stores positions, indices, normals, texcoords, colors, BVH nodes, and
  alias tables as `Vec<wgpu::Buffer>`.
- `create_scene_bindgroup` and `create_sw_bvh_bindgroup` expose those vectors as
  `BindingResource::BufferArray`.
- `pathtracer.wgsl` declares those resources as `binding_array<...>` and indexes
  them dynamically by `mesh_idx`, `texcoords_buf_idx`, `light_idx`, and similar
  IDs.

The software BVH path is therefore not actually Metal-portable today, even
though it does not need hardware ray tracing. It still needs buffer binding
arrays for scene data and per-mesh BVH data.

## Target architecture

Add a packed scene representation that keeps `SceneCPU` as the public input
format but uploads Metal-compatible aggregate buffers for the GPU path.

The packed GPU representation should contain single storage buffers for each
resource family:

- `packed_positions: array<vec3f>`
- `packed_indices: array<u32>`
- `packed_normals: array<vec3f>`
- `packed_texcoords: array<vec2f>`
- `packed_colors: array<vec4f>`
- `packed_bvh_nodes: array<BvhNode>`
- `packed_alias_bins: array<AliasBin>`
- `packed_env_alias_bins: array<AliasBin>`

Add metadata tables that describe offsets and counts into those buffers:

- `PackedMeshInfo`: position/index/normal/texcoord/color offsets and counts,
  plus sentinel-style optional attribute handling.
- `PackedBvhInfo`: node offset and node count for each mesh BVH.
- `PackedAliasTableInfo`: alias-bin offset and count for each light or
  environment.

Keep these existing single-buffer resources as-is where possible:

- instances
- materials
- environments
- lights
- software TLAS nodes

Keep texture and sampler binding arrays for the first milestone. WGPU/Metal
already advertises texture/sampler binding-array support on the relevant
adapter, and replacing texture bindless access is not needed to remove the
current `BUFFER_BINDING_ARRAY` blocker.

## Implementation phases

### Phase 1: Data model and upload

Introduce an internal packed GPU scene type, for example `ScenePackedGpu`, while
leaving `SceneCPU` unchanged.

Add a packed upload function:

```rust
pub fn build_packed_accel_structures_and_upload(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    scene: &SceneCPU,
    textures: Vec<wgpu::Texture>,
    texture_views: Vec<wgpu::TextureView>,
    samplers: Vec<wgpu::Sampler>,
    envs_info: &[EnvMapInfo],
) -> ScenePackedGpu
```

The function should:

- Flatten each per-mesh vector into one aggregate buffer per resource family.
- Build per-mesh offset/count metadata.
- Build the software BVH per mesh exactly as today, but append all `BvhNode`
  arrays into `packed_bvh_nodes`.
- Preserve the current BVH behavior where building the software BVH may reorder
  triangle indices.
- Build light and environment alias tables as today, but flatten them into
  packed alias buffers with offset/count metadata.
- Upload non-empty dummy buffers only when a whole aggregate buffer would
  otherwise be empty.

### Phase 2: Packed WGPU shader path

Add packed WGSL shader variants rather than editing the existing bind-array
shader in place. The first target should be the software BVH renderer only.

Replace declarations like:

```wgsl
@group(0) @binding(1) var<storage, read> verts_pos_array: binding_array<VertsPos>;
@group(0) @binding(5) var<storage, read> indices_array: binding_array<Indices>;
@group(3) @binding(0) var<storage, read> bvh_nodes_array: binding_array<BvhNodes>;
```

with declarations like:

```wgsl
@group(0) @binding(1) var<storage, read> packed_mesh_infos: array<PackedMeshInfo>;
@group(0) @binding(2) var<storage, read> packed_positions: array<vec3f>;
@group(0) @binding(3) var<storage, read> packed_indices: array<u32>;
@group(3) @binding(0) var<storage, read> packed_bvh_infos: array<PackedBvhInfo>;
@group(3) @binding(1) var<storage, read> packed_bvh_nodes: array<BvhNode>;
```

Access should change mechanically:

- `verts_pos_array[mesh_idx].data[index]` becomes
  `packed_positions[mesh.position_offset + index]`.
- `indices_array[mesh_idx].data[index]` becomes
  `packed_indices[mesh.index_offset + index]`.
- `bvh_nodes_array[mesh_idx].data[node_idx]` becomes
  `packed_bvh_nodes[bvh.node_offset + node_idx]`.
- `alias_tables[light_idx].data[bin_idx]` becomes
  `packed_alias_bins[alias_table.offset + bin_idx]`.
- `env_alias_tables[env_idx].data[bin_idx]` becomes
  `packed_env_alias_bins[env_alias_table.offset + bin_idx]`.

Preserve the existing path tracing functions, material evaluation, BSDF
sampling, accumulation, debug modes, false-color modes, tonemapping, and
software BVH traversal semantics.

### Phase 3: Device feature selection

Split device creation into capability-aware paths:

- Existing path: keeps `BUFFER_BINDING_ARRAY` and the current shader/bindgroup
  model for Vulkan-capable adapters.
- Packed software path: requests only features needed by the packed WGPU
  software-BVH shader.

Add explicit feature probes:

```rust
pub fn supports_buffer_binding_arrays(adapter: &wgpu::Adapter) -> bool
pub fn supports_packed_wgpu_path(adapter: &wgpu::Adapter) -> bool
```

The packed path must not require `BUFFER_BINDING_ARRAY`. It may still require
texture/sampler binding arrays for the first milestone. If texture binding
arrays are unavailable on a backend, report the packed path as unsupported
rather than silently falling back to incorrect rendering.

### Phase 4: Runtime routing

Add an upload/render mode enum:

```rust
pub enum SceneUploadMode {
    LegacyBindArrays,
    PackedSoftwareBvh,
    Auto,
}
```

`Auto` should choose:

- `LegacyBindArrays` when `BUFFER_BINDING_ARRAY` is supported.
- `PackedSoftwareBvh` when buffer binding arrays are missing but packed-path
  requirements are present.
- A clear unsupported error otherwise.

On macOS/Metal with Apple Silicon, `Auto` should route to
`PackedSoftwareBvh`.

### Phase 5: Validation and cleanup

Keep the old and new paths side by side until render parity is proven. Do not
remove the existing WGPU ray-query path in the first packed-buffer milestone.

Only consider unifying or deleting the legacy bind-array path after:

- packed software BVH renders comparable images on Vulkan and Metal,
- performance is measured on representative scenes,
- texture/material parity is verified,
- downstream users have a migration path.

## Testing and acceptance criteria

### Compile and validation checks

- `check_shaders` or an equivalent shader compile test must compile the packed
  WGSL variant.
- The packed software-BVH shader must not declare `binding_array` for buffer
  resources.
- Existing legacy shaders must continue compiling.

### Render parity

Render the same small scene through legacy software BVH and packed software BVH
on a backend that supports both. Compare images with a tolerance rather than
exact pixel equality.

Required coverage:

- diffuse/matte materials,
- GLTF-PBR roughness and metallic behavior,
- vertex normals,
- missing normals with geometric-normal fallback,
- UV interpolation,
- normal maps if available,
- vertex colors,
- alpha/opacity skipping,
- emissive mesh lights,
- environment lighting,
- light and environment alias-table sampling,
- false-color and debug entrypoints.

### macOS acceptance

On Apple Silicon through WGPU/Metal:

- Device creation succeeds without `BUFFER_BINDING_ARRAY`.
- Packed scene upload succeeds for a small preview scene.
- Packed software BVH path renders an image.
- Soyel's material preview can use the packed path instead of showing the
  current unsupported-Lupin warning.
- Lack of native MetalRT support is not a failure for this milestone.

### Regression checks

- Existing Vulkan-capable legacy path still works.
- Existing WGPU ray-query path is not changed except for routing decisions.
- Existing public `SceneCPU` scene-building code continues to compile.

## Risks and tradeoffs

- Packed buffers add offset bookkeeping and increase the chance of indexing
  mistakes. Keep metadata structs small, explicit, and covered by tests.
- The first milestone does not solve backends that lack texture binding arrays.
  That is intentional to keep the change scoped to the current macOS blocker.
- Packed buffers may perform differently from per-mesh buffers. Measure before
  deleting the legacy path.
- MetalRT integration remains a separate project because WGPU does not expose
  Apple's native Metal acceleration-structure APIs in the form Lupin needs.

## Recommended first milestone

Implement only the packed software-BVH path and route Soyel's preview through it
on macOS/Metal. Do not attempt native MetalRT yet.

The minimum success condition is:

1. Lupin initializes on Apple Silicon without `BUFFER_BINDING_ARRAY`.
2. A small packed software-BVH scene renders through WGPU/Metal.
3. The same path can render Soyel's four-quad material preview scene.
4. Existing legacy WGPU behavior is preserved for adapters that support buffer
   binding arrays.
