export type Vec3 = [number, number, number];
export type Quat = [number, number, number, number];

export type CameraState = {
  position: Vec3;
  target: Vec3;
  up: Vec3;
  fovDegrees: number;
};

export type Transform = {
  translation: Vec3;
  rotation: Quat;
  scale: Vec3;
};

export type ViewportObject = {
  id: string;
  label: string;
  meshId: "floor" | "wall" | "swatch" | "light";
  materialId?: string;
  transform: Transform;
  visible: boolean;
};

export type ViewportScene = {
  revision: number;
  camera: CameraState;
  objects: ViewportObject[];
  selection: string | null;
};

export const defaultCameraState: CameraState = {
  position: [0, 0.95, -3.1],
  target: [0, 0.68, 0.05],
  up: [0, 1, 0],
  fovDegrees: 42,
};

export function createDefaultViewportScene(): ViewportScene {
  return {
    revision: 1,
    camera: { ...defaultCameraState },
    selection: "chair-shell",
    objects: [
      {
        id: "chair-shell",
        label: "Chair Shell",
        meshId: "swatch",
        transform: identityTransform(),
        visible: true,
      },
      {
        id: "aluminum-base",
        label: "Aluminum Base",
        meshId: "floor",
        transform: identityTransform(),
        visible: true,
      },
      {
        id: "soft-grip",
        label: "Soft Grip",
        meshId: "wall",
        transform: identityTransform(),
        visible: true,
      },
      {
        id: "control-dial",
        label: "Control Dial",
        meshId: "light",
        transform: identityTransform(),
        visible: true,
      },
    ],
  };
}

export function identityTransform(): Transform {
  return {
    translation: [0, 0, 0],
    rotation: [0, 0, 0, 1],
    scale: [1, 1, 1],
  };
}

export function withSceneRevision(scene: ViewportScene, patch: Partial<Omit<ViewportScene, "revision">>) {
  return {
    ...scene,
    ...patch,
    revision: scene.revision + 1,
  };
}

export function materialPreviewColor(name?: string): string {
  if (!name) {
    return "#a8b1aa";
  }

  let hash = 0;
  for (let index = 0; index < name.length; index += 1) {
    hash = Math.imul(hash, 31) + name.charCodeAt(index);
  }

  const red = 0.38 + ((hash & 0xff) / 255) * 0.34;
  const green = 0.34 + (((hash >> 8) & 0xff) / 255) * 0.34;
  const blue = 0.32 + (((hash >> 16) & 0xff) / 255) * 0.34;

  return `rgb(${Math.round(red * 255)} ${Math.round(green * 255)} ${Math.round(blue * 255)})`;
}
