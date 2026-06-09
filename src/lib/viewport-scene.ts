export type Vec2 = [number, number];
export type Vec3 = [number, number, number];
export type Vec4 = [number, number, number, number];
export type Quat = [number, number, number, number];

export type CameraState = {
  position: Vec3;
  target: Vec3;
  up: Vec3;
  fovDegrees: number;
};

export type SceneCamera = CameraState & {
  id: string;
  name: string;
};

export type Transform = {
  translation: Vec3;
  rotation: Quat;
  scale: Vec3;
};

export type ProceduralMeshPrimitive =
  | { type: "plane"; size: Vec2 }
  | { type: "box"; size: Vec3 };

export type TriangleMeshAsset = {
  positions: number[];
  normals?: number[];
  uvs?: number[];
  indices: number[];
};

export type MeshSource =
  | {
      type: "procedural";
      primitive: ProceduralMeshPrimitive;
    }
  | {
      type: "triangleMesh";
      geometry: TriangleMeshAsset;
    };

export type MeshAsset = {
  id: string;
  name: string;
  source: MeshSource;
  bounds?: {
    min: Vec3;
    max: Vec3;
  };
};

export type TextureRef = {
  uri: string;
  colorSpace?: "srgb" | "linear";
};

export type MaterialAsset = {
  id: string;
  name: string;
  model: "gltf-pbr";
  baseColor: Vec4;
  roughness: number;
  metallic: number;
  emissive: Vec3;
  textureRefs: Record<string, TextureRef>;
};

export type SceneNode = {
  id: string;
  name: string;
  parentId: string | null;
  meshId: string | null;
  materialBindings: Record<string, string>;
  transform: Transform;
  visible: boolean;
  selectable: boolean;
};

export type SoyelScene = {
  schemaVersion: 1;
  revision: number;
  units: "meter";
  cameras: Record<string, SceneCamera>;
  activeCameraId: string;
  nodes: Record<string, SceneNode>;
  meshes: Record<string, MeshAsset>;
  materials: Record<string, MaterialAsset>;
  selection: string | null;
};

export type ViewportScene = SoyelScene;
export type ViewportObject = SceneNode;

export const defaultCameraState: CameraState = {
  position: [0, 0.95, -3.35],
  target: [0, 0.82, 0.1],
  up: [0, 1, 0],
  fovDegrees: 42,
};

