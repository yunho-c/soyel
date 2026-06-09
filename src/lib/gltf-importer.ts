import * as THREE from "three";
import { GLTFLoader, type GLTF } from "three/addons/loaders/GLTFLoader.js";

import {
  cloneCamera,
  identityTransform,
  pbrMaterial,
  quatFromEuler,
  type MaterialAsset,
  type MeshAsset,
  type SceneCamera,
  type SceneNode,
  type SoyelScene,
  type Transform,
  type Vec3,
} from "$lib/viewport-scene";

type ImportedSceneParts = {
  nodes: Record<string, SceneNode>;
  meshes: Record<string, MeshAsset>;
  materials: Record<string, MaterialAsset>;
  cameras: Record<string, SceneCamera>;
  activeCameraId: string;
  selection: string | null;
  bounds: SceneBounds | null;
  hasEmissiveMaterial: boolean;
};

type SceneBounds = {
  min: Vec3;
  max: Vec3;
};

type ImportResourceFile = File & {
  webkitRelativePath?: string;
};

export async function importGltfScene(file: File, resourceFiles: Iterable<File> = [file]): Promise<SoyelScene> {
  const resourceIndex = buildResourceIndex(resourceFiles);
  const objectUrls: string[] = [];
  const manager = new THREE.LoadingManager();
  manager.setURLModifier((url) => {
    const resource = resolveResourceFile(url, resourceIndex);
    if (!resource) {
      return url;
    }

    const objectUrl = URL.createObjectURL(resource);
    objectUrls.push(objectUrl);
    return objectUrl;
  });

  let gltf: GLTF;
  try {
    gltf = await parseGltf(await readGltfPayload(file, resourceIndex), file.name, manager);
  } finally {
    for (const url of objectUrls) {
      URL.revokeObjectURL(url);
    }
  }

  const imported = normalizeGltf(gltf, file.name);

  if (!imported.bounds) {
    throw new Error("The imported glTF scene did not contain any static mesh geometry.");
  }

  if (!imported.hasEmissiveMaterial) {
    addDefaultAreaLight(imported, file.name);
  }

  return {
    schemaVersion: 1,
    revision: 1,
    units: "meter",
    cameras: imported.cameras,
    activeCameraId: imported.activeCameraId,
    nodes: imported.nodes,
    meshes: imported.meshes,
    materials: imported.materials,
    selection: imported.selection,
  };
}

async function readGltfPayload(file: File, resourceIndex: Map<string, File>) {
  if (!file.name.toLowerCase().endsWith(".gltf")) {
    return file.arrayBuffer();
  }

  const text = await file.text();
  const missing = missingExternalResourceUris(text, resourceIndex);
  if (missing.length) {
    throw new Error(
      `The glTF file "${file.name}" references external resources that were not included: ${missing.join(
        ", ",
      )}. Select the .gltf file together with its .bin and image sidecars, or import the .glb variant.`,
    );
  }

  return text;
}

function parseGltf(payload: string | ArrayBuffer, filename: string, manager: THREE.LoadingManager) {
  return new Promise<GLTF>((resolve, reject) => {
    const loader = new GLTFLoader(manager);
    loader.parse(
      payload,
      "",
      (gltf) => resolve(gltf),
      (error) => {
        const kind = filename.toLowerCase().endsWith(".gltf") ? "glTF" : "GLB";
        reject(new Error(`Failed to parse ${kind} file "${filename}": ${String(error)}`));
      },
    );
  });
}

function buildResourceIndex(files: Iterable<File>) {
  const index = new Map<string, File>();

  for (const file of files as Iterable<ImportResourceFile>) {
    const relativePath = file.webkitRelativePath || file.name;
    for (const key of resourceKeys(relativePath)) {
      index.set(key, file);
    }
  }

  return index;
}

function missingExternalResourceUris(text: string, resourceIndex: Map<string, File>) {
  const missing = new Set<string>();
  const document = JSON.parse(text) as {
    buffers?: Array<{ uri?: string }>;
    images?: Array<{ uri?: string }>;
  };

  for (const resource of [...(document.buffers ?? []), ...(document.images ?? [])]) {
    const uri = resource.uri;
    if (!uri || isEmbeddedOrRemoteUri(uri)) {
      continue;
    }

    if (!resolveResourceFile(uri, resourceIndex)) {
      missing.add(uri);
    }
  }

  return [...missing].sort();
}

