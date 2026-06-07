# Soyel interactive editor strategy

## Summary

Soyel should evolve from a static path-traced preview into a hybrid interactive
editor viewport. The interactive layer should be responsible for camera control,
selection, outlines, transform gizmos, and immediate visual feedback. The path
tracer should provide progressively refined high-fidelity frames for the same
scene state.

The recommended model is a stacked renderer:

1. A fast Three.js/Threlte viewport for interaction and editing.
2. A Lupin-rendered bitmap layer for path-traced refinement.

This keeps interaction responsive while still preserving Soyel's goal of
showing physically based material previews.

## Goals

- Let users orbit, pan, and zoom around the preview scene.
- Let users select scene parts and inspect them with crisp hover/selection
  outlines.
- Later, let users move and rotate parts with CAD-style triad/gizmo controls.
- Keep transform editing immediate, even when path tracing is slow or
  unavailable.
- Keep Lupin as the high-quality renderer, not the source of UI overlays or
  interaction state.

## Architecture

### Canonical scene state

There must be one canonical editor scene model. Three.js/Threlte and Lupin
should both render from this state rather than maintaining independent scene
truth.

Suggested shape:

```ts
type ViewportScene = {
  revision: number;
  camera: CameraState;
  objects: ViewportObject[];
  selection: string | null;
};

type CameraState = {
  position: [number, number, number];
  target: [number, number, number];
  up: [number, number, number];
  fovDegrees: number;
};

type ViewportObject = {
  id: string;
  meshId: string;
  transform: Transform;
  materialId: string;
  visible: boolean;
};

type Transform = {
  translation: [number, number, number];
  rotation: [number, number, number, number];
  scale: [number, number, number];
};
```

All interactions should update this model first. Renderers should be consumers.

### Interactive layer

Use Three.js through Threlte for the editor viewport:

- Camera orbit/pan/zoom.
- Raycast-based hover and selection.
- Object outlines and selected-part highlights.
- TransformControls-style translate/rotate/scale gizmos.
- Grid, axes, bounds, and helper overlays.
- Immediate raster preview while a path-traced frame is stale or accumulating.

This layer should be visible during interaction. It should not be an invisible
camera-control helper. Once Soyel supports editing part transforms, Three's
scene graph and controls are useful enough to justify the dependency.

### Path-traced layer

Use Lupin/Rust for path-traced snapshots of the same canonical scene state:

- Render frames in the background for the current scene revision.
- Return image frames tagged with `revision`, camera state, dimensions, sample
  count, and accumulation status.
- Drop stale frames whose revision does not match the current editor state.
- Restart accumulation when camera, object transforms, materials, or visibility
  change.

The path-traced layer should not render gizmos, outlines, hover states, grid
overlays, or selection decorations.

## Layering behavior

Use stacked canvases or a canvas plus bitmap layer:

1. Bottom layer: latest accepted Lupin path-traced frame.
2. Top layer: Threlte/Three interactive viewport.
3. DOM overlay: toolbar, status, render progress, and lightweight labels.

During camera or transform interaction:

- Keep the Three layer fully visible or mostly opaque.
- Cancel or invalidate any in-progress path-traced frame.
- Show immediate raster movement, outlines, and gizmos.
- Mark the path-traced frame as stale if it no longer matches the scene
  revision.

After interaction settles:

- Debounce path tracing by roughly 150-250 ms.
- Submit the current scene revision to the backend.
- Fade the Three scene toward a low-opacity overlay if a matching path-traced
  frame is available.
- Keep outlines and transform gizmos crisp on top.
- Progressively replace the bottom bitmap as better Lupin frames arrive.

Avoid complex alpha compositing for the first version. The initial goal is a
clear mode transition: raster while editing, path-traced while inspecting.

## Data flow

1. User interacts with the Threlte viewport.
2. The frontend updates `ViewportScene` and increments `revision`.
3. The Three layer renders immediately from the new state.
4. Any pending Lupin request for an older revision is ignored or canceled.
5. After debounce, the frontend sends the current scene state to the backend.
6. The backend converts that state into a Lupin scene and camera.
7. The backend returns one or more path-traced frames tagged with the submitted
   revision.
8. The frontend accepts only frames whose revision still matches the current
   scene.

This revision contract is required. Without it, stale path-traced frames will
flash in after the user moves the camera or transforms an object.

## Interaction model

Initial camera controls:

- Left drag on empty space: orbit camera around target.
- Wheel or trackpad scroll: dolly in/out.
- Middle drag, right drag, or shift-left drag: pan target.
- Double click object: focus camera target on object bounds.
- Double click empty space or toolbar command: reset camera.

Initial object controls:

- Click object: select.
- Hover object: outline or tint in Three layer.
- Selected object: persistent outline and transform triad.
- Translate/rotate/scale modes can follow Three TransformControls conventions:
  `W` translate, `E` rotate, `R` scale, `Q` local/world space, `Esc` cancel.

Orbit controls should be disabled while dragging transform controls so camera
movement and object movement do not compete.

## Backend implications

The current `render_preview_frame` command accepts only width, height, and
sample count. It should eventually accept an editor snapshot:

```ts
type RenderPreviewRequest = {
  revision: number;
  width: number;
  height: number;
  samples: number;
  camera: CameraState;
  objects: ViewportObject[];
};
```

The backend response should include:

```ts
type RenderPreviewFrame = {
  revision: number;
  width: number;
  height: number;
  samples: number;
  pixels: Uint8Array;
};
```

For the first hybrid implementation, the backend can keep rendering the simple
preview scene while honoring camera changes. Object transform editing can be
added after the canonical scene model exists.

## Recommended milestones

### Milestone 1: Interactive camera

- Add Three/Threlte viewport infrastructure.
- Render a simple raster proxy of the current preview scene.
- Add orbit/pan/zoom camera controls.
- Serialize camera state into the preview render request.
- Restart path tracing after camera changes settle.

### Milestone 2: Selection and overlays

- Add object IDs to the scene model.
- Add raycast hover and click selection.
- Render hover and selection outlines in Three.
- Keep the path-traced bitmap free of overlays.

### Milestone 3: Transform controls

- Add translate/rotate controls for selected parts.
- Update canonical object transforms in real time.
- Serialize object transforms to Lupin.
- Restart path tracing after transform changes settle.

### Milestone 4: Progressive path tracing

- Move from single-frame preview requests to cancellable progressive jobs.
- Stream or poll updated sample counts for the current revision.
- Drop stale frames aggressively.
- Show render progress in the viewport chrome.

## Risks and constraints

- Geometry and material data will exist in both Three and Lupin forms. Keep
  conversion explicit and derived from canonical scene data.
- Three raster materials will not perfectly match Lupin. Keep them simple but
  consistent in base color, roughness, metallic, opacity, and visibility.
- Path tracing will lag interaction. This is acceptable if stale frames are
  clearly invalidated and the raster layer remains responsive.
- Transform gizmos, outlines, grids, axes, and hover effects must never be sent
  to Lupin as renderable scene geometry.
- The Lupin packed-buffer macOS workaround is still needed for path-traced
  output on Apple Silicon. The Three layer only solves interactivity.

## Default recommendation

Use Threlte/Three as the visible interactive editor layer and Lupin as the
progressive refinement renderer. Do not use Three as a hidden helper only, and
do not try to make Lupin responsible for CAD-style interaction overlays.