export function createDefaultCornellScene(): SoyelScene {
  const camera = {
    id: "camera-main",
    name: "Main Camera",
    ...cloneCamera(defaultCameraState),
  };

  return {
    schemaVersion: 1,
    revision: 1,
    units: "meter",
    cameras: { [camera.id]: camera },
    activeCameraId: camera.id,
    selection: "sample-block",
    meshes: {
      cornellFloor: planeMesh("cornellFloor", "Cornell Floor", [2.7, 2.3]),
      cornellCeiling: planeMesh("cornellCeiling", "Cornell Ceiling", [2.7, 2.3]),
      cornellBackWall: planeMesh("cornellBackWall", "Cornell Back Wall", [2.7, 1.9]),
      cornellLeftWall: planeMesh("cornellLeftWall", "Cornell Left Wall", [2.3, 1.9]),
      cornellRightWall: planeMesh("cornellRightWall", "Cornell Right Wall", [2.3, 1.9]),
      light: planeMesh("light", "Area Light", [0.68, 0.56]),
      shortBlock: boxMesh("shortBlock", "Short Block", [0.62, 0.55, 0.62]),
      tallBlock: boxMesh("tallBlock", "Tall Block", [0.56, 1.1, 0.56]),
    },
    materials: {
      floorMat: pbrMaterial("floorMat", "Warm Diffuse Floor", [0.66, 0.65, 0.59, 1], 0.82, 0),
      ceilingMat: pbrMaterial("ceilingMat", "Warm Diffuse Ceiling", [0.72, 0.71, 0.66, 1], 0.82, 0),
      backWallMat: pbrMaterial("backWallMat", "Warm Back Wall", [0.7, 0.69, 0.64, 1], 0.82, 0),
      leftWallMat: pbrMaterial("leftWallMat", "Red Wall", [0.62, 0.22, 0.18, 1], 0.82, 0),
      rightWallMat: pbrMaterial("rightWallMat", "Green Wall", [0.22, 0.52, 0.32, 1], 0.82, 0),
      sampleBlockMat: pbrMaterial("sampleBlockMat", "Sample Block", [0.68, 0.73, 0.7, 1], 0.46, 0.08),
      tallBlockMat: pbrMaterial("tallBlockMat", "Tall Block", [0.48, 0.5, 0.47, 1], 0.74, 0),
      lightMat: {
        ...pbrMaterial("lightMat", "Area Light", [1, 0.88, 0.58, 1], 0.28, 0),
        emissive: [20, 17, 12],
      },
    },
    nodes: {
      "sample-block": sceneNode("sample-block", "Sample Block", "shortBlock", "sampleBlockMat", {
        translation: [0.48, 0.275, -0.25],
        rotation: quatFromEuler(0, -0.28, 0),
        scale: [1, 1, 1],
      }),
      "tall-block": sceneNode("tall-block", "Tall Block", "tallBlock", "tallBlockMat", {
        translation: [-0.43, 0.55, 0.28],
        rotation: quatFromEuler(0, 0.32, 0),
        scale: [1, 1, 1],
      }),
      "left-wall": sceneNode("left-wall", "Left Wall", "cornellLeftWall", "leftWallMat", {
        translation: [-1.35, 0.95, 0],
        rotation: quatFromEuler(0, Math.PI / 2, 0),
        scale: [1, 1, 1],
      }),
      "right-wall": sceneNode("right-wall", "Right Wall", "cornellRightWall", "rightWallMat", {
        translation: [1.35, 0.95, 0],
        rotation: quatFromEuler(0, -Math.PI / 2, 0),
        scale: [1, 1, 1],
      }),
      "back-wall": sceneNode("back-wall", "Back Wall", "cornellBackWall", "backWallMat", {
        translation: [0, 0.95, 1.15],
        rotation: identityQuat(),
        scale: [1, 1, 1],
      }),
      floor: sceneNode("floor", "Floor", "cornellFloor", "floorMat", {
        translation: [0, 0, 0],
        rotation: quatFromEuler(-Math.PI / 2, 0, 0),
        scale: [1, 1, 1],
      }),
      ceiling: sceneNode("ceiling", "Ceiling", "cornellCeiling", "ceilingMat", {
        translation: [0, 1.9, 0],
        rotation: quatFromEuler(Math.PI / 2, 0, 0),
        scale: [1, 1, 1],
      }),
      "area-light": sceneNode("area-light", "Area Light", "light", "lightMat", {
        translation: [0, 1.88, -0.12],
        rotation: quatFromEuler(Math.PI / 2, 0, 0),
        scale: [1, 1, 1],
      }),
    },
  };
}

export function createDefaultViewportScene(): ViewportScene {
  return createDefaultCornellScene();
}

export function getActiveCamera(scene: SoyelScene): SceneCamera {
  return (
    scene.cameras[scene.activeCameraId] ??
    Object.values(scene.cameras)[0] ?? {
      id: "camera-main",
      name: "Main Camera",
      ...cloneCamera(defaultCameraState),
    }
  );
}

export function withSceneRevision(scene: SoyelScene, patch: Partial<Omit<SoyelScene, "revision">>) {
  return {
    ...scene,
    ...patch,
    revision: scene.revision + 1,
  };
}

export function withActiveCamera(scene: SoyelScene, camera: CameraState) {
  const current = getActiveCamera(scene);
  return withSceneRevision(scene, {
    cameras: {
      ...scene.cameras,
      [current.id]: {
        ...current,
        ...cloneCamera(camera),
      },
    },
  });
}

export function withNodeMaterial(scene: SoyelScene, nodeId: string, material: MaterialAsset) {
  const node = scene.nodes[nodeId];
  if (!node) {
    return scene;
  }

  return withSceneRevision(scene, {
    materials: {
      ...scene.materials,
      [material.id]: material,
    },
    nodes: {
      ...scene.nodes,
      [nodeId]: {
        ...node,
        materialBindings: {
          ...node.materialBindings,
          default: material.id,
        },
      },
    },
  });
}

export function withNodeVisibility(scene: SoyelScene, nodeId: string, visible: boolean) {
  const node = scene.nodes[nodeId];
  if (!node || node.visible === visible) {
    return scene;
  }

  return withSceneRevision(scene, {
    nodes: {
      ...scene.nodes,
      [nodeId]: {
        ...node,
        visible,
      },
    },
  });
}

