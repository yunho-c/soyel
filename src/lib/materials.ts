import { invoke } from "@tauri-apps/api/core";

import type { CameraState } from "$lib/viewport-scene";

export type SurfaceSelection = {
  id: string;
  label: string;
};

export type MaterialFile = {
  map: string;
  role: string;
  resolution: string;
  format: string;
  url: string;
  md5?: string;
  size?: number;
};

export type PolyHavenMaterial = {
  id: string;
  name: string;
  categories: string[];
  tags: string[];
  authors: string[];
  maxResolution?: number[];
  dimensionsMm?: number[];
  thumbnailUrl?: string;
  downloadCount: number;
  datePublished?: number;
};

export type PolyHavenCategory = {
  id: string;
  name: string;
  count: number;
};

export type RendererStatus = {
  engineName: string;
  engineVersion: string;
  gpuApi: string;
  materialModel: string;
  selectedSurface: SurfaceSelection;
  appliedMaterials: AppliedMaterial[];
  notes: string[];
};

export type AppliedMaterial = {
  surfaceId: string;
  surfaceLabel: string;
  materialId: string;
  materialName: string;
  thumbnailUrl?: string;
  categories: string[];
  authors: string[];
  maps: MaterialFile[];
};

export type MaterialApplication = {
  id: string;
  name: string;
  thumbnailUrl?: string;
  categories: string[];
  authors: string[];
  maps: MaterialFile[];
};

export type PolyHavenMaterialFiles = {
  id: string;
  resolution: string;
  files: MaterialFile[];
};

export type DownloadedMaterial = {
  id: string;
  resolution: string;
  directory: string;
  files: Array<{
    role: string;
    map: string;
    format: string;
    localPath: string;
    sourceUrl: string;
    bytes: number;
  }>;
};

export type RenderPreviewFrame = {
  revision: number;
  width: number;
  height: number;
  samples: number;
  surfaceLabel: string;
  materialName?: string;
  pixels: number[];
};

export type RenderPreviewRequest = {
  revision?: number;
  width?: number;
  height?: number;
  samples?: number;
  camera?: CameraState;
};

const mockFiles: MaterialFile[] = [
  {
    map: "diff",
    role: "baseColor",
    resolution: "2k",
    format: "jpg",
    url: "https://cdn.polyhaven.com/asset_img/thumbs/brick_floor_003.png?width=512&height=512",
    size: 2_420_000,
  },
  {
    map: "rough",
    role: "roughness",
    resolution: "2k",
    format: "png",
    url: "https://cdn.polyhaven.com/asset_img/thumbs/brick_floor_003.png?width=512&height=512",
    size: 1_860_000,
  },
  {
    map: "nor_gl",
    role: "normal",
    resolution: "2k",
    format: "png",
    url: "https://cdn.polyhaven.com/asset_img/thumbs/brick_floor_003.png?width=512&height=512",
    size: 3_120_000,
  },
];

const mockMaterials: PolyHavenMaterial[] = [
  {
    id: "brick_floor_003",
    name: "Brick Floor 003",
    categories: ["floor", "brick", "outdoor"],
    tags: ["brown", "pavement", "aged"],
    authors: ["Dimitrios Savva (Photography)", "Rob Tuytel (Processing)"],
    maxResolution: [8192, 8192],
    dimensionsMm: [2000, 2000],
    thumbnailUrl: "https://cdn.polyhaven.com/asset_img/thumbs/brick_floor_003.png?width=512&height=512",
    downloadCount: 129_400,
    datePublished: 1_618_385_199,
  },
  {
    id: "aerial_asphalt_01",
    name: "Aerial Asphalt 01",
    categories: ["floor", "asphalt", "road"],
    tags: ["flat", "cracked", "industrial"],
    authors: ["Rob Tuytel (All)"],
    maxResolution: [8192, 8192],
    dimensionsMm: [30000, 30000],
    thumbnailUrl: "https://cdn.polyhaven.com/asset_img/thumbs/aerial_asphalt_01.png?width=512&height=512",
    downloadCount: 91_200,
    datePublished: 1_597_061_145,
  },
  {
    id: "wood_planks_grey_02",
    name: "Wood Planks Grey 02",
    categories: ["wood", "floor", "wall"],
    tags: ["planks", "weathered", "grey"],
    authors: ["Poly Haven (All)"],
    maxResolution: [8192, 8192],
    dimensionsMm: [4000, 4000],
    thumbnailUrl: "https://cdn.polyhaven.com/asset_img/thumbs/wood_planks_grey_02.png?width=512&height=512",
    downloadCount: 73_600,
    datePublished: 1_664_150_400,
  },
  {
    id: "rock_boulder_dry",
    name: "Rock Boulder Dry",
    categories: ["rock", "natural", "outdoor"],
    tags: ["stone", "rough", "scan"],
    authors: ["Poly Haven (All)"],
    maxResolution: [8192, 8192],
    dimensionsMm: [2500, 2500],
    thumbnailUrl: "https://cdn.polyhaven.com/asset_img/thumbs/rock_boulder_dry.png?width=512&height=512",
    downloadCount: 68_900,
    datePublished: 1_672_012_800,
  },
];

