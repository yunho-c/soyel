use std::{
    collections::{BTreeSet, HashMap},
    path::PathBuf,
    sync::Mutex,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

const POLYHAVEN_API: &str = "https://api.polyhaven.com";
const POLYHAVEN_USER_AGENT: &str = "soyel-renderer/0.1 (Poly Haven material discovery)";

#[derive(Debug)]
struct AppState {
    renderer: Mutex<RendererSession>,
    polyhaven: reqwest::Client,
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
            id: "chair-shell".to_string(),
            label: "Chair Shell".to_string(),
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
            polyhaven_search_materials,
            polyhaven_material_files,
            download_polyhaven_material,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