function resolveResourceFile(uri: string, resourceIndex: Map<string, File>) {
  if (isEmbeddedOrRemoteUri(uri)) {
    return null;
  }

  for (const key of resourceKeys(uri)) {
    const file = resourceIndex.get(key);
    if (file) {
      return file;
    }
  }

  return null;
}

function resourceKeys(path: string) {
  const normalized = normalizeResourcePath(path);
  const parts = normalized.split("/").filter(Boolean);
  const basename = parts.at(-1) ?? normalized;

  return new Set([normalized, basename, decodeURIComponent(normalized), decodeURIComponent(basename)]);
}

function normalizeResourcePath(path: string) {
  return path
    .split(/[?#]/u)[0]
    .replace(/\\/gu, "/")
    .replace(/^\.\/+/u, "")
    .replace(/^\/+/u, "");
}

function isEmbeddedOrRemoteUri(uri: string) {
  return /^(data|blob|https?):/iu.test(uri);
}

function normalizeGltf(gltf: GLTF, filename: string): ImportedSceneParts {
  const prefix = `import:${slug(filename, "scene")}`;
  const rootId = `${prefix}:root`;
  const objectNodeIds = new Map<string, string>();
  const materialIds = new Map<string, string>();
  const parts: ImportedSceneParts = {
    nodes: {
      [rootId]: branchNode(rootId, filename.replace(/\.(glb|gltf)$/i, "") || "Imported Scene", null, true),
    },
    meshes: {},
    materials: {},
    cameras: {},
    activeCameraId: `${prefix}:camera`,
    selection: null,
    bounds: null,
    hasEmissiveMaterial: false,
  };

  let objectCounter = 0;
  let meshCounter = 0;
  let materialCounter = 0;
  let cameraCounter = 0;

  gltf.scene.updateWorldMatrix(true, true);

  const ensureObjectNode = (object: THREE.Object3D): string => {
    const existing = objectNodeIds.get(object.uuid);
    if (existing) {
      return existing;
    }

    const parentId =
      object.parent && object.parent !== gltf.scene ? ensureObjectNode(object.parent) : rootId;
    const nodeId = `${prefix}:object:${objectCounter++}-${slug(object.name, object.type.toLowerCase())}`;
    objectNodeIds.set(object.uuid, nodeId);
    parts.nodes[nodeId] = branchNode(
      nodeId,
      object.name || object.type,
      parentId,
      object.visible,
    );
    return nodeId;
  };

  const ensureMaterial = (material: THREE.Material | undefined): string => {
    if (!material) {
      const fallbackId = `${prefix}:material:fallback`;
      if (!parts.materials[fallbackId]) {
        parts.materials[fallbackId] = pbrMaterial(
          fallbackId,
          "Imported Default",
          [0.68, 0.7, 0.66, 1],
          0.72,
          0.02,
        );
      }
      return fallbackId;
    }

    const existing = materialIds.get(material.uuid);
    if (existing) {
      return existing;
    }

    const id = `${prefix}:material:${materialCounter++}-${slug(material.name, "material")}`;
    const asset = materialAssetFromThree(id, material);
    parts.materials[id] = asset;
    parts.hasEmissiveMaterial ||= asset.emissive.some((value) => value > 0.001);
    materialIds.set(material.uuid, id);
    return id;
  };

  gltf.scene.traverse((object) => {
    if (object === gltf.scene) {
      return;
    }

    const parentId = ensureObjectNode(object);

    if (isPerspectiveCamera(object)) {
      const cameraId = `${prefix}:camera:${cameraCounter++}-${slug(object.name, "camera")}`;
      parts.cameras[cameraId] = cameraFromThree(cameraId, object, parts.bounds);
      parts.activeCameraId = cameraId;
      return;
    }

    if (!isMesh(object)) {
      return;
    }

    const geometry = object.geometry.clone();
    const position = geometry.getAttribute("position");
    if (!position || position.count < 3) {
      geometry.dispose();
      return;
    }

    if (!geometry.getAttribute("normal")) {
      geometry.computeVertexNormals();
    }

    const materialList = Array.isArray(object.material) ? object.material : [object.material];
    const groups = geometry.groups.length
      ? geometry.groups
      : [
          {
            start: 0,
            count: geometry.getIndex()?.count ?? position.count,
            materialIndex: 0,
          },
        ];
    const worldTransform = transformFromMatrix(object.matrixWorld);

    for (const group of groups) {
      const geometryPayload = triangleMeshFromGeometry(geometry, group.start, group.count);
      if (!geometryPayload || geometryPayload.indices.length < 3) {
        continue;
      }

      const materialId = ensureMaterial(materialList[group.materialIndex ?? 0]);
      const meshId = `${prefix}:mesh:${meshCounter}-${slug(object.name, "mesh")}`;
      const nodeId = `${prefix}:node:${meshCounter}-${slug(object.name, "mesh")}`;
      const bounds = boundsFromTriangleMesh(geometryPayload.positions, geometryPayload.indices);

      parts.meshes[meshId] = {
        id: meshId,
        name: object.name || `Mesh ${meshCounter + 1}`,
        source: {
          type: "triangleMesh",
          geometry: geometryPayload,
        },
        bounds,
      };
      parts.nodes[nodeId] = {
        id: nodeId,
        name: meshNodeName(object.name, groups.length, meshCounter),
        parentId,
        meshId,
        materialBindings: { default: materialId },
        transform: worldTransform,
        visible: object.visible,
        selectable: true,
      };
      parts.selection ??= nodeId;
      mergeBounds(parts, bounds, worldTransform);
      meshCounter += 1;
    }

    geometry.dispose();
  });

  if (Object.keys(parts.cameras).length === 0 && parts.bounds) {
    const camera = framedCamera(`${prefix}:camera`, "Imported Camera", parts.bounds);
    parts.cameras[camera.id] = camera;
    parts.activeCameraId = camera.id;
  }

  return parts;
}

function branchNode(id: string, name: string, parentId: string | null, visible: boolean): SceneNode {
  return {
    id,
    name,
    parentId,
    meshId: null,
    materialBindings: {},
    transform: identityTransform(),
    visible,
    selectable: false,
  };
}

function materialAssetFromThree(id: string, material: THREE.Material): MaterialAsset {
  const standard = material as THREE.MeshStandardMaterial;
  const color = standard.color instanceof THREE.Color ? standard.color : new THREE.Color(0xadb2a8);
  const opacity = typeof standard.opacity === "number" ? standard.opacity : 1;
  const emissive = standard.emissive instanceof THREE.Color ? standard.emissive : new THREE.Color(0x000000);
  const emissiveIntensity =
    typeof standard.emissiveIntensity === "number" && Number.isFinite(standard.emissiveIntensity)
      ? standard.emissiveIntensity
      : 1;

  return {
    id,
    name: material.name || "Imported Material",
    model: "gltf-pbr",
    baseColor: [color.r, color.g, color.b, opacity],
    roughness: finiteOr(standard.roughness, 0.72),
    metallic: finiteOr(standard.metalness, 0.02),
    emissive: [
      emissive.r * emissiveIntensity,
      emissive.g * emissiveIntensity,
      emissive.b * emissiveIntensity,
    ],
    textureRefs: textureRefsFromMaterial(standard),
  };
}

function textureRefsFromMaterial(material: THREE.MeshStandardMaterial): MaterialAsset["textureRefs"] {
  const refs: MaterialAsset["textureRefs"] = {};
  const entries = [
    ["baseColor", material.map, "srgb"],
    ["roughness", material.roughnessMap, "linear"],
    ["metallic", material.metalnessMap, "linear"],
    ["normal", material.normalMap, "linear"],
    ["emissive", material.emissiveMap, "srgb"],
  ] as const;

  for (const [role, texture, colorSpace] of entries) {
    if (texture) {
      refs[role] = {
        uri: texture.name || texture.uuid,
        colorSpace,
      };
    }
  }

  return refs;
}

function triangleMeshFromGeometry(
  geometry: THREE.BufferGeometry,
  start: number,
  count: number,
): { positions: number[]; normals?: number[]; uvs?: number[]; indices: number[] } | null {
  const position = geometry.getAttribute("position");
  if (!position) {
    return null;
  }

  const normal = geometry.getAttribute("normal");
  const uv = geometry.getAttribute("uv");
  const index = geometry.getIndex();
  const triangleCount = Math.floor(count / 3) * 3;
  const indices: number[] = [];

  if (index) {
    for (let offset = start; offset < start + triangleCount; offset += 1) {
      indices.push(index.getX(offset));
    }
  } else {
    for (let vertex = start; vertex < start + triangleCount; vertex += 1) {
      indices.push(vertex);
    }
  }

  return {
    positions: Array.from(position.array, Number),
    normals: normal ? Array.from(normal.array, Number) : undefined,
    uvs: uv ? Array.from(uv.array, Number) : undefined,
    indices,
  };
}

function boundsFromTriangleMesh(positions: number[], indices: number[]): SceneBounds {
  const bounds: SceneBounds = {
    min: [Number.POSITIVE_INFINITY, Number.POSITIVE_INFINITY, Number.POSITIVE_INFINITY],
    max: [Number.NEGATIVE_INFINITY, Number.NEGATIVE_INFINITY, Number.NEGATIVE_INFINITY],
  };

  for (const vertexIndex of indices) {
    const offset = vertexIndex * 3;
    expandBounds(bounds, [positions[offset] ?? 0, positions[offset + 1] ?? 0, positions[offset + 2] ?? 0]);
  }

  return normalizeBounds(bounds);
}

function mergeBounds(parts: ImportedSceneParts, bounds: SceneBounds, transform: Transform) {
  const corners = [
    [bounds.min[0], bounds.min[1], bounds.min[2]],
    [bounds.max[0], bounds.min[1], bounds.min[2]],
    [bounds.min[0], bounds.max[1], bounds.min[2]],
    [bounds.max[0], bounds.max[1], bounds.min[2]],
    [bounds.min[0], bounds.min[1], bounds.max[2]],
    [bounds.max[0], bounds.min[1], bounds.max[2]],
    [bounds.min[0], bounds.max[1], bounds.max[2]],
    [bounds.max[0], bounds.max[1], bounds.max[2]],
  ] satisfies Vec3[];

  const matrix = matrixFromTransform(transform);
  for (const corner of corners) {
    const point = new THREE.Vector3(...corner).applyMatrix4(matrix);
    if (!parts.bounds) {
      parts.bounds = { min: [point.x, point.y, point.z], max: [point.x, point.y, point.z] };
    } else {
      expandBounds(parts.bounds, [point.x, point.y, point.z]);
    }
  }
}

function expandBounds(bounds: SceneBounds, point: Vec3) {
  bounds.min = [
    Math.min(bounds.min[0], point[0]),
    Math.min(bounds.min[1], point[1]),
    Math.min(bounds.min[2], point[2]),
  ];
  bounds.max = [
    Math.max(bounds.max[0], point[0]),
    Math.max(bounds.max[1], point[1]),
    Math.max(bounds.max[2], point[2]),
  ];
}

function normalizeBounds(bounds: SceneBounds): SceneBounds {
  return {
    min: bounds.min.map((value) => (Number.isFinite(value) ? value : 0)) as Vec3,
    max: bounds.max.map((value) => (Number.isFinite(value) ? value : 0)) as Vec3,
  };
}

function addDefaultAreaLight(parts: ImportedSceneParts, filename: string) {
  if (!parts.bounds) {
    return;
  }

  const prefix = `import:${slug(filename, "scene")}`;
  const center = boundsCenter(parts.bounds);
  const radius = Math.max(0.5, boundsRadius(parts.bounds));
  const meshId = `${prefix}:mesh:default-light`;
  const materialId = `${prefix}:material:default-light`;
  const nodeId = `${prefix}:node:default-light`;
  const rootId = `${prefix}:root`;

  parts.meshes[meshId] = {
    id: meshId,
    name: "Default Area Light",
    source: {
      type: "procedural",
      primitive: { type: "plane", size: [radius * 0.9, radius * 0.65] },
    },
    bounds: {
      min: [-radius * 0.45, -radius * 0.325, 0],
      max: [radius * 0.45, radius * 0.325, 0],
    },
  };
  parts.materials[materialId] = {
    ...pbrMaterial(materialId, "Default Area Light", [1, 0.9, 0.68, 1], 0.28, 0),
    emissive: [12, 10, 7],
  };
  parts.nodes[nodeId] = {
    id: nodeId,
    name: "Default Area Light",
    parentId: parts.nodes[rootId] ? rootId : null,
    meshId,
    materialBindings: { default: materialId },
    transform: {
      translation: [center[0], parts.bounds.max[1] + radius * 0.62, center[2]],
      rotation: quatFromEuler(Math.PI / 2, 0, 0),
      scale: [1, 1, 1],
    },
    visible: true,
    selectable: false,
  };
}

function framedCamera(id: string, name: string, bounds: SceneBounds): SceneCamera {
  const center = boundsCenter(bounds);
  const radius = Math.max(0.5, boundsRadius(bounds));
  const camera = {
    id,
    name,
    position: [center[0], center[1] + radius * 0.35, center[2] - radius * 2.4] as Vec3,
    target: center,
    up: [0, 1, 0] as Vec3,
    fovDegrees: 42,
  };

  return {
    ...camera,
    ...cloneCamera(camera),
  };
}

function cameraFromThree(id: string, camera: THREE.PerspectiveCamera, bounds: SceneBounds | null): SceneCamera {
  const position = new THREE.Vector3();
  const direction = new THREE.Vector3();
  camera.getWorldPosition(position);
  camera.getWorldDirection(direction);
  const focus = Math.max(1, bounds ? boundsRadius(bounds) : 1);
  const target = position.clone().add(direction.multiplyScalar(focus));

  return {
    id,
    name: camera.name || "Imported Camera",
    position: [position.x, position.y, position.z],
    target: [target.x, target.y, target.z],
    up: [camera.up.x, camera.up.y, camera.up.z],
    fovDegrees: camera.fov,
  };
}

function boundsCenter(bounds: SceneBounds): Vec3 {
  return [
    (bounds.min[0] + bounds.max[0]) * 0.5,
    (bounds.min[1] + bounds.max[1]) * 0.5,
    (bounds.min[2] + bounds.max[2]) * 0.5,
  ];
}

function boundsRadius(bounds: SceneBounds) {
  const dx = bounds.max[0] - bounds.min[0];
  const dy = bounds.max[1] - bounds.min[1];
  const dz = bounds.max[2] - bounds.min[2];
  return Math.max(Math.hypot(dx, dy, dz) * 0.5, 0.5);
}

function transformFromMatrix(matrix: THREE.Matrix4): Transform {
  const translation = new THREE.Vector3();
  const rotation = new THREE.Quaternion();
  const scale = new THREE.Vector3();
  matrix.decompose(translation, rotation, scale);

  return {
    translation: [translation.x, translation.y, translation.z],
    rotation: [rotation.x, rotation.y, rotation.z, rotation.w],
    scale: [scale.x, scale.y, scale.z],
  };
}

function matrixFromTransform(transform: Transform) {
  return new THREE.Matrix4().compose(
    new THREE.Vector3(...transform.translation),
    new THREE.Quaternion(...transform.rotation),
    new THREE.Vector3(...transform.scale),
  );
}

function meshNodeName(name: string, groupCount: number, index: number) {
  if (groupCount <= 1) {
    return name || `Mesh ${index + 1}`;
  }

  return `${name || "Mesh"} primitive ${index + 1}`;
}

function isMesh(object: THREE.Object3D): object is THREE.Mesh {
  return (object as THREE.Mesh).isMesh === true;
}

function isPerspectiveCamera(object: THREE.Object3D): object is THREE.PerspectiveCamera {
  return (object as THREE.PerspectiveCamera).isPerspectiveCamera === true;
}

function finiteOr(value: unknown, fallback: number) {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function slug(value: string | undefined, fallback: string) {
  const normalized = value
    ?.toLowerCase()
    .replace(/\.[^.]+$/u, "")
    .replace(/[^a-z0-9]+/gu, "-")
    .replace(/^-|-$/gu, "");

  return normalized || fallback;
}