const mockCategories: PolyHavenCategory[] = [
  {
    id: "all",
    name: "All",
    count: mockMaterials.length,
  },
  ...Array.from(new Set(mockMaterials.flatMap((material) => material.categories)))
    .sort()
    .map((id) => ({
      id,
      name: id
        .split("/")
        .map((part) =>
          part
            .split(/[- ]+/)
            .filter(Boolean)
            .map((word) => `${word[0]?.toUpperCase() ?? ""}${word.slice(1)}`)
            .join(" "),
        )
        .join(" / "),
      count: mockMaterials.filter((material) => material.categories.includes(id)).length,
    })),
];

const mockStatus: RendererStatus = {
  engineName: "LupinPathTracer",
  engineVersion: "0.1.0",
  gpuApi: "wgpu path tracing",
  materialModel: "Lupin Yocto/GL material fields with glTF-PBR compatibility",
  selectedSurface: { id: "chair-shell", label: "Chair Shell" },
  appliedMaterials: [],
  notes: [
    "Browser preview mode is using local sample data.",
    "Run inside Tauri to query and download live Poly Haven files.",
  ],
};

const mockPreviewFrame: RenderPreviewFrame = {
  revision: 1,
  width: 360,
  height: 260,
  samples: 6,
  surfaceLabel: "Chair Shell",
  materialName: "Brick Floor 003",
  pixels: [],
};

function hasTauriRuntime() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export async function rendererStatus() {
  if (!hasTauriRuntime()) {
    return mockStatus;
  }

  return invoke<RendererStatus>("renderer_status");
}

export async function selectSceneSurface(surfaceId: string, surfaceLabel: string) {
  if (!hasTauriRuntime()) {
    mockStatus.selectedSurface = { id: surfaceId, label: surfaceLabel };
    return mockStatus;
  }

  return invoke<RendererStatus>("select_scene_surface", { surfaceId, surfaceLabel });
}

export async function searchPolyHavenMaterials(args: {
  query?: string;
  category?: string;
  sort?: string;
  limit?: number;
}) {
  if (!hasTauriRuntime()) {
    const query = args.query?.toLowerCase().trim() ?? "";
    return mockMaterials
      .filter((material) => {
        const inCategory = !args.category || args.category === "all" || material.categories.includes(args.category);
        const inQuery =
          !query ||
          material.name.toLowerCase().includes(query) ||
          material.id.includes(query) ||
          material.tags.some((tag) => tag.toLowerCase().includes(query));
        return inCategory && inQuery;
      })
      .slice(0, args.limit ?? 48);
  }

  return invoke<PolyHavenMaterial[]>("polyhaven_search_materials", args);
}

export async function polyHavenTextureCategories() {
  if (!hasTauriRuntime()) {
    return mockCategories;
  }

  return invoke<PolyHavenCategory[]>("polyhaven_texture_categories");
}

export async function polyHavenMaterialFiles(id: string, resolution: string) {
  if (!hasTauriRuntime()) {
    return { id, resolution, files: mockFiles } satisfies PolyHavenMaterialFiles;
  }

  return invoke<PolyHavenMaterialFiles>("polyhaven_material_files", { id, resolution });
}

export async function applyMaterialToSelection(material: MaterialApplication) {
  if (!hasTauriRuntime()) {
    const surface = mockStatus.selectedSurface;
    mockStatus.appliedMaterials = [
      ...mockStatus.appliedMaterials.filter((item) => item.surfaceId !== surface.id),
      {
        surfaceId: surface.id,
        surfaceLabel: surface.label,
        materialId: material.id,
        materialName: material.name,
        thumbnailUrl: material.thumbnailUrl,
        categories: material.categories,
        authors: material.authors,
        maps: material.maps,
      },
    ];
    return mockStatus;
  }

  return invoke<RendererStatus>("apply_material_to_selection", { material });
}

export async function renderPreviewFrame(args: RenderPreviewRequest) {
  if (!hasTauriRuntime()) {
    return {
      ...mockPreviewFrame,
      revision: args.revision ?? mockPreviewFrame.revision,
      width: args.width ?? mockPreviewFrame.width,
      height: args.height ?? mockPreviewFrame.height,
      surfaceLabel: mockStatus.selectedSurface.label,
      materialName: mockStatus.appliedMaterials.find((item) => item.surfaceId === mockStatus.selectedSurface.id)
        ?.materialName,
    };
  }

  return invoke<RenderPreviewFrame>("render_preview_frame", args);
}

export async function downloadPolyHavenMaterial(id: string, resolution: string, roles: string[]) {
  if (!hasTauriRuntime()) {
    return {
      id,
      resolution,
      directory: "/tmp/soyel-polyhaven/mock",
      files: mockFiles
        .filter((file) => roles.length === 0 || roles.includes(file.role))
        .map((file) => ({
          role: file.role,
          map: file.map,
          format: file.format,
          localPath: `/tmp/soyel-polyhaven/mock/${id}_${file.map}.${file.format}`,
          sourceUrl: file.url,
          bytes: file.size ?? 0,
        })),
    } satisfies DownloadedMaterial;
  }

  return invoke<DownloadedMaterial>("download_polyhaven_material", { id, resolution, roles });
}
