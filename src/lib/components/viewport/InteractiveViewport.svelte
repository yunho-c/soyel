<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import * as THREE from "three";
  import { OrbitControls } from "three/addons/controls/OrbitControls.js";

  import type { AppliedMaterial } from "$lib/materials";
  import {
    getActiveCamera,
    getRenderableNodes,
    getSceneNodeMaterial,
    materialPreviewColor,
    type CameraState,
    type MeshAsset,
    type SceneNode,
    type ViewportScene,
  } from "$lib/viewport-scene";

  type Props = {
    viewportScene: ViewportScene;
    selectedSurfaceId?: string;
    appliedMaterials?: AppliedMaterial[];
    isRendering?: boolean;
    isPathTraceStale?: boolean;
    rasterFillOpacity?: number;
    rasterGuideOpacity?: number;
    onCameraChange?: (camera: CameraState, active: boolean) => void;
    onInteractionChange?: (active: boolean) => void;
    onSelectSurface?: (id: string, label: string) => void;
  };

  let {
    viewportScene,
    selectedSurfaceId,
    appliedMaterials = [],
    isRendering = false,
    isPathTraceStale = false,
    rasterFillOpacity = 1,
    rasterGuideOpacity = 1,
    onCameraChange,
    onInteractionChange,
    onSelectSurface,
  }: Props = $props();

  let container = $state<HTMLDivElement | null>(null);
  let hoverId = $state<string | null>(null);
  let isInteracting = $state(false);

  let renderer: THREE.WebGLRenderer | null = null;
  let scene: THREE.Scene | null = null;
  let camera: THREE.PerspectiveCamera | null = null;
  let controls: OrbitControls | null = null;
  let resizeObserver: ResizeObserver | null = null;
  let animationFrame = 0;
  let applyingCamera = false;
  let rasterGuides: THREE.Object3D[] = [];

  const raycaster = new THREE.Raycaster();
  const pointer = new THREE.Vector2();
  const meshRecords = new Map<
    string,
    {
      node: SceneNode;
      meshAsset: MeshAsset;
      group: THREE.Group;
      mesh: THREE.Mesh;
      material: THREE.MeshStandardMaterial;
      outline: THREE.LineSegments;
      outlineMaterial: THREE.LineBasicMaterial;
    }
  >();

  let pointerDown: { x: number; y: number } | null = null;

  $effect(() => {
    if (!camera || !controls) {
      return;
    }

    applyCameraState(getActiveCamera(viewportScene));
  });

  $effect(() => {
    updateSceneObjects();
  });

  $effect(() => {
    updateObjectPresentation();
  });

  onMount(() => {
    if (!container) {
      return;
    }

    scene = new THREE.Scene();
    camera = new THREE.PerspectiveCamera(getActiveCamera(viewportScene).fovDegrees, 1, 0.02, 100);
    renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
    renderer.setClearColor(0x000000, 0);
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    renderer.domElement.className = "absolute inset-0 size-full";
    renderer.domElement.setAttribute("aria-label", "Interactive raster viewport");
    container.appendChild(renderer.domElement);

    const ambient = new THREE.AmbientLight(0xffffff, 1.45);
    scene.add(ambient);

    const key = new THREE.DirectionalLight(0xfff2d6, 3.4);
    key.position.set(-1.4, 2.6, -2.2);
    scene.add(key);

    const fill = new THREE.DirectionalLight(0xb8d7ff, 1.2);
    fill.position.set(2.2, 1.4, 2.8);
    scene.add(fill);

    const grid = new THREE.GridHelper(3.2, 16, 0x526057, 0x28312d);
    grid.position.y = -0.002;
    rasterGuides = [grid];
    scene.add(grid);

    const axes = new THREE.AxesHelper(0.55);
    axes.position.set(-1.35, 0.025, -1.12);
    rasterGuides = [...rasterGuides, axes];
    scene.add(axes);

    controls = new OrbitControls(camera, renderer.domElement);
    controls.enableDamping = true;
    controls.dampingFactor = 0.08;
    controls.minDistance = 1.45;
    controls.maxDistance = 6.5;
    controls.screenSpacePanning = true;
    controls.addEventListener("start", handleControlsStart);
    controls.addEventListener("change", handleControlsChange);
    controls.addEventListener("end", handleControlsEnd);

    renderer.domElement.addEventListener("pointerdown", handlePointerDown);
    renderer.domElement.addEventListener("pointermove", handlePointerMove);
    renderer.domElement.addEventListener("pointerup", handlePointerUp);
    renderer.domElement.addEventListener("pointerleave", handlePointerLeave);
    renderer.domElement.addEventListener("dblclick", handleDoubleClick);

    updateSceneObjects();
    applyCameraState(getActiveCamera(viewportScene));
    resizeToContainer();
    resizeObserver = new ResizeObserver(resizeToContainer);
    resizeObserver.observe(container);
    animate();
  });

  onDestroy(() => {
    cancelAnimationFrame(animationFrame);
    resizeObserver?.disconnect();

    if (renderer) {
      renderer.domElement.removeEventListener("pointerdown", handlePointerDown);
      renderer.domElement.removeEventListener("pointermove", handlePointerMove);
      renderer.domElement.removeEventListener("pointerup", handlePointerUp);
      renderer.domElement.removeEventListener("pointerleave", handlePointerLeave);
      renderer.domElement.removeEventListener("dblclick", handleDoubleClick);
    }

    controls?.removeEventListener("start", handleControlsStart);
    controls?.removeEventListener("change", handleControlsChange);
    controls?.removeEventListener("end", handleControlsEnd);
    controls?.dispose();

    for (const record of meshRecords.values()) {
      record.mesh.geometry.dispose();
      record.material.dispose();
      record.outline.geometry.dispose();
      record.outlineMaterial.dispose();
    }

    renderer?.dispose();
    renderer?.domElement.remove();
  });

  function animate() {
    animationFrame = requestAnimationFrame(animate);
    controls?.update();

    if (renderer && scene && camera) {
      renderer.render(scene, camera);
    }
  }

  function updateSceneObjects() {
    if (!scene) {
      return;
    }

    const renderableNodes = getRenderableNodes(viewportScene);
    const liveIds = new Set(renderableNodes.map((node) => node.id));
    for (const [id, record] of meshRecords) {
      if (!liveIds.has(id)) {
        disposeRecord(record);
        meshRecords.delete(id);
      }
    }

    for (const node of renderableNodes) {
      const meshAsset = viewportScene.meshes[node.meshId as string];
      let record = meshRecords.get(node.id);
      if (record && record.meshAsset.id !== meshAsset.id) {
        disposeRecord(record);
        meshRecords.delete(node.id);
        record = undefined;
      }

      if (!record) {
        record = createObjectRecord(node, meshAsset);
        meshRecords.set(node.id, record);
        scene.add(record.group);
      }

      record.node = node;
      record.meshAsset = meshAsset;
      record.group.visible = node.visible;
      record.group.position.set(...node.transform.translation);
      record.group.quaternion.set(...node.transform.rotation);
      record.group.scale.set(...node.transform.scale);
    }
  }

  function updateObjectPresentation() {
    const appliedBySurface = new Map(appliedMaterials.map((material) => [material.surfaceId, material]));
    const fillOpacity = clamp01(rasterFillOpacity);
    const materialWrites = fillOpacity > 0.01;
    const guideOpacity = clamp01(rasterGuideOpacity);
    const showGuides = guideOpacity > 0.01;

    for (const guide of rasterGuides) {
      guide.visible = showGuides;
    }

    for (const [id, record] of meshRecords) {
      const applied = appliedBySurface.get(id);
      const materialAsset = getSceneNodeMaterial(viewportScene, record.node);
      const baseColor = applied
        ? materialPreviewColor(applied.materialName)
        : materialAsset
          ? colorToCss(materialAsset.baseColor)
          : "#9aa39b";
      const selected = selectedSurfaceId === id || viewportScene.selection === id;
      const hovered = hoverId === id;

      record.material.color.set(baseColor);
      record.material.emissive.set(hovered ? "#18251d" : "#000000");
      record.material.opacity = (selected ? 0.95 : 0.76) * fillOpacity;
      record.material.colorWrite = materialWrites;
      record.material.depthWrite = materialWrites;
      record.material.metalness = materialAsset?.metallic ?? 0.04;
      record.material.roughness = materialAsset?.roughness ?? 0.72;
      record.outline.visible = selected || hovered;
      record.outlineMaterial.color.set(selected ? "#7ff0b2" : "#f6d16b");
    }
  }

  function clamp01(value: number) {
    return Math.max(0, Math.min(1, Number.isFinite(value) ? value : 1));
  }

  function colorToCss(color: [number, number, number, number]) {
    return `rgb(${Math.round(color[0] * 255)} ${Math.round(color[1] * 255)} ${Math.round(color[2] * 255)})`;
  }

  function createObjectRecord(node: SceneNode, meshAsset: MeshAsset) {
    const materialAsset = getSceneNodeMaterial(viewportScene, node);
    const material = new THREE.MeshStandardMaterial({
      color: materialAsset ? colorToCss(materialAsset.baseColor) : "#9aa39b",
      side: THREE.DoubleSide,
      transparent: true,
      opacity: 0.76,
      roughness: materialAsset?.roughness ?? 0.72,
      metalness: materialAsset?.metallic ?? 0.04,
    });
    const geometry = createProxyGeometry(meshAsset);
    const mesh = new THREE.Mesh(geometry, material);
    mesh.userData = { surfaceId: node.id, label: node.name };

    const outlineMaterial = new THREE.LineBasicMaterial({
      color: "#7ff0b2",
      transparent: true,
      opacity: 0.95,
      depthTest: false,
    });
    const outline = new THREE.LineSegments(new THREE.EdgesGeometry(geometry), outlineMaterial);
    outline.userData = mesh.userData;
    outline.renderOrder = 10;
    outline.visible = false;

    const group = new THREE.Group();
    group.add(mesh);
    group.add(outline);

    return { node, meshAsset, group, mesh, material, outline, outlineMaterial };
  }

  function createProxyGeometry(meshAsset: MeshAsset) {
    const primitive = meshAsset.source.primitive;
    switch (primitive.type) {
      case "plane":
        return new THREE.PlaneGeometry(primitive.size[0], primitive.size[1]);
      case "box":
        return new THREE.BoxGeometry(primitive.size[0], primitive.size[1], primitive.size[2]);
    }
  }

  function disposeRecord(record: {
    group: THREE.Group;
    mesh: THREE.Mesh;
    material: THREE.MeshStandardMaterial;
    outline: THREE.LineSegments;
    outlineMaterial: THREE.LineBasicMaterial;
  }) {
    scene?.remove(record.group);
    record.mesh.geometry.dispose();
    record.material.dispose();
    record.outline.geometry.dispose();
    record.outlineMaterial.dispose();
  }

  function applyCameraState(state: CameraState) {
    if (!camera || !controls) {
      return;
    }

    applyingCamera = true;
    camera.position.set(...state.position);
    camera.up.set(...state.up);
    camera.fov = state.fovDegrees;
    camera.updateProjectionMatrix();
    controls.target.set(...state.target);
    controls.update();
    applyingCamera = false;
  }

  function emitCamera(active: boolean) {
    if (!camera || !controls || applyingCamera) {
      return;
    }

    onCameraChange?.(
      {
        position: [camera.position.x, camera.position.y, camera.position.z],
        target: [controls.target.x, controls.target.y, controls.target.z],
        up: [camera.up.x, camera.up.y, camera.up.z],
        fovDegrees: camera.fov,
      },
      active,
    );
  }

  function handleControlsStart() {
    isInteracting = true;
    onInteractionChange?.(true);
  }

  function handleControlsChange() {
    emitCamera(isInteracting);
  }

  function handleControlsEnd() {
    emitCamera(false);
    isInteracting = false;
    onInteractionChange?.(false);
  }

  function handlePointerDown(event: PointerEvent) {
    pointerDown = { x: event.clientX, y: event.clientY };
  }

  function handlePointerMove(event: PointerEvent) {
    const hit = pickSurface(event);
    hoverId = hit?.id ?? null;
  }

  function handlePointerUp(event: PointerEvent) {
    if (!pointerDown) {
      return;
    }

    const movement = Math.hypot(event.clientX - pointerDown.x, event.clientY - pointerDown.y);
    pointerDown = null;
    if (movement > 5) {
      return;
    }

    const hit = pickSurface(event);
    if (hit) {
      onSelectSurface?.(hit.id, hit.label);
    }
  }

  function handlePointerLeave() {
    hoverId = null;
    pointerDown = null;
  }

  function handleDoubleClick(event: MouseEvent) {
    const hit = pickSurface(event);
    if (!hit || !camera || !controls) {
      return;
    }

    const record = meshRecords.get(hit.id);
    if (!record) {
      return;
    }

    const target = new THREE.Vector3();
    record.group.getWorldPosition(target);
    const offset = camera.position.clone().sub(controls.target);
    controls.target.copy(target);
    camera.position.copy(target.clone().add(offset));
    controls.update();
    emitCamera(false);
  }

  function pickSurface(event: MouseEvent | PointerEvent) {
    if (!renderer || !camera) {
      return null;
    }

    const rect = renderer.domElement.getBoundingClientRect();
    pointer.x = ((event.clientX - rect.left) / rect.width) * 2 - 1;
    pointer.y = -(((event.clientY - rect.top) / rect.height) * 2 - 1);
    raycaster.setFromCamera(pointer, camera);

    const meshes = Array.from(meshRecords.values()).map((record) => record.mesh);
    const hit = raycaster.intersectObjects(meshes, false)[0];
    if (!hit) {
      return null;
    }

    return {
      id: hit.object.userData.surfaceId as string,
      label: hit.object.userData.label as string,
    };
  }

  function resizeToContainer() {
    if (!container || !renderer || !camera) {
      return;
    }

    const width = Math.max(1, container.clientWidth);
    const height = Math.max(1, container.clientHeight);
    renderer.setSize(width, height, false);
    camera.aspect = width / height;
    camera.updateProjectionMatrix();
  }
</script>

<div
  bind:this={container}
  class="absolute inset-0 overflow-hidden"
  class:cursor-grabbing={isInteracting}
  class:cursor-grab={!isInteracting}
>
  <div class="pointer-events-none absolute left-3 top-3 z-10 flex items-center gap-2">
    <div class="rounded-md border border-white/15 bg-black/35 px-2.5 py-1.5 text-[0.68rem] font-medium uppercase text-white/75 backdrop-blur">
      {#if isRendering}
        Rendering
      {:else if isPathTraceStale}
        Raster preview
      {:else}
        Path traced
      {/if}
    </div>
    {#if hoverId}
      <div class="rounded-md border border-white/15 bg-black/35 px-2.5 py-1.5 text-[0.68rem] text-white/70 backdrop-blur">
        {meshRecords.get(hoverId)?.node.name}
      </div>
    {/if}
  </div>
</div>
