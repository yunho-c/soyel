use std::{
    collections::{BTreeSet, HashMap},
    panic::{self, AssertUnwindSafe},
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::ipc::{Channel, InvokeResponseBody};

const POLYHAVEN_API: &str = "https://api.polyhaven.com";
const POLYHAVEN_USER_AGENT: &str = "soyel-renderer/0.1 (Poly Haven material discovery)";
const PREVIEW_WIDTH: u32 = 360;
const PREVIEW_HEIGHT: u32 = 260;
const PREVIEW_SAMPLES: u32 = 6;
const STREAM_PREVIEW_SAMPLES: u32 = 64;
const MAX_STREAM_PREVIEW_SAMPLES: u32 = 1024;
const STREAM_FRAME_MAGIC: u32 = 0x4652_5953;
const STREAM_FRAME_VERSION: u32 = 1;
const STREAM_FRAME_HEADER_BYTES: usize = 32;
const STREAM_FRAME_FINAL: u32 = 1;

struct AppState {
    renderer: Mutex<RendererSession>,
    polyhaven: reqwest::Client,
    render_dispatcher: RenderDispatcher,
}

impl AppState {
    fn new() -> Self {
        let polyhaven = reqwest::Client::builder()
            .user_agent(POLYHAVEN_USER_AGENT)
            .build()
            .expect("failed to create Poly Haven client");

        Self {
            renderer: Mutex::new(RendererSession::default()),
            polyhaven,
            render_dispatcher: RenderDispatcher::new(),
        }
    }
}

struct RenderDispatcher {
    next_job_id: AtomicU64,
    latest_preview_job_id: Arc<AtomicU64>,
    jobs: mpsc::Sender<RenderJob>,
}

impl RenderDispatcher {
    fn new() -> Self {
        let (jobs, receiver) = mpsc::channel();
        let latest_preview_job_id = Arc::new(AtomicU64::new(0));
        let worker_latest_preview_job_id = Arc::clone(&latest_preview_job_id);

        thread::Builder::new()
            .name("soyel-rt-preview".to_string())
            .spawn(move || render_worker_loop(receiver, worker_latest_preview_job_id))
            .expect("failed to start RT preview render worker");

        Self {
            next_job_id: AtomicU64::new(1),
            latest_preview_job_id,
            jobs,
        }
    }

    fn submit_preview(&self, request: PreviewRenderJob) -> Result<u64, String> {
        let job_id = self.next_job_id.fetch_add(1, Ordering::SeqCst);
        self.latest_preview_job_id.store(job_id, Ordering::SeqCst);
        self.jobs
            .send(RenderJob::Preview { job_id, request })
            .map_err(|error| format!("RT preview worker is unavailable: {error}"))?;
        Ok(job_id)
    }
}

enum RenderJob {
    Preview {
        job_id: u64,
        request: PreviewRenderJob,
    },
}

struct PreviewRenderJob {
    revision: u64,
    width: u32,
    height: u32,
    samples: u32,
    surface_id: String,
    scene: Option<SceneSnapshot>,
    material: Option<PreviewMaterial>,
    camera: Option<PreviewCamera>,
    on_frame: Channel<InvokeResponseBody>,
}

fn render_worker_loop(receiver: mpsc::Receiver<RenderJob>, latest_preview_job_id: Arc<AtomicU64>) {
    let mut worker = PreviewRenderWorker::default();

    for job in receiver {
        match job {
            RenderJob::Preview { job_id, request } => {
                if latest_preview_job_id.load(Ordering::SeqCst) != job_id {
                    continue;
                }

                let surface_id = request.surface_id.clone();
                let result = panic::catch_unwind(AssertUnwindSafe(|| {
                    worker.render_streaming(request, || {
                        latest_preview_job_id.load(Ordering::SeqCst) != job_id
                    })
                }));

                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => {
                        eprintln!("[soyel] Lupin preview render failed for {surface_id}: {error}");
                    }
                    Err(payload) => {
                        eprintln!(
                            "[soyel] Lupin preview render crashed for {surface_id}: {}",
                            panic_payload_to_string(payload.as_ref())
                        );
                    }
                }
            }
        }
    }
}

#[derive(Default)]
struct PreviewRenderWorker {
    context: Option<PreviewRenderContext>,
    targets: Option<PreviewRenderTargets>,
}

struct PreviewRenderContext {
    device: lupin_pt::wgpu::Device,
    queue: lupin_pt::wgpu::Queue,
    pathtrace_resources: lupin_pt::PathtraceResources,
    tonemap_resources: lupin_pt::TonemapResources,
}

struct PreviewRenderTargets {
    width: u32,
    height: u32,
    output: lupin_pt::DoubleBufferedTexture,
    tonemapped: lupin_pt::wgpu::Texture,
}

impl PreviewRenderWorker {
    fn ensure_context(&mut self) -> Result<(), String> {
        if self.context.is_none() {
            ensure_lupin_preview_supported()?;
            let (device, queue, _) = lupin_pt::init_default_wgpu_context_no_window();
            let pathtrace_resources = lupin_pt::build_pathtrace_resources(
                &device,
                &lupin_pt::BakedPathtraceParams {
                    with_runtime_checks: false,
                    max_bounces: 4,
                    samples_per_pixel: 1,
                },
            );
            let tonemap_resources = lupin_pt::build_tonemap_resources(&device);

            self.context = Some(PreviewRenderContext {
                device,
                queue,
                pathtrace_resources,
                tonemap_resources,
            });
        }

        Ok(())
    }

    fn ensure_targets(&mut self, width: u32, height: u32) -> Result<(), String> {
        self.ensure_context()?;
        let needs_recreate = self
            .targets
            .as_ref()
            .map(|targets| targets.width != width || targets.height != height)
            .unwrap_or(true);

        if needs_recreate {
            let targets = {
                let context = self
                    .context
                    .as_ref()
                    .expect("preview render context initialized");
                PreviewRenderTargets::new(&context.device, width, height)
            };
            self.targets = Some(targets);
        }

        Ok(())
    }

    fn render_streaming(
        &mut self,
        request: PreviewRenderJob,
        mut should_cancel: impl FnMut() -> bool,
    ) -> Result<(), String> {
        self.ensure_context()?;
        self.ensure_targets(request.width, request.height)?;

        let context = self
            .context
            .as_ref()
            .expect("preview render context initialized");
        let (scene, camera_params, camera_transform) = build_soyel_preview_scene(
            &context.device,
            &context.queue,
            request.scene.as_ref(),
            Some(request.surface_id.as_str()),
            request.material.as_ref(),
            request.width as f32 / request.height as f32,
            request.camera.as_ref(),
        )?;
        let targets = self
            .targets
            .as_mut()
            .expect("preview render targets initialized");

        for accum_counter in 0..request.samples {
            if should_cancel() {
                return Ok(());
            }

            lupin_pt::pathtrace_scene(
                &context.device,
                &context.queue,
                &context.pathtrace_resources,
                &scene,
                targets.output.front(),
                Default::default(),
                &lupin_pt::PathtraceDesc {
                    accum_params: Some(lupin_pt::AccumulationParams {
                        prev_frame: targets.output.back(),
                        accum_counter,
                    }),
                    tile_params: None,
                    camera_params,
                    camera_transform,
                    force_software_bvh: true,
                    advanced: lupin_pt::AdvancedParams {
                        max_radiance: 12.0,
                        ..Default::default()
                    },
                },
            );

            let completed_samples = accum_counter + 1;
            let final_frame = completed_samples == request.samples;
            if should_emit_stream_preview_frame(completed_samples, request.samples) {
                if should_cancel() {
                    return Ok(());
                }

                lupin_pt::tonemap_and_fit_aspect(
                    &context.device,
                    &context.queue,
                    &context.tonemap_resources,
                    targets.output.front(),
                    &targets.tonemapped,
                    &lupin_pt::TonemapDesc {
                        viewport: None,
                        exposure: 0.0,
                        filmic: true,
                        srgb: true,
                        clear: true,
                    },
                );

                let pixels = read_rgba8_texture(
                    &context.device,
                    &context.queue,
                    &targets.tonemapped,
                    request.width,
                    request.height,
                )?;
                if should_cancel() {
                    return Ok(());
                }

                let packet = stream_preview_frame_packet(
                    request.revision,
                    request.width,
                    request.height,
                    completed_samples,
                    request.samples,
                    final_frame,
                    pixels,
                );
                request
                    .on_frame
                    .send(InvokeResponseBody::Raw(packet))
                    .map_err(|error| error.to_string())?;
            }

            targets.output.flip();
        }

        Ok(())
    }
}

impl PreviewRenderTargets {
    fn new(device: &lupin_pt::wgpu::Device, width: u32, height: u32) -> Self {
        use lupin_pt::wgpu;

        let output = lupin_pt::DoubleBufferedTexture::create(
            device,
            &wgpu::TextureDescriptor {
                label: Some("Soyel Lupin streaming preview HDR output"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::STORAGE_BINDING
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            },
        );

        let tonemapped = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Soyel Lupin streaming preview RGBA output"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        Self {
            width,
            height,
            output,
            tonemapped,
        }
    }
}

