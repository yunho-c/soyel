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
  meshId:
    | "cornellFloor"
    | "cornellCeiling"
    | "cornellBackWall"
    | "cornellLeftWall"
    | "cornellRightWall"
    | "shortBlock"
    | "tallBlock"
    | "light";
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
  position: [0, 0.95, -3.35],
  target: [0, 0.82, 0.1],
  up: [0, 1, 0],
  fovDegrees: 42,
};

export function createDefaultViewportScene(): ViewportScene {
  return {
    revision: 1,
    camera: { ...defaultCameraState },
    selection: "sample-block",
    objects: [
      {
        id: "sample-block",
        label: "Sample Block",
        meshId: "shortBlock",
        transform: identityTransform(),
        visible: true,
      },
      {
        id: "tall-block",
        label: "Tall Block",
        meshId: "tallBlock",
        transform: identityTransform(),
        visible: true,
      },
      {
        id: "left-wall",
        label: "Left Wall",
        meshId: "cornellLeftWall",
        transform: identityTransform(),
        visible: true,
      },
      {
        id: "right-wall",
        label: "Right Wall",
        meshId: "cornellRightWall",
        transform: identityTransform(),
        visible: true,
      },
      {
        id: "back-wall",
        label: "Back Wall",
        meshId: "cornellBackWall",
        transform: identityTransform(),
        visible: true,
      },
      {
        id: "floor",
        label: "Floor",
        meshId: "cornellFloor",
        transform: identityTransform(),
        visible: true,
      },
      {
        id: "ceiling",
        label: "Ceiling",
        meshId: "cornellCeiling",
        transform: identityTransform(),
        visible: true,
      },
      {
        id: "area-light",
        label: "Area Light",
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
