# RT preview architecture

## Summary

Soyel's path-traced preview should run as a Rust-side render actor, not inside a
Tauri command body. Tauri IPC commands should validate and snapshot the current
request, enqueue a render job, and return immediately. The render worker should
own the long-running Lupin/WGPU work and stream progressive frames back through a
Tauri `Channel`.

This keeps the Svelte UI and Tauri IPC path responsive while Lupin accumulates
samples, waits on GPU readbacks, or rebuilds scene resources.

## Current contract

The frontend sends a preview request containing:

- `revision`: the canonical scene revision the frame belongs to.
- `width` and `height`: preview target dimensions.
- `samples`: requested accumulation sample count.
- `camera`: active editor camera.
- `scene`: immutable scene snapshot for the requested revision.
- `onFrame`: Tauri channel for binary frame packets.

The backend returns a `jobId` immediately. Progressive frame packets keep the
existing binary layout:

1. Magic and packet version.
2. Scene revision.
3. Width and height.
4. Completed samples and requested samples.
5. Final-frame flag.
6. RGBA8 pixels.

The frontend must continue to accept frames only when the packet revision matches
the current scene revision. The `jobId` is for debugging and future explicit
cancellation; it is not a replacement for revision checks.

## Backend model

`AppState` owns a `RenderDispatcher`. The dispatcher owns:

- A monotonically increasing preview job id.
- An atomic `latest_preview_job_id` used for latest-job-wins cancellation.
- A channel to one dedicated render worker.

`stream_preview_frame` should do only short-lived work:

1. Lock the renderer session.
2. Snapshot the selected surface and applied material override.
3. Clamp dimensions and sample count.
4. Submit a `PreviewRenderJob`.
5. Return the submitted `jobId`.

The render worker owns the expensive path:

1. Lazily initialize the headless Lupin/WGPU context.
2. Build reusable pathtrace and tonemap pipeline resources once.
3. Recreate reusable output textures only when dimensions change.
4. Build the Lupin scene for each immutable scene snapshot.
5. Accumulate samples one invocation at a time.
6. Emit frames at powers of two and on the final sample.
7. Stop before the next sample if a newer preview job has been submitted.

The worker is intentionally single-threaded. Preview rendering benefits more from
deterministic GPU ownership and cancellation than from a generic thread pool.

## Cancellation

Preview rendering uses latest-job-wins semantics:

- Submitting a preview job stores its `jobId` as the latest preview job.
- The worker skips queued jobs whose id is already stale.
- An active job checks the latest id before each sample.
- Stale jobs return without sending more frames.

Frame-level safety still belongs to the frontend revision check because already
sent frames can arrive after the editor has advanced to a newer scene revision.

## Resource reuse

The first actor implementation reuses:

- WGPU device and queue.
- Lupin pathtrace resources.
- Lupin tonemap resources.
- HDR accumulation textures and RGBA tonemap texture while dimensions are stable.

It may still rebuild scene GPU resources per job. That keeps the actor refactor
small and preserves the existing scene conversion path. A later scene-cache pass
can diff `SceneSnapshot` contents and update only changed geometry, materials, or
bindings.

## Failure behavior

The render worker logs render errors and panics with the affected surface id. The
IPC command only reports enqueue failures, such as the worker channel being
closed. This separation is deliberate: once a job has been accepted, frame
delivery is asynchronous and tied to the `Channel`.

Frontend state should treat missing final frames as a failed or canceled render
and keep the raster viewport available.