#[derive(Debug, Default)]
struct RendererSession {
    selected_surface: SurfaceSelection,
    applied_materials: HashMap<String, AppliedMaterial>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RendererStatus {
    engine_name: String,
    engine_version: String,
    gpu_api: String,
    material_model: String,
    selected_surface: SurfaceSelection,
    applied_materials: Vec<AppliedMaterial>,
    notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RenderPreviewFrame {
    revision: u64,
    width: u32,
    height: u32,
    samples: u32,
    surface_label: String,
    material_name: Option<String>,
    pixels: Vec<u8>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreviewCamera {
    position: [f32; 3],
    target: [f32; 3],
    up: [f32; 3],
    fov_degrees: f32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SceneSnapshot {
    revision: u64,
    cameras: HashMap<String, PreviewCamera>,
    active_camera_id: String,
    nodes: HashMap<String, SceneNodeSnapshot>,
    meshes: HashMap<String, MeshAssetSnapshot>,
    materials: HashMap<String, MaterialAssetSnapshot>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SceneNodeSnapshot {
    id: String,
    name: String,
    #[serde(default)]
    parent_id: Option<String>,
    mesh_id: Option<String>,
    material_bindings: HashMap<String, String>,
    transform: SceneTransformSnapshot,
    visible: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SceneTransformSnapshot {
    translation: [f32; 3],
    rotation: [f32; 4],
    scale: [f32; 3],
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MeshAssetSnapshot {
    source: MeshSourceSnapshot,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum MeshSourceSnapshot {
    Procedural {
        primitive: ProceduralMeshPrimitiveSnapshot,
    },
    TriangleMesh {
        geometry: TriangleMeshSnapshot,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TriangleMeshSnapshot {
    positions: Vec<f32>,
    #[allow(dead_code)]
    normals: Option<Vec<f32>>,
    #[allow(dead_code)]
    uvs: Option<Vec<f32>>,
    indices: Vec<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum ProceduralMeshPrimitiveSnapshot {
    Plane { size: [f32; 2] },
    Box { size: [f32; 3] },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MaterialAssetSnapshot {
    base_color: [f32; 4],
    roughness: f32,
    metallic: f32,
    emissive: [f32; 3],
}

impl RendererStatus {
    fn from_session(session: &RendererSession) -> Self {
        let lupin_material = lupin_pt::Material::default();
        let default_roughness = lupin_material.roughness;

        let mut applied_materials = session
            .applied_materials
            .values()
            .cloned()
            .collect::<Vec<_>>();
        applied_materials.sort_by(|a, b| a.surface_label.cmp(&b.surface_label));

        Self {
            engine_name: "LupinPathTracer".to_string(),
            engine_version: env!("CARGO_PKG_VERSION").to_string(),
            gpu_api: "wgpu path tracing".to_string(),
            material_model: "Lupin Yocto/GL material fields with glTF-PBR compatibility".to_string(),
            selected_surface: session.selected_surface.clone(),
            applied_materials,
            notes: vec![
                format!("Lupin material defaults loaded; baseline roughness {default_roughness:.2}"),
                "Poly Haven textures are mapped to Lupin color, roughness, normal, metallic, and displacement roles."
                    .to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SurfaceSelection {
    id: String,
    label: String,
}

impl Default for SurfaceSelection {
    fn default() -> Self {
        Self {
            id: "sample-block".to_string(),
            label: "Sample Block".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct MaterialApplication {
    id: String,
    name: String,
    thumbnail_url: Option<String>,
    categories: Vec<String>,
    authors: Vec<String>,
    maps: Vec<MaterialFile>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppliedMaterial {
    surface_id: String,
    surface_label: String,
    material_id: String,
    material_name: String,
    thumbnail_url: Option<String>,
    categories: Vec<String>,
    authors: Vec<String>,
    maps: Vec<MaterialFile>,
}

#[derive(Debug, Clone)]
struct PreviewMaterial {
    name: String,
    categories: Vec<String>,
    maps: Vec<MaterialFile>,
}

impl From<&AppliedMaterial> for PreviewMaterial {
    fn from(material: &AppliedMaterial) -> Self {
        Self {
            name: material.material_name.clone(),
            categories: material.categories.clone(),
            maps: material.maps.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct PolyHavenAsset {
    name: String,
    #[serde(rename = "type")]
    asset_type: u8,
    date_published: Option<i64>,
    download_count: Option<u64>,
    authors: Option<HashMap<String, String>>,
    categories: Option<Vec<String>>,
    tags: Option<Vec<String>>,
    max_resolution: Option<Vec<u32>>,
    dimensions: Option<Vec<u32>>,
    thumbnail_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PolyHavenMaterial {
    id: String,
    name: String,
    categories: Vec<String>,
    tags: Vec<String>,
    authors: Vec<String>,
    max_resolution: Option<Vec<u32>>,
    dimensions_mm: Option<Vec<u32>>,
    thumbnail_url: Option<String>,
    download_count: u64,
    date_published: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PolyHavenCategory {
    id: String,
    name: String,
    count: u64,
}

impl PolyHavenMaterial {
    fn from_asset(id: String, asset: PolyHavenAsset) -> Option<Self> {
        if asset.asset_type != 1 {
            return None;
        }

        Some(Self {
            id,
            name: asset.name,
            categories: asset.categories.unwrap_or_default(),
            tags: asset.tags.unwrap_or_default(),
            authors: asset
                .authors
                .unwrap_or_default()
                .into_iter()
                .map(|(name, role)| {
                    if role.is_empty() {
                        name
                    } else {
                        format!("{name} ({role})")
                    }
                })
                .collect(),
            max_resolution: asset.max_resolution,
            dimensions_mm: asset.dimensions,
            thumbnail_url: asset.thumbnail_url,
            download_count: asset.download_count.unwrap_or_default(),
            date_published: asset.date_published,
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct MaterialFile {
    map: String,
    role: String,
    resolution: String,
    format: String,
    url: String,
    md5: Option<String>,
    size: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PolyHavenMaterialFiles {
    id: String,
    resolution: String,
    files: Vec<MaterialFile>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadedMaterial {
    id: String,
    resolution: String,
    directory: String,
    files: Vec<DownloadedMaterialFile>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadedMaterialFile {
    role: String,
    map: String,
    format: String,
    local_path: String,
    source_url: String,
    bytes: u64,
}

#[tauri::command]
fn renderer_status(state: tauri::State<'_, AppState>) -> Result<RendererStatus, String> {
    let session = state
        .renderer
        .lock()
        .map_err(|_| "renderer session lock poisoned".to_string())?;

    Ok(RendererStatus::from_session(&session))
}

#[tauri::command]
fn select_scene_surface(
    state: tauri::State<'_, AppState>,
    surface_id: String,
    surface_label: String,
) -> Result<RendererStatus, String> {
    let mut session = state
        .renderer
        .lock()
        .map_err(|_| "renderer session lock poisoned".to_string())?;

    session.selected_surface = SurfaceSelection {
        id: surface_id,
        label: surface_label,
    };

    Ok(RendererStatus::from_session(&session))
}

#[tauri::command]
fn apply_material_to_selection(
    state: tauri::State<'_, AppState>,
    material: MaterialApplication,
) -> Result<RendererStatus, String> {
    let mut session = state
        .renderer
        .lock()
        .map_err(|_| "renderer session lock poisoned".to_string())?;
    let surface = session.selected_surface.clone();

    session.applied_materials.insert(
        surface.id.clone(),
        AppliedMaterial {
            surface_id: surface.id,
            surface_label: surface.label,
            material_id: material.id,
            material_name: material.name,
            thumbnail_url: material.thumbnail_url,
            categories: material.categories,
            authors: material.authors,
            maps: material.maps,
        },
    );

    Ok(RendererStatus::from_session(&session))
}

#[tauri::command]
fn render_preview_frame(
    state: tauri::State<'_, AppState>,
    revision: Option<u64>,
    width: Option<u32>,
    height: Option<u32>,
    samples: Option<u32>,
    camera: Option<PreviewCamera>,
    scene: Option<SceneSnapshot>,
) -> Result<RenderPreviewFrame, String> {
    let (surface_id, surface_label, material) = {
        let session = state
            .renderer
            .lock()
            .map_err(|_| "renderer session lock poisoned".to_string())?;
        let surface = session.selected_surface.clone();
        let applied = session
            .applied_materials
            .get(&surface.id)
            .map(PreviewMaterial::from);

        (surface.id, surface.label, applied)
    };

    let width = width.unwrap_or(PREVIEW_WIDTH).clamp(160, 960);
    let height = height.unwrap_or(PREVIEW_HEIGHT).clamp(120, 720);
    let samples = samples.unwrap_or(PREVIEW_SAMPLES).clamp(1, 24);

    let pixels = match panic::catch_unwind(AssertUnwindSafe(|| {
        render_lupin_preview(
            width,
            height,
            samples,
            scene.as_ref(),
            Some(surface_id.as_str()),
            material.as_ref(),
            camera.as_ref(),
        )
    })) {
        Ok(Ok(pixels)) => pixels,
        Ok(Err(error)) => {
            let message = format!("Lupin preview render failed for {surface_id}: {error}");
            eprintln!("[soyel] {message}");
            return Err(message);
        }
        Err(payload) => {
            let message = format!(
                "Lupin preview render crashed for {surface_id}: {}",
                panic_payload_to_string(payload.as_ref())
            );
            eprintln!("[soyel] {message}");
            return Err(message);
        }
    };
    let material_name = material.map(|material| material.name);

    Ok(RenderPreviewFrame {
        revision: revision.unwrap_or_default(),
        width,
        height,
        samples,
        surface_label,
        material_name,
        pixels,
    })
}

#[tauri::command]
fn stream_preview_frame(
    state: tauri::State<'_, AppState>,
    revision: Option<u64>,
    width: Option<u32>,
    height: Option<u32>,
    samples: Option<u32>,
    camera: Option<PreviewCamera>,
    scene: Option<SceneSnapshot>,
    on_frame: Channel<InvokeResponseBody>,
) -> Result<u64, String> {
    let (surface_id, material) = {
        let session = state
            .renderer
            .lock()
            .map_err(|_| "renderer session lock poisoned".to_string())?;
        let surface = session.selected_surface.clone();
        let applied = session
            .applied_materials
            .get(&surface.id)
            .map(PreviewMaterial::from);

        (surface.id, applied)
    };

    let width = width.unwrap_or(PREVIEW_WIDTH).clamp(160, 960);
    let height = height.unwrap_or(PREVIEW_HEIGHT).clamp(120, 720);
    let samples = samples
        .unwrap_or(STREAM_PREVIEW_SAMPLES)
        .clamp(1, MAX_STREAM_PREVIEW_SAMPLES);
    let revision = revision.unwrap_or_default();

    state.render_dispatcher.submit_preview(PreviewRenderJob {
        revision,
        width,
        height,
        samples,
        surface_id,
        scene,
        material,
        camera,
        on_frame,
    })
}

fn panic_payload_to_string(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        return (*message).to_string();
    }

    if let Some(message) = payload.downcast_ref::<String>() {
        return message.clone();
    }

    "unknown panic".to_string()
}

#[tauri::command]
async fn polyhaven_search_materials(
    state: tauri::State<'_, AppState>,
    query: Option<String>,
    category: Option<String>,
    sort: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<PolyHavenMaterial>, String> {
    let mut request = state
        .polyhaven
        .get(format!("{POLYHAVEN_API}/assets"))
        .query(&[("type", "textures")]);

    if let Some(category) = category
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty() && *v != "all")
    {
        request = request.query(&[("categories", category)]);
    }

    let assets = request
        .send()
        .await
        .map_err(polyhaven_error)?
        .error_for_status()
        .map_err(polyhaven_error)?
        .json::<HashMap<String, PolyHavenAsset>>()
        .await
        .map_err(polyhaven_error)?;

    let needle = query.unwrap_or_default().to_lowercase();
    let mut materials = assets
        .into_iter()
        .filter_map(|(id, asset)| PolyHavenMaterial::from_asset(id, asset))
        .filter(|material| {
            if needle.is_empty() {
                return true;
            }

            material.name.to_lowercase().contains(&needle)
                || material.id.to_lowercase().contains(&needle)
                || material
                    .categories
                    .iter()
                    .chain(material.tags.iter())
                    .any(|value| value.to_lowercase().contains(&needle))
        })
        .collect::<Vec<_>>();

    match sort.as_deref().unwrap_or("popular") {
        "latest" => materials.sort_by(|a, b| b.date_published.cmp(&a.date_published)),
        "name" => materials.sort_by(|a, b| a.name.cmp(&b.name)),
        _ => materials.sort_by(|a, b| b.download_count.cmp(&a.download_count)),
    }

    materials.truncate(limit.unwrap_or(48).clamp(1, 200));
    Ok(materials)
}

#[tauri::command]
async fn polyhaven_texture_categories(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<PolyHavenCategory>, String> {
    let categories = state
        .polyhaven
        .get(format!("{POLYHAVEN_API}/categories/textures"))
        .send()
        .await
        .map_err(polyhaven_error)?
        .error_for_status()
        .map_err(polyhaven_error)?
        .json::<HashMap<String, u64>>()
        .await
        .map_err(polyhaven_error)?;

    Ok(texture_categories_from_counts(categories))
}

fn render_lupin_preview(
    width: u32,
    height: u32,
    samples: u32,
    scene_snapshot: Option<&SceneSnapshot>,
    material_override_surface_id: Option<&str>,
    material: Option<&PreviewMaterial>,
    camera: Option<&PreviewCamera>,
) -> Result<Vec<u8>, String> {
    use lupin_pt::wgpu;

    ensure_lupin_preview_supported()?;

    let (device, queue, _) = lupin_pt::init_default_wgpu_context_no_window();
    let pathtrace_resources = lupin_pt::build_pathtrace_resources(
        &device,
        &lupin_pt::BakedPathtraceParams {
            with_runtime_checks: false,
            max_bounces: 4,
            samples_per_pixel: 1,
        },
    );
    let tonemap_resources = lupin_pt::build_tonemap_resources(&device);
    let (scene, camera_params, camera_transform) = build_soyel_preview_scene(
        &device,
        &queue,
        scene_snapshot,
        material_override_surface_id,
        material,
        width as f32 / height as f32,
        camera,
    )?;

    let mut output = lupin_pt::DoubleBufferedTexture::create(
        &device,
        &wgpu::TextureDescriptor {
            label: Some("Soyel Lupin preview HDR output"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        },
    );

    for accum_counter in 0..samples {
        lupin_pt::pathtrace_scene(
            &device,
            &queue,
            &pathtrace_resources,
            &scene,
            output.front(),
            Default::default(),
            &lupin_pt::PathtraceDesc {
                accum_params: Some(lupin_pt::AccumulationParams {
                    prev_frame: output.back(),
                    accum_counter,
                }),
                tile_params: None,
                camera_params,
                camera_transform,
                force_software_bvh: true,
                advanced: lupin_pt::AdvancedParams {
                    max_radiance: 12.0,
                    ..Default::default()
                },
            },
        );
        output.flip();
    }
    output.flip();

    let tonemapped = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Soyel Lupin preview RGBA output"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });

    lupin_pt::tonemap_and_fit_aspect(
        &device,
        &queue,
        &tonemap_resources,
        output.front(),
        &tonemapped,
        &lupin_pt::TonemapDesc {
            viewport: None,
            exposure: 0.0,
            filmic: true,
            srgb: true,
            clear: true,
        },
    );

    read_rgba8_texture(&device, &queue, &tonemapped, width, height)
}

fn should_emit_stream_preview_frame(completed_samples: u32, total_samples: u32) -> bool {
    completed_samples == total_samples || completed_samples.is_power_of_two()
}

fn stream_preview_frame_packet(
    revision: u64,
    width: u32,
    height: u32,
    completed_samples: u32,
    total_samples: u32,
    final_frame: bool,
    pixels: Vec<u8>,
) -> Vec<u8> {
    let mut packet = Vec::with_capacity(STREAM_FRAME_HEADER_BYTES + pixels.len());
    for value in [
        STREAM_FRAME_MAGIC,
        STREAM_FRAME_VERSION,
        revision.min(u32::MAX as u64) as u32,
        width,
        height,
        completed_samples,
        total_samples,
        if final_frame { STREAM_FRAME_FINAL } else { 0 },
    ] {
        packet.extend_from_slice(&value.to_le_bytes());
    }
    packet.extend_from_slice(&pixels);
    packet
}

fn ensure_lupin_preview_supported() -> Result<(), String> {
    use lupin_pt::wgpu;

    let instance = wgpu::Instance::new(&default_wgpu_instance_descriptor());
    let adapter_options = wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::default(),
        compatible_surface: None,
        force_fallback_adapter: false,
    };
    let adapter = lupin_pt::wait_for(instance.request_adapter(&adapter_options))
        .map_err(|error| format!("failed to get WGPU adapter: {error}"))?;
    if lupin_pt::supports_buffer_binding_arrays(&adapter)
        || lupin_pt::supports_packed_wgpu_path(&adapter)
    {
        return Ok(());
    }

    Err(format!(
        "WGPU adapter {:?} supports neither Lupin's legacy buffer-binding-array path nor its packed software-BVH path",
        adapter.get_info().name
    ))
}

fn default_wgpu_instance_descriptor() -> lupin_pt::wgpu::InstanceDescriptor {
    let mut desc = lupin_pt::wgpu::InstanceDescriptor::default();

    #[cfg(target_os = "windows")]
    {
        desc.backends = lupin_pt::wgpu::Backends::VULKAN;
    }

    #[cfg(all(not(target_os = "windows"), not(target_arch = "wasm32")))]
    {
        desc.backends = lupin_pt::wgpu::Backends::PRIMARY;
    }

    #[cfg(target_arch = "wasm32")]
    {
        desc.backends = lupin_pt::wgpu::Backends::GL;
    }

    desc
}

fn build_soyel_preview_scene(
    device: &lupin_pt::wgpu::Device,
    queue: &lupin_pt::wgpu::Queue,
    scene_snapshot: Option<&SceneSnapshot>,
    material_override_surface_id: Option<&str>,
    material: Option<&PreviewMaterial>,
    aspect: f32,
    camera: Option<&PreviewCamera>,
) -> Result<(lupin_pt::Scene, lupin_pt::CameraParams, lupin_pt::Mat3x4), String> {
    let fallback_scene;
    let snapshot = match scene_snapshot {
        Some(scene) => scene,
        None => {
            fallback_scene = default_cornell_scene_snapshot();
            &fallback_scene
        }
    };
    let scene = scene_cpu_from_snapshot(snapshot, material_override_surface_id, material)?;
    lupin_pt::validate_scene(&scene, 0, 0);
    let gpu_scene = lupin_pt::build_accel_structures_and_upload(
        device,
        queue,
        &scene,
        vec![],
        vec![],
        vec![],
        &[],
        true,
    );

    let scene_camera = camera.or_else(|| active_snapshot_camera(snapshot));
    let (camera_params, camera_transform) = preview_camera_for_request(aspect, scene_camera);

    Ok((gpu_scene, camera_params, camera_transform))
}

fn scene_cpu_from_snapshot(
    snapshot: &SceneSnapshot,
    material_override_surface_id: Option<&str>,
    material_override: Option<&PreviewMaterial>,
) -> Result<lupin_pt::SceneCPU, String> {
    let mut scene = lupin_pt::SceneCPU::default();
    let mut material_indices = HashMap::new();
    let mut node_ids = snapshot.nodes.keys().cloned().collect::<Vec<_>>();
    node_ids.sort();

    for node_id in node_ids {
        let node = &snapshot.nodes[&node_id];
        if !is_snapshot_node_effectively_visible(snapshot, node) {
            continue;
        }

        let Some(mesh_id) = node.mesh_id.as_deref() else {
            continue;
        };
        let mesh = snapshot.meshes.get(mesh_id).ok_or_else(|| {
            format!(
                "scene node {} ({}) references missing mesh {mesh_id}",
                node.id, node.name
            )
        })?;
        let material_index = material_index_for_node(
            &mut scene,
            &mut material_indices,
            snapshot,
            node,
            material_override_surface_id,
            material_override,
        )?;

        match &mesh.source {
            MeshSourceSnapshot::Procedural {
                primitive: ProceduralMeshPrimitiveSnapshot::Plane { size },
            } => {
                push_snapshot_plane(&mut scene, *size, &node.transform, material_index);
            }
            MeshSourceSnapshot::Procedural {
                primitive: ProceduralMeshPrimitiveSnapshot::Box { size },
            } => {
                push_snapshot_box(&mut scene, *size, &node.transform, material_index);
            }
            MeshSourceSnapshot::TriangleMesh { geometry } => {
                push_snapshot_triangle_mesh(&mut scene, geometry, &node.transform, material_index)?;
            }
        }
    }

    if scene.instances.is_empty() {
        return Err(format!(
            "scene snapshot revision {} contains no visible renderable nodes",
            snapshot.revision
        ));
    }

    Ok(scene)
}

fn is_snapshot_node_effectively_visible(
    snapshot: &SceneSnapshot,
    node: &SceneNodeSnapshot,
) -> bool {
    let mut current = Some(node);
    let mut visited = BTreeSet::new();

    while let Some(node) = current {
        if !node.visible {
            return false;
        }

        let Some(parent_id) = node.parent_id.as_deref() else {
            return true;
        };

        if !visited.insert(node.id.as_str()) {
            return false;
        }

        current = snapshot.nodes.get(parent_id);
    }

    true
}

fn material_index_for_node(
    scene: &mut lupin_pt::SceneCPU,
    material_indices: &mut HashMap<String, u32>,
    snapshot: &SceneSnapshot,
    node: &SceneNodeSnapshot,
    material_override_surface_id: Option<&str>,
    material_override: Option<&PreviewMaterial>,
) -> Result<u32, String> {
    if material_override_surface_id == Some(node.id.as_str()) {
        if let Some(material) = material_override {
            let key = format!("override:{}", node.id);
            return Ok(*material_indices.entry(key).or_insert_with(|| {
                push_preview_material(scene, preview_material_from_polyhaven(Some(material)))
            }));
        }
    }

    let Some(material_id) = node.material_bindings.get("default") else {
        let key = "default:fallback".to_string();
        return Ok(*material_indices.entry(key).or_insert_with(|| {
            push_preview_material(scene, preview_material(0.68, 0.73, 0.70, 0.72, 0.0))
        }));
    };

    if let Some(index) = material_indices.get(material_id) {
        return Ok(*index);
    }

    let material = snapshot.materials.get(material_id).ok_or_else(|| {
        format!(
            "scene node {} ({}) references missing material {material_id}",
            node.id, node.name
        )
    })?;
    let index = push_preview_material(scene, preview_material_from_asset(material));
    material_indices.insert(material_id.clone(), index);
    Ok(index)
}

fn push_snapshot_plane(
    scene: &mut lupin_pt::SceneCPU,
    size: [f32; 2],
    transform: &SceneTransformSnapshot,
    mat_idx: u32,
) {
    let half_width = size[0] * 0.5;
    let half_height = size[1] * 0.5;
    push_preview_quad(
        scene,
        [
            transform_point(transform, [-half_width, -half_height, 0.0]),
            transform_point(transform, [half_width, -half_height, 0.0]),
            transform_point(transform, [half_width, half_height, 0.0]),
            transform_point(transform, [-half_width, half_height, 0.0]),
        ],
        mat_idx,
    );
}

fn push_snapshot_box(
    scene: &mut lupin_pt::SceneCPU,
    size: [f32; 3],
    transform: &SceneTransformSnapshot,
    mat_idx: u32,
) {
    let half = [size[0] * 0.5, size[1] * 0.5, size[2] * 0.5];
    let x0 = -half[0];
    let x1 = half[0];
    let y0 = -half[1];
    let y1 = half[1];
    let z0 = -half[2];
    let z1 = half[2];
    let vertex = |x, y, z| transform_point(transform, [x, y, z]);

    push_preview_quad(
        scene,
        [
            vertex(x0, y0, z0),
            vertex(x1, y0, z0),
            vertex(x1, y1, z0),
            vertex(x0, y1, z0),
        ],
        mat_idx,
    );
    push_preview_quad(
        scene,
        [
            vertex(x0, y0, z1),
            vertex(x1, y0, z1),
            vertex(x1, y1, z1),
            vertex(x0, y1, z1),
        ],
        mat_idx,
    );
    push_preview_quad(
        scene,
        [
            vertex(x0, y0, z0),
            vertex(x0, y0, z1),
            vertex(x0, y1, z1),
            vertex(x0, y1, z0),
        ],
        mat_idx,
    );
    push_preview_quad(
        scene,
        [
            vertex(x1, y0, z0),
            vertex(x1, y0, z1),
            vertex(x1, y1, z1),
            vertex(x1, y1, z0),
        ],
        mat_idx,
    );
    push_preview_quad(
        scene,
        [
            vertex(x0, y1, z0),
            vertex(x1, y1, z0),
            vertex(x1, y1, z1),
            vertex(x0, y1, z1),
        ],
        mat_idx,
    );
    push_preview_quad(
        scene,
        [
            vertex(x0, y0, z0),
            vertex(x1, y0, z0),
            vertex(x1, y0, z1),
            vertex(x0, y0, z1),
        ],
        mat_idx,
    );
}

fn push_snapshot_triangle_mesh(
    scene: &mut lupin_pt::SceneCPU,
    geometry: &TriangleMeshSnapshot,
    transform: &SceneTransformSnapshot,
    mat_idx: u32,
) -> Result<(), String> {
    if geometry.positions.len() % 3 != 0 {
        return Err(format!(
            "triangle mesh positions length {} is not divisible by 3",
            geometry.positions.len()
        ));
    }

    let vertex_count = geometry.positions.len() / 3;
    if geometry.indices.len() < 3 || geometry.indices.len() % 3 != 0 {
        return Err(format!(
            "triangle mesh index length {} does not describe whole triangles",
            geometry.indices.len()
        ));
    }

    for index in &geometry.indices {
        if *index as usize >= vertex_count {
            return Err(format!(
                "triangle mesh index {index} references only {vertex_count} vertices"
            ));
        }
    }

    let verts = geometry
        .positions
        .chunks_exact(3)
        .map(|point| transform_point(transform, [point[0], point[1], point[2]]))
        .collect::<Vec<_>>();
    let mesh_idx = scene.mesh_infos.len() as u32;
    scene.mesh_infos.push(lupin_pt::MeshInfo::default());
    scene.verts_pos_array.push(verts);
    scene.indices_array.push(geometry.indices.clone());
    scene.instances.push(lupin_pt::Instance {
        mesh_idx,
        mat_idx,
        ..Default::default()
    });

    Ok(())
}

fn transform_point(transform: &SceneTransformSnapshot, point: [f32; 3]) -> lupin_pt::Vec4 {
    let scaled = [
        point[0] * transform.scale[0],
        point[1] * transform.scale[1],
        point[2] * transform.scale[2],
    ];
    let rotated = rotate_by_quat(scaled, transform.rotation);
    lupin_pt::Vec4::new3(
        rotated[0] + transform.translation[0],
        rotated[1] + transform.translation[1],
        rotated[2] + transform.translation[2],
    )
}

fn rotate_by_quat(point: [f32; 3], rotation: [f32; 4]) -> [f32; 3] {
    let length = (rotation[0] * rotation[0]
        + rotation[1] * rotation[1]
        + rotation[2] * rotation[2]
        + rotation[3] * rotation[3])
        .sqrt();
    if length < 0.001 || !length.is_finite() {
        return point;
    }

    let q = [
        rotation[0] / length,
        rotation[1] / length,
        rotation[2] / length,
        rotation[3] / length,
    ];
    let qv = [q[0], q[1], q[2]];
    let uv = cross3(qv, point);
    let uuv = cross3(qv, uv);

    [
        point[0] + 2.0 * (q[3] * uv[0] + uuv[0]),
        point[1] + 2.0 * (q[3] * uv[1] + uuv[1]),
        point[2] + 2.0 * (q[3] * uv[2] + uuv[2]),
    ]
}

fn active_snapshot_camera(snapshot: &SceneSnapshot) -> Option<&PreviewCamera> {
    snapshot
        .cameras
        .get(&snapshot.active_camera_id)
        .or_else(|| snapshot.cameras.values().next())
}

fn preview_camera_for_request(
    aspect: f32,
    camera: Option<&PreviewCamera>,
) -> (lupin_pt::CameraParams, lupin_pt::Mat3x4) {
    let fallback = || {
        let camera_params = lupin_pt::CameraParams {
            is_orthographic: false,
            lens: lens_from_vertical_fov(0.032, aspect, 42.0),
            aperture: 0.0,
            focus: 3.2,
            film: 0.032,
            aspect,
        };
        let camera_transform = lupin_pt::Mat3x4 {
            m: [
                [-1.0, 0.0, 0.0],
                [0.0, 0.96, 0.28],
                [0.0, -0.28, 0.96],
                [0.0, 0.92, -3.05],
            ],
        };

        (camera_params, camera_transform)
    };

    let Some(camera) = camera else {
        return fallback();
    };

    let direction = sub3(camera.target, camera.position);
    let focus = length3(direction);
    if focus < 0.001 || !focus.is_finite() {
        return fallback();
    }

    let forward = normalize3(direction);
    let right = normalize3(cross3(forward, camera.up));
    if length3(right) < 0.001 {
        return fallback();
    }

    let true_up = normalize3(cross3(right, forward));
    let fov_degrees = if camera.fov_degrees.is_finite() {
        camera.fov_degrees.clamp(18.0, 80.0)
    } else {
        42.0
    };
    let film = 0.032;
    let lens = lens_from_vertical_fov(film, aspect, fov_degrees);

    let camera_params = lupin_pt::CameraParams {
        is_orthographic: false,
        lens,
        aperture: 0.0,
        focus,
        film,
        aspect,
    };
    let camera_transform = lupin_pt::Mat3x4 {
        m: [right, true_up, forward, camera.position],
    };

    (camera_params, camera_transform)
}

fn lens_from_vertical_fov(film: f32, aspect: f32, fov_degrees: f32) -> f32 {
    let safe_aspect = if aspect.is_finite() && aspect > 0.001 {
        aspect
    } else {
        1.0
    };
    let vertical_film = if safe_aspect >= 1.0 {
        film / safe_aspect
    } else {
        film
    };

    vertical_film / (2.0 * (fov_degrees.to_radians() * 0.5).tan())
}

fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn length3(v: [f32; 3]) -> f32 {
    dot3(v, v).sqrt()
}

fn normalize3(v: [f32; 3]) -> [f32; 3] {
    let length = length3(v);
    if length < 0.001 || !length.is_finite() {
        return [0.0, 0.0, 0.0];
    }

    [v[0] / length, v[1] / length, v[2] / length]
}

fn push_preview_material(scene: &mut lupin_pt::SceneCPU, material: lupin_pt::Material) -> u32 {
    let index = scene.materials.len() as u32;
    scene.materials.push(material);
    index
}

fn push_preview_quad(scene: &mut lupin_pt::SceneCPU, verts: [lupin_pt::Vec4; 4], mat_idx: u32) {
    let mesh_idx = scene.mesh_infos.len() as u32;
    scene.mesh_infos.push(lupin_pt::MeshInfo::default());
    scene.verts_pos_array.push(verts.to_vec());
    scene.indices_array.push(vec![0, 1, 2, 2, 3, 0]);
    scene.instances.push(lupin_pt::Instance {
        mesh_idx,
        mat_idx,
        ..Default::default()
    });
}

fn preview_material(r: f32, g: f32, b: f32, roughness: f32, metallic: f32) -> lupin_pt::Material {
    let mut material = lupin_pt::Material::default();
    material.color = lupin_pt::Vec4 {
        x: r,
        y: g,
        z: b,
        w: 1.0,
    };
    material.mat_type = lupin_pt::MaterialType::GltfPbr;
    material.roughness = roughness;
    material.metallic = metallic;
    material
}

fn preview_material_from_asset(material: &MaterialAssetSnapshot) -> lupin_pt::Material {
    let mut lupin_material = preview_material(
        material.base_color[0].clamp(0.0, 1.0),
        material.base_color[1].clamp(0.0, 1.0),
        material.base_color[2].clamp(0.0, 1.0),
        material.roughness.clamp(0.02, 1.0),
        material.metallic.clamp(0.0, 1.0),
    );
    lupin_material.emission = lupin_pt::Vec4 {
        x: material.emissive[0].max(0.0),
        y: material.emissive[1].max(0.0),
        z: material.emissive[2].max(0.0),
        w: 0.0,
    };
    lupin_material
}

fn preview_material_from_polyhaven(material: Option<&PreviewMaterial>) -> lupin_pt::Material {
    let accent = material_preview_color(material.map(|material| material.name.as_str()));
    let roughness = material.map(polyhaven_preview_roughness).unwrap_or(0.46);
    let metallic = material.map(polyhaven_preview_metallic).unwrap_or(0.08);

    preview_material(accent.x, accent.y, accent.z, roughness, metallic)
}

fn default_cornell_scene_snapshot() -> SceneSnapshot {
    let camera = PreviewCamera {
        position: [0.0, 0.95, -3.35],
        target: [0.0, 0.82, 0.1],
        up: [0.0, 1.0, 0.0],
        fov_degrees: 42.0,
    };

    SceneSnapshot {
        revision: 1,
        cameras: HashMap::from([("camera-main".to_string(), camera)]),
        active_camera_id: "camera-main".to_string(),
        meshes: HashMap::from([
            ("cornellFloor".to_string(), plane_snapshot([2.7, 2.3])),
            ("cornellCeiling".to_string(), plane_snapshot([2.7, 2.3])),
            ("cornellBackWall".to_string(), plane_snapshot([2.7, 1.9])),
            ("cornellLeftWall".to_string(), plane_snapshot([2.3, 1.9])),
            ("cornellRightWall".to_string(), plane_snapshot([2.3, 1.9])),
            ("light".to_string(), plane_snapshot([0.68, 0.56])),
            ("shortBlock".to_string(), box_snapshot([0.62, 0.55, 0.62])),
            ("tallBlock".to_string(), box_snapshot([0.56, 1.1, 0.56])),
        ]),
        materials: HashMap::from([
            (
                "floorMat".to_string(),
                material_snapshot([0.66, 0.65, 0.59, 1.0], 0.82, 0.0, [0.0, 0.0, 0.0]),
            ),
            (
                "ceilingMat".to_string(),
                material_snapshot([0.72, 0.71, 0.66, 1.0], 0.82, 0.0, [0.0, 0.0, 0.0]),
            ),
            (
                "backWallMat".to_string(),
                material_snapshot([0.70, 0.69, 0.64, 1.0], 0.82, 0.0, [0.0, 0.0, 0.0]),
            ),
            (
                "leftWallMat".to_string(),
                material_snapshot([0.62, 0.22, 0.18, 1.0], 0.82, 0.0, [0.0, 0.0, 0.0]),
            ),
            (
                "rightWallMat".to_string(),
                material_snapshot([0.22, 0.52, 0.32, 1.0], 0.82, 0.0, [0.0, 0.0, 0.0]),
            ),
            (
                "sampleBlockMat".to_string(),
                material_snapshot([0.68, 0.73, 0.70, 1.0], 0.46, 0.08, [0.0, 0.0, 0.0]),
            ),
            (
                "tallBlockMat".to_string(),
                material_snapshot([0.48, 0.50, 0.47, 1.0], 0.74, 0.0, [0.0, 0.0, 0.0]),
            ),
            (
                "lightMat".to_string(),
                material_snapshot([1.0, 0.88, 0.58, 1.0], 0.28, 0.0, [20.0, 17.0, 12.0]),
            ),
        ]),
        nodes: HashMap::from([
            (
                "sample-block".to_string(),
                node_snapshot(
                    "sample-block",
                    "Sample Block",
                    "shortBlock",
                    "sampleBlockMat",
                    [0.48, 0.275, -0.25],
                    quat_from_euler(0.0, -0.28, 0.0),
                ),
            ),
            (
                "tall-block".to_string(),
                node_snapshot(
                    "tall-block",
                    "Tall Block",
                    "tallBlock",
                    "tallBlockMat",
                    [-0.43, 0.55, 0.28],
                    quat_from_euler(0.0, 0.32, 0.0),
                ),
            ),
            (
                "left-wall".to_string(),
                node_snapshot(
                    "left-wall",
                    "Left Wall",
                    "cornellLeftWall",
                    "leftWallMat",
                    [-1.35, 0.95, 0.0],
                    quat_from_euler(0.0, std::f32::consts::FRAC_PI_2, 0.0),
                ),
            ),
            (
                "right-wall".to_string(),
                node_snapshot(
                    "right-wall",
                    "Right Wall",
                    "cornellRightWall",
                    "rightWallMat",
                    [1.35, 0.95, 0.0],
                    quat_from_euler(0.0, -std::f32::consts::FRAC_PI_2, 0.0),
                ),
            ),
            (
                "back-wall".to_string(),
                node_snapshot(
                    "back-wall",
                    "Back Wall",
                    "cornellBackWall",
                    "backWallMat",
                    [0.0, 0.95, 1.15],
                    [0.0, 0.0, 0.0, 1.0],
                ),
            ),
            (
                "floor".to_string(),
                node_snapshot(
                    "floor",
                    "Floor",
                    "cornellFloor",
                    "floorMat",
                    [0.0, 0.0, 0.0],
                    quat_from_euler(-std::f32::consts::FRAC_PI_2, 0.0, 0.0),
                ),
            ),
            (
                "ceiling".to_string(),
                node_snapshot(
                    "ceiling",
                    "Ceiling",
                    "cornellCeiling",
                    "ceilingMat",
                    [0.0, 1.9, 0.0],
                    quat_from_euler(std::f32::consts::FRAC_PI_2, 0.0, 0.0),
                ),
            ),
            (
                "area-light".to_string(),
                node_snapshot(
                    "area-light",
                    "Area Light",
                    "light",
                    "lightMat",
                    [0.0, 1.88, -0.12],
                    quat_from_euler(std::f32::consts::FRAC_PI_2, 0.0, 0.0),
                ),
            ),
        ]),
    }
}

fn plane_snapshot(size: [f32; 2]) -> MeshAssetSnapshot {
    MeshAssetSnapshot {
        source: MeshSourceSnapshot::Procedural {
            primitive: ProceduralMeshPrimitiveSnapshot::Plane { size },
        },
    }
}

fn box_snapshot(size: [f32; 3]) -> MeshAssetSnapshot {
    MeshAssetSnapshot {
        source: MeshSourceSnapshot::Procedural {
            primitive: ProceduralMeshPrimitiveSnapshot::Box { size },
        },
    }
}

fn material_snapshot(
    base_color: [f32; 4],
    roughness: f32,
    metallic: f32,
    emissive: [f32; 3],
) -> MaterialAssetSnapshot {
    MaterialAssetSnapshot {
        base_color,
        roughness,
        metallic,
        emissive,
    }
}

fn node_snapshot(
    id: &str,
    name: &str,
    mesh_id: &str,
    material_id: &str,
    translation: [f32; 3],
    rotation: [f32; 4],
) -> SceneNodeSnapshot {
    SceneNodeSnapshot {
        id: id.to_string(),
        name: name.to_string(),
        parent_id: None,
        mesh_id: Some(mesh_id.to_string()),
        material_bindings: HashMap::from([("default".to_string(), material_id.to_string())]),
        transform: SceneTransformSnapshot {
            translation,
            rotation,
            scale: [1.0, 1.0, 1.0],
        },
        visible: true,
    }
}

fn quat_from_euler(x: f32, y: f32, z: f32) -> [f32; 4] {
    let (sx, cx) = (x * 0.5).sin_cos();
    let (sy, cy) = (y * 0.5).sin_cos();
    let (sz, cz) = (z * 0.5).sin_cos();

    [
        sx * cy * cz + cx * sy * sz,
        cx * sy * cz - sx * cy * sz,
        cx * cy * sz + sx * sy * cz,
        cx * cy * cz - sx * sy * sz,
    ]
}

fn polyhaven_preview_roughness(material: &PreviewMaterial) -> f32 {
    let mut roughness = if material_has_category(material, "fabric")
        || material_has_category(material, "brick")
        || material_has_category(material, "rock")
        || material_has_category(material, "terrain")
    {
        0.82
    } else if material_has_category(material, "wood") {
        0.62
    } else if material_has_category(material, "metal") {
        0.34
    } else {
        0.56
    };

    if material_has_role(material, "normal") || material_has_role(material, "displacement") {
        roughness = (roughness + 0.08_f32).min(0.92_f32);
    }

    if material_has_role(material, "roughness")
        || material_has_role(material, "occlusionRoughnessMetallic")
    {
        roughness
    } else {
        (roughness + 0.46) * 0.5
    }
}

fn polyhaven_preview_metallic(material: &PreviewMaterial) -> f32 {
    if material_has_role(material, "metallic") || material_has_category(material, "metal") {
        0.78
    } else {
        0.04
    }
}

fn material_has_role(material: &PreviewMaterial, role: &str) -> bool {
    material.maps.iter().any(|map| map.role == role)
}

fn material_has_category(material: &PreviewMaterial, category: &str) -> bool {
    material
        .categories
        .iter()
        .any(|value| value.to_lowercase().contains(category))
}

fn material_preview_color(material_name: Option<&str>) -> lupin_pt::Vec4 {
    let Some(name) = material_name.filter(|value| !value.is_empty()) else {
        return lupin_pt::Vec4 {
            x: 0.68,
            y: 0.73,
            z: 0.70,
            w: 1.0,
        };
    };

    let mut hash = 0u32;
    for byte in name.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u32);
    }

    let red = 0.38 + ((hash & 0xff) as f32 / 255.0) * 0.34;
    let green = 0.34 + (((hash >> 8) & 0xff) as f32 / 255.0) * 0.34;
    let blue = 0.32 + (((hash >> 16) & 0xff) as f32 / 255.0) * 0.34;

    lupin_pt::Vec4 {
        x: red,
        y: green,
        z: blue,
        w: 1.0,
    }
}

fn read_rgba8_texture(
    device: &lupin_pt::wgpu::Device,
    queue: &lupin_pt::wgpu::Queue,
    texture: &lupin_pt::wgpu::Texture,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, String> {
    use lupin_pt::wgpu;

    let bytes_per_pixel = 4usize;
    let unpadded_bytes_per_row = bytes_per_pixel * width as usize;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
    let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;
    let buffer_size = padded_bytes_per_row as u64 * height as u64;

    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Soyel Lupin preview readback"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Soyel Lupin preview readback encoder"),
    });
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row as u32),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(encoder.finish()));

    {
        let buffer_slice = buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|error| format!("GPU readback poll failed: {error}"))?;
        rx.recv()
            .map_err(|error| format!("GPU readback channel failed: {error}"))?
            .map_err(|error| format!("GPU readback mapping failed: {error}"))?;
    }

    let view = buffer.slice(..).get_mapped_range();
    let mut pixels = Vec::with_capacity(unpadded_bytes_per_row * height as usize);
    for row in view.chunks(padded_bytes_per_row).take(height as usize) {
        pixels.extend_from_slice(&row[..unpadded_bytes_per_row]);
    }
    drop(view);
    buffer.unmap();

    Ok(pixels)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn preview_camera_from_frontend_looks_toward_scene() {
        let camera = default_frontend_preview_camera();
        let (_, transform) = preview_camera_for_request(16.0 / 9.0, Some(&camera));

        assert!(
            transform.m[2][2] > 0.9,
            "Lupin camera forward axis should point toward world +Z, got {:?}",
            transform.m[2]
        );
        assert!(
            transform.m[0][0] < -0.9,
            "Lupin camera right axis should match Three's screen-right basis, got {:?}",
            transform.m[0]
        );
        assert!(
            transform.m[1][1] > 0.9,
            "Lupin camera up axis should preserve Three's pitch direction, got {:?}",
            transform.m[1]
        );
    }

    #[test]
    fn preview_camera_preserves_three_vertical_fov_across_aspects() {
        let camera = default_frontend_preview_camera();

        for aspect in [1.0, 16.0 / 9.0, 9.0 / 16.0] {
            let (params, _) = preview_camera_for_request(aspect, Some(&camera));
            let effective_fov = effective_vertical_fov_degrees(&params);

            assert!(
                (effective_fov - camera.fov_degrees).abs() < 0.001,
                "aspect {aspect} produced vertical fov {effective_fov}, expected {}",
                camera.fov_degrees
            );
        }
    }

    #[test]
    fn stream_preview_packet_contains_header_and_pixels() {
        let pixels = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let packet = stream_preview_frame_packet(7, 2, 1, 4, 64, true, pixels.clone());

        assert_eq!(packet.len(), STREAM_FRAME_HEADER_BYTES + pixels.len());
        assert_eq!(
            u32::from_le_bytes(packet[0..4].try_into().unwrap()),
            STREAM_FRAME_MAGIC
        );
        assert_eq!(
            u32::from_le_bytes(packet[4..8].try_into().unwrap()),
            STREAM_FRAME_VERSION
        );
        assert_eq!(u32::from_le_bytes(packet[8..12].try_into().unwrap()), 7);
        assert_eq!(u32::from_le_bytes(packet[12..16].try_into().unwrap()), 2);
        assert_eq!(u32::from_le_bytes(packet[16..20].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(packet[20..24].try_into().unwrap()), 4);
        assert_eq!(u32::from_le_bytes(packet[24..28].try_into().unwrap()), 64);
        assert_eq!(
            u32::from_le_bytes(packet[28..32].try_into().unwrap()),
            STREAM_FRAME_FINAL
        );
        assert_eq!(&packet[STREAM_FRAME_HEADER_BYTES..], pixels.as_slice());
    }

    #[test]
    fn default_scene_snapshot_converts_all_cornell_nodes() {
        let snapshot = default_cornell_scene_snapshot();
        let scene = scene_cpu_from_snapshot(&snapshot, None, None).unwrap();

        assert_eq!(snapshot.nodes.len(), 8);
        assert_eq!(scene.instances.len(), 18);
        assert_eq!(scene.mesh_infos.len(), 18);
        assert_eq!(scene.materials.len(), 8);
        assert!(
            scene
                .materials
                .iter()
                .any(|material| material.emission.x > 10.0 && material.emission.y > 10.0),
            "converted scene should preserve the emissive area-light material"
        );
    }

    #[test]
    fn triangle_mesh_snapshot_converts_to_lupin_mesh() {
        let snapshot = triangle_mesh_scene_snapshot(true);
        let scene = scene_cpu_from_snapshot(&snapshot, None, None).unwrap();

        assert_eq!(scene.instances.len(), 1);
        assert_eq!(scene.mesh_infos.len(), 1);
        assert_eq!(scene.verts_pos_array[0].len(), 3);
        assert_eq!(scene.indices_array[0], vec![0, 1, 2]);
    }

    #[test]
    fn hidden_parent_hides_triangle_mesh_child() {
        let snapshot = triangle_mesh_scene_snapshot(false);
        let error = scene_cpu_from_snapshot(&snapshot, None, None).unwrap_err();

        assert!(
            error.contains("contains no visible renderable nodes"),
            "unexpected error: {error}"
        );
    }

    #[test]
    #[ignore = "requires a supported WGPU adapter and Lupin packed/software-BVH path"]
    fn soyel_preview_scene_renders_nonzero_pixels() -> Result<(), String> {
        let width = 128;
        let height = 96;
        let camera = default_frontend_preview_camera();
        let scene = default_cornell_scene_snapshot();
        let pixels =
            render_lupin_preview(width, height, 4, Some(&scene), None, None, Some(&camera))?;

        assert_eq!(pixels.len(), width as usize * height as usize * 4);

        let stats = rgba8_stats(&pixels);
        assert!(
            stats.non_black_pixels > 64,
            "Soyel Lupin preview scene produced too few visible pixels: {stats:?}"
        );
        assert!(
            stats.max_luma > 8,
            "Soyel Lupin preview scene produced only black or near-black pixels: {stats:?}"
        );
        assert!(
            stats.unique_rgb_count > 2,
            "Soyel Lupin preview scene did not produce varied RGB output: {stats:?}"
        );

        Ok(())
    }

    fn default_frontend_preview_camera() -> PreviewCamera {
        PreviewCamera {
            position: [0.0, 0.95, -3.1],
            target: [0.0, 0.68, 0.05],
            up: [0.0, 1.0, 0.0],
            fov_degrees: 42.0,
        }
    }

    fn triangle_mesh_scene_snapshot(root_visible: bool) -> SceneSnapshot {
        let root = SceneNodeSnapshot {
            id: "root".to_string(),
            name: "Root".to_string(),
            parent_id: None,
            mesh_id: None,
            material_bindings: HashMap::new(),
            transform: SceneTransformSnapshot {
                translation: [0.0, 0.0, 0.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: [1.0, 1.0, 1.0],
            },
            visible: root_visible,
        };
        let child = SceneNodeSnapshot {
            id: "triangle".to_string(),
            name: "Triangle".to_string(),
            parent_id: Some("root".to_string()),
            mesh_id: Some("triangleMesh".to_string()),
            material_bindings: HashMap::from([("default".to_string(), "mat".to_string())]),
            transform: SceneTransformSnapshot {
                translation: [0.0, 0.0, 0.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: [1.0, 1.0, 1.0],
            },
            visible: true,
        };

        SceneSnapshot {
            revision: 7,
            cameras: HashMap::from([(
                "camera-main".to_string(),
                default_frontend_preview_camera(),
            )]),
            active_camera_id: "camera-main".to_string(),
            nodes: HashMap::from([("root".to_string(), root), ("triangle".to_string(), child)]),
            meshes: HashMap::from([(
                "triangleMesh".to_string(),
                MeshAssetSnapshot {
                    source: MeshSourceSnapshot::TriangleMesh {
                        geometry: TriangleMeshSnapshot {
                            positions: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
                            normals: None,
                            uvs: None,
                            indices: vec![0, 1, 2],
                        },
                    },
                },
            )]),
            materials: HashMap::from([(
                "mat".to_string(),
                material_snapshot([0.8, 0.7, 0.6, 1.0], 0.5, 0.0, [0.0, 0.0, 0.0]),
            )]),
        }
    }

    #[derive(Debug)]
    struct Rgba8Stats {
        non_black_pixels: usize,
        max_luma: u8,
        unique_rgb_count: usize,
    }

    fn rgba8_stats(pixels: &[u8]) -> Rgba8Stats {
        let mut unique_rgb = BTreeSet::new();
        let mut non_black_pixels = 0;
        let mut max_luma = 0;

        for pixel in pixels.chunks_exact(4) {
            let [red, green, blue, _alpha] = [pixel[0], pixel[1], pixel[2], pixel[3]];
            unique_rgb.insert((red, green, blue));

            let luma = ((red as u16 * 54 + green as u16 * 183 + blue as u16 * 19) / 256) as u8;
            max_luma = max_luma.max(luma);
            if luma > 2 {
                non_black_pixels += 1;
            }
        }

        Rgba8Stats {
            non_black_pixels,
            max_luma,
            unique_rgb_count: unique_rgb.len(),
        }
    }

    fn effective_vertical_fov_degrees(params: &lupin_pt::CameraParams) -> f32 {
        let vertical_film = if params.aspect >= 1.0 {
            params.film / params.aspect
        } else {
            params.film
        };

        (2.0 * (vertical_film / (2.0 * params.lens)).atan()).to_degrees()
    }
}

#[tauri::command]
async fn polyhaven_material_files(
    state: tauri::State<'_, AppState>,
    id: String,
    resolution: Option<String>,
) -> Result<PolyHavenMaterialFiles, String> {
    fetch_material_files(&state.polyhaven, id, resolution).await
}

#[tauri::command]
async fn download_polyhaven_material(
    state: tauri::State<'_, AppState>,
    id: String,
    resolution: Option<String>,
    roles: Option<Vec<String>>,
) -> Result<DownloadedMaterial, String> {
    let requested_roles = roles.unwrap_or_default();
    let material_files = fetch_material_files(&state.polyhaven, id.clone(), resolution).await?;
    let resolved_resolution = material_files.resolution;
    let selected = select_download_files(material_files.files, &requested_roles);

    if selected.is_empty() {
        return Err(format!(
            "no downloadable texture maps found for {id} at {}",
            resolved_resolution
        ));
    }

    let directory = material_cache_dir(&id, &resolved_resolution)?;
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;

    let mut downloaded = Vec::with_capacity(selected.len());
    for file in selected {
        let bytes = state
            .polyhaven
            .get(&file.url)
            .send()
            .await
            .map_err(polyhaven_error)?
            .error_for_status()
            .map_err(polyhaven_error)?
            .bytes()
            .await
            .map_err(polyhaven_error)?;

        let filename = texture_filename(&id, &file);
        let local_path = directory.join(filename);
        std::fs::write(&local_path, &bytes).map_err(|error| error.to_string())?;

        downloaded.push(DownloadedMaterialFile {
            role: file.role,
            map: file.map,
            format: file.format,
            local_path: local_path.display().to_string(),
            source_url: file.url,
            bytes: bytes.len() as u64,
        });
    }

    Ok(DownloadedMaterial {
        id,
        resolution: resolved_resolution,
        directory: directory.display().to_string(),
        files: downloaded,
    })
}

async fn fetch_material_files(
    client: &reqwest::Client,
    id: String,
    resolution: Option<String>,
) -> Result<PolyHavenMaterialFiles, String> {
    let requested_resolution = normalize_resolution(resolution);
    let tree = client
        .get(format!("{POLYHAVEN_API}/files/{id}"))
        .send()
        .await
        .map_err(polyhaven_error)?
        .error_for_status()
        .map_err(polyhaven_error)?
        .json::<Value>()
        .await
        .map_err(polyhaven_error)?;

    let resolved_resolution = resolve_texture_resolution(&tree, &requested_resolution);
    let mut files = extract_texture_files(&tree, &resolved_resolution);
    files.sort_by(|a, b| {
        role_rank(&a.role)
            .cmp(&role_rank(&b.role))
            .then(a.map.cmp(&b.map))
    });

    Ok(PolyHavenMaterialFiles {
        id,
        resolution: resolved_resolution,
        files,
    })
}

fn extract_texture_files(tree: &Value, resolution: &str) -> Vec<MaterialFile> {
    let Some(map_nodes) = tree.as_object() else {
        return Vec::new();
    };

    let mut files = Vec::new();
    for (map, resolutions) in map_nodes {
        if matches!(map.as_str(), "blend" | "gltf" | "mtlx") {
            continue;
        }

        let Some(resolution_nodes) = resolutions.as_object() else {
            continue;
        };

        for (candidate_resolution, formats) in resolution_nodes {
            if candidate_resolution != resolution {
                continue;
            }

            let Some(format_nodes) = formats.as_object() else {
                continue;
            };

            for (format, value) in format_nodes {
                let Some(url) = value.get("url").and_then(Value::as_str) else {
                    continue;
                };

                files.push(MaterialFile {
                    map: map.clone(),
                    role: material_role(map),
                    resolution: candidate_resolution.clone(),
                    format: format.clone(),
                    url: url.to_string(),
                    md5: value.get("md5").and_then(Value::as_str).map(str::to_string),
                    size: value.get("size").and_then(Value::as_u64),
                });
            }
        }
    }

    files
}

fn select_download_files(
    files: Vec<MaterialFile>,
    requested_roles: &[String],
) -> Vec<MaterialFile> {
    let mut by_role: HashMap<String, MaterialFile> = HashMap::new();

    for file in files {
        if !requested_roles.is_empty() && !requested_roles.iter().any(|role| role == &file.role) {
            continue;
        }

        by_role
            .entry(file.role.clone())
            .and_modify(|current| {
                if file_rank(&file) < file_rank(current) {
                    *current = file.clone();
                }
            })
            .or_insert(file);
    }

    let mut selected = by_role.into_values().collect::<Vec<_>>();
    selected.sort_by(|a, b| role_rank(&a.role).cmp(&role_rank(&b.role)));
    selected
}

fn material_role(map: &str) -> String {
    let normalized = map.to_lowercase();

    if normalized.contains("diff") || normalized.contains("color") || normalized == "albedo" {
        "baseColor"
    } else if normalized.contains("rough") {
        "roughness"
    } else if normalized.contains("nor") || normalized.contains("normal") {
        "normal"
    } else if normalized.contains("metal") {
        "metallic"
    } else if normalized.contains("arm") {
        "occlusionRoughnessMetallic"
    } else if normalized.contains("ao") {
        "ambientOcclusion"
    } else if normalized.contains("disp") || normalized.contains("height") {
        "displacement"
    } else if normalized.contains("spec") {
        "specular"
    } else {
        "auxiliary"
    }
    .to_string()
}

fn role_rank(role: &str) -> u8 {
    match role {
        "baseColor" => 0,
        "roughness" => 1,
        "normal" => 2,
        "metallic" => 3,
        "ambientOcclusion" => 4,
        "occlusionRoughnessMetallic" => 5,
        "displacement" => 6,
        "specular" => 7,
        _ => 8,
    }
}

fn file_rank(file: &MaterialFile) -> (u8, u8) {
    (format_rank(&file.format), map_rank(&file.map))
}

fn map_rank(map: &str) -> u8 {
    match map {
        "nor_gl" => 0,
        "diff" => 0,
        "rough" => 0,
        "metal" => 0,
        "disp" => 0,
        "nor_dx" => 1,
        _ => 2,
    }
}

fn format_rank(format: &str) -> u8 {
    match format {
        "png" => 0,
        "jpg" | "jpeg" => 1,
        "exr" => 2,
        _ => 3,
    }
}

fn normalize_resolution(resolution: Option<String>) -> String {
    resolution
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("2k")
        .to_lowercase()
}

fn resolve_texture_resolution(tree: &Value, requested: &str) -> String {
    let Some(map_nodes) = tree.as_object() else {
        return requested.to_string();
    };

    let mut available = BTreeSet::new();
    for (map, resolutions) in map_nodes {
        if matches!(map.as_str(), "blend" | "gltf" | "mtlx") {
            continue;
        }

        let Some(resolution_nodes) = resolutions.as_object() else {
            continue;
        };

        available.extend(resolution_nodes.keys().cloned());
    }

    if available.contains(requested) {
        return requested.to_string();
    }

    let requested_rank = resolution_rank(requested);
    available
        .iter()
        .filter(|resolution| resolution_rank(resolution.as_str()) <= requested_rank)
        .max_by_key(|resolution| resolution_rank(resolution.as_str()))
        .cloned()
        .or_else(|| {
            available
                .iter()
                .min_by_key(|resolution| resolution_rank(resolution.as_str()))
                .cloned()
        })
        .unwrap_or_else(|| requested.to_string())
}

fn resolution_rank(resolution: &str) -> u32 {
    let normalized = resolution.trim().to_lowercase();

    if let Some(value) = normalized.strip_suffix('k') {
        return value.parse::<u32>().unwrap_or_default() * 1024;
    }

    normalized.parse::<u32>().unwrap_or_default()
}

fn texture_categories_from_counts(counts: HashMap<String, u64>) -> Vec<PolyHavenCategory> {
    let mut categories = counts
        .into_iter()
        .map(|(id, count)| PolyHavenCategory {
            name: category_label(&id),
            id,
            count,
        })
        .collect::<Vec<_>>();

    categories.sort_by(|a, b| {
        (a.id != "all")
            .cmp(&(b.id != "all"))
            .then(b.count.cmp(&a.count))
            .then(a.name.cmp(&b.name))
    });
    categories
}

fn category_label(id: &str) -> String {
    if id == "all" {
        return "All".to_string();
    }

    id.split('/')
        .map(|part| {
            part.split(['-', ' '])
                .filter(|word| !word.is_empty())
                .map(|word| {
                    let mut chars = word.chars();
                    let Some(first) = chars.next() else {
                        return String::new();
                    };
                    format!("{}{}", first.to_uppercase(), chars.as_str())
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join(" / ")
}

fn texture_filename(id: &str, file: &MaterialFile) -> String {
    format!(
        "{}_{}_{}.{}",
        sanitize_path_segment(id),
        sanitize_path_segment(&file.map),
        sanitize_path_segment(&file.resolution),
        sanitize_path_segment(&file.format)
    )
}

fn material_cache_dir(id: &str, resolution: &str) -> Result<PathBuf, String> {
    Ok(std::env::temp_dir()
        .join("soyel-polyhaven")
        .join(sanitize_path_segment(id))
        .join(sanitize_path_segment(resolution)))
}

fn sanitize_path_segment(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn polyhaven_error(error: reqwest::Error) -> String {
    format!("Poly Haven request failed: {error}")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            renderer_status,
            select_scene_surface,
            apply_material_to_selection,
            render_preview_frame,
            stream_preview_frame,
            polyhaven_search_materials,
            polyhaven_texture_categories,
            polyhaven_material_files,
            download_polyhaven_material,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