export function getSceneNodeMaterial(scene: SoyelScene, node: SceneNode) {
  const materialId = node.materialBindings.default;
  return materialId ? scene.materials[materialId] : undefined;
}

export function isNodeEffectivelyVisible(scene: SoyelScene, node: SceneNode) {
  let current: SceneNode | undefined = node;
  const visited = new Set<string>();

  while (current) {
    if (!current.visible) {
      return false;
    }

    if (!current.parentId) {
      return true;
    }

    if (visited.has(current.parentId)) {
      return false;
    }

    visited.add(current.id);
    current = scene.nodes[current.parentId];
  }

  return true;
}

export function getRenderableNodes(scene: SoyelScene) {
  return Object.values(scene.nodes).filter(
    (node) => isNodeEffectivelyVisible(scene, node) && node.meshId && scene.meshes[node.meshId],
  );
}

export function getSelectableNodes(scene: SoyelScene) {
  return Object.values(scene.nodes).filter((node) => node.selectable);
}

export function identityTransform(): Transform {
  return {
    translation: [0, 0, 0],
    rotation: identityQuat(),
    scale: [1, 1, 1],
  };
}

export function materialPreviewColor(name?: string): string {
  const [red, green, blue] = materialPreviewBaseColor(name);
  return `rgb(${Math.round(red * 255)} ${Math.round(green * 255)} ${Math.round(blue * 255)})`;
}

export function materialPreviewBaseColor(name?: string): Vec4 {
  if (!name) {
    return [0.68, 0.73, 0.7, 1];
  }

  let hash = 0;
  for (let index = 0; index < name.length; index += 1) {
    hash = Math.imul(hash, 31) + name.charCodeAt(index);
  }

  const red = 0.38 + ((hash & 0xff) / 255) * 0.34;
  const green = 0.34 + (((hash >> 8) & 0xff) / 255) * 0.34;
  const blue = 0.32 + (((hash >> 16) & 0xff) / 255) * 0.34;

  return [red, green, blue, 1];
}

function planeMesh(id: string, name: string, size: Vec2): MeshAsset {
  return {
    id,
    name,
    source: {
      type: "procedural",
      primitive: { type: "plane", size },
    },
    bounds: {
      min: [-size[0] / 2, -size[1] / 2, 0],
      max: [size[0] / 2, size[1] / 2, 0],
    },
  };
}

function boxMesh(id: string, name: string, size: Vec3): MeshAsset {
  return {
    id,
    name,
    source: {
      type: "procedural",
      primitive: { type: "box", size },
    },
    bounds: {
      min: [-size[0] / 2, -size[1] / 2, -size[2] / 2],
      max: [size[0] / 2, size[1] / 2, size[2] / 2],
    },
  };
}

export function pbrMaterial(
  id: string,
  name: string,
  baseColor: Vec4,
  roughness: number,
  metallic: number,
): MaterialAsset {
  return {
    id,
    name,
    model: "gltf-pbr",
    baseColor,
    roughness,
    metallic,
    emissive: [0, 0, 0],
    textureRefs: {},
  };
}

export function sceneNode(
  id: string,
  name: string,
  meshId: string,
  materialId: string,
  transform: Transform,
): SceneNode {
  return {
    id,
    name,
    parentId: null,
    meshId,
    materialBindings: { default: materialId },
    transform,
    visible: true,
    selectable: true,
  };
}

export function cloneCamera(camera: CameraState): CameraState {
  return {
    position: [...camera.position],
    target: [...camera.target],
    up: [...camera.up],
    fovDegrees: camera.fovDegrees,
  };
}

export function identityQuat(): Quat {
  return [0, 0, 0, 1];
}

export function quatFromEuler(x: number, y: number, z: number): Quat {
  const sx = Math.sin(x * 0.5);
  const cx = Math.cos(x * 0.5);
  const sy = Math.sin(y * 0.5);
  const cy = Math.cos(y * 0.5);
  const sz = Math.sin(z * 0.5);
  const cz = Math.cos(z * 0.5);

  return [
    sx * cy * cz + cx * sy * sz,
    cx * sy * cz - sx * cy * sz,
    cx * cy * sz + sx * sy * cz,
    cx * cy * cz - sx * sy * sz,
  ];
}
