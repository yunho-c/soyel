<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import BoxIcon from "lucide-svelte/icons/box";
  import BracesIcon from "lucide-svelte/icons/braces";
  import CheckIcon from "lucide-svelte/icons/check";
  import ChevronsDownUpIcon from "lucide-svelte/icons/chevrons-down-up";
  import ChevronsLeftRightIcon from "lucide-svelte/icons/chevrons-left-right";
  import ChevronsRightLeftIcon from "lucide-svelte/icons/chevrons-right-left";
  import CircleDotDashedIcon from "lucide-svelte/icons/circle-dot-dashed";
  import CuboidIcon from "lucide-svelte/icons/cuboid";
  import DownloadIcon from "lucide-svelte/icons/download";
  import EyeIcon from "lucide-svelte/icons/eye";
  import FileStackIcon from "lucide-svelte/icons/file-stack";
  import FolderTreeIcon from "lucide-svelte/icons/folder-tree";
  import Grid3X3Icon from "lucide-svelte/icons/grid-3x3";
  import ImageIcon from "lucide-svelte/icons/image";
  import Layers3Icon from "lucide-svelte/icons/layers-3";
  import PackageCheckIcon from "lucide-svelte/icons/package-check";
  import PanelBottomCloseIcon from "lucide-svelte/icons/panel-bottom-close";
  import PanelBottomOpenIcon from "lucide-svelte/icons/panel-bottom-open";
  import PanelLeftCloseIcon from "lucide-svelte/icons/panel-left-close";
  import PanelLeftOpenIcon from "lucide-svelte/icons/panel-left-open";
  import PanelRightCloseIcon from "lucide-svelte/icons/panel-right-close";
  import PanelRightOpenIcon from "lucide-svelte/icons/panel-right-open";
  import PlayIcon from "lucide-svelte/icons/play";
  import RefreshCwIcon from "lucide-svelte/icons/refresh-cw";
  import RotateCcwIcon from "lucide-svelte/icons/rotate-ccw";
  import SearchIcon from "lucide-svelte/icons/search";
  import SlidersHorizontalIcon from "lucide-svelte/icons/sliders-horizontal";
  import SparklesIcon from "lucide-svelte/icons/sparkles";
  import TerminalIcon from "lucide-svelte/icons/terminal";
  import TimerIcon from "lucide-svelte/icons/timer";

  import SampleCountMenu from "$lib/components/SampleCountMenu.svelte";
  import InteractiveViewport from "$lib/components/viewport/InteractiveViewport.svelte";
  import { Button } from "$lib/components/ui/button";
  import * as Resizable from "$lib/components/ui/resizable";
  import * as Select from "$lib/components/ui/select";
  import {
    applyMaterialToSelection,
    downloadPolyHavenMaterial,
    polyHavenMaterialFiles,
    polyHavenTextureCategories,
    rendererStatus,
    searchPolyHavenMaterials,
    selectSceneSurface,
    streamPreviewFrame,
    type DownloadedMaterial,
    type MaterialFile,
    type PolyHavenCategory,
    type PolyHavenMaterial,
    type RenderPreviewFrame,
    type RendererStatus,
  } from "$lib/materials";
  import {
    createDefaultViewportScene,
    defaultCameraState,
    withSceneRevision,
    type CameraState,
  } from "$lib/viewport-scene";
  import {
    loadWorkspaceLayout,
    saveWorkspaceLayout,
    type WorkspaceActivity,
    type WorkspacePanel,
  } from "$lib/workspace-layout";

  let layout = $state(loadWorkspaceLayout());
  let status = $state<RendererStatus | null>(null);
  let viewportScene = $state(createDefaultViewportScene());
  let materials = $state<PolyHavenMaterial[]>([]);
  let materialCategories = $state<PolyHavenCategory[]>([{ id: "all", name: "All", count: 0 }]);
  let selectedMaterial = $state<PolyHavenMaterial | null>(null);
  let selectedFiles = $state<MaterialFile[]>([]);
  let downloaded = $state<DownloadedMaterial | null>(null);
  let previewFrame = $state<RenderPreviewFrame | null>(null);
  let previewCanvas = $state<HTMLCanvasElement | null>(null);
  let previewHost = $state<HTMLDivElement | null>(null);
  let previewSize = $state({ width: 360, height: 260 });
  let query = $state("");
  let category = $state("all");
  let sort = $state("popular");
  let resolution = $state("2k");
  let isLoading = $state(false);
  let isApplying = $state(false);
  let isRendering = $state(false);
  let isViewportInteracting = $state(false);
  let pathTraceStale = $state(true);
  let previewError = $state("");
  let errorMessage = $state("");
  let previewTimer: ReturnType<typeof setTimeout> | null = null;
  let activePreviewRevision = 0;
  let activePreviewRequest = 0;
  const sampleCountPresets = [16, 32, 64, 128, 256, 512, 1024];
  const sampleCountStorageKey = "soyel:rt-sample-counts:v1";
  let previewSamples = $state(128);
  let renderSamples = $state(512);
  let lastScheduledPreviewSamples = $state(128);
  let sampleCountsReady = $state(false);
  type ViewportDisplayMode = "preview" | "ray-traced" | "hybrid";

  const viewportDisplayModes = [
    { value: "preview", label: "Preview" },
    { value: "ray-traced", label: "Ray-Traced" },
    { value: "hybrid", label: "Hybrid" },
  ] satisfies Array<{ value: ViewportDisplayMode; label: string }>;

  let viewportDisplayMode = $state<ViewportDisplayMode>("hybrid");
  let selectedViewportDisplayMode = $derived(
    viewportDisplayModes.find((mode) => mode.value === viewportDisplayMode)?.label ?? "Hybrid",
  );
  let hasPathTracedPixels = $derived(
    previewFrame ? previewFrame.pixels.length === previewFrame.width * previewFrame.height * 4 : false,
  );
  let showPathTracedCanvas = $derived(viewportDisplayMode !== "preview" && hasPathTracedPixels);
  let showRasterViewport = $derived(
    viewportDisplayMode === "preview" ||
      (viewportDisplayMode === "hybrid" && (isViewportInteracting || !hasPathTracedPixels)),
  );
  let showInteractiveViewport = $derived(viewportDisplayMode !== "ray-traced");
  let rasterFillOpacity = $derived(showRasterViewport ? 1 : 0);
  let rasterGuideOpacity = $derived(showRasterViewport ? 1 : 0);
  let showRayTraceNotice = $derived(
    viewportDisplayMode === "ray-traced" && !isRendering && !hasPathTracedPixels,
  );
  let rayTraceNotice = $derived(previewError || "No ray-traced frame");
  let lastRequestedViewportDisplayMode = $state<ViewportDisplayMode>("hybrid");

  const activities = [
    { id: "scene", label: "Scene", icon: FolderTreeIcon },
    { id: "assets", label: "Assets", icon: FileStackIcon },
    { id: "materials", label: "Materials", icon: CircleDotDashedIcon },
    { id: "render", label: "Render", icon: SparklesIcon },
  ] satisfies Array<{
    id: WorkspaceActivity;
    label: string;
    icon: typeof FolderTreeIcon;
  }>;

  const bottomPanels = [
    { id: "console", label: "Console", icon: TerminalIcon },
    { id: "render-log", label: "Render Log", icon: BracesIcon },
    { id: "jobs", label: "Jobs", icon: TimerIcon },
  ] satisfies Array<{
    id: WorkspacePanel;
    label: string;
    icon: typeof TerminalIcon;
  }>;

  const surfaces = [
    { id: "sample-block", label: "Sample Block", meta: "Material target", icon: CuboidIcon },
    { id: "tall-block", label: "Tall Block", meta: "Reference object", icon: BoxIcon },
    { id: "left-wall", label: "Left Wall", meta: "Red wall", icon: Layers3Icon },
    { id: "right-wall", label: "Right Wall", meta: "Green wall", icon: Layers3Icon },
    { id: "back-wall", label: "Back Wall", meta: "Diffuse wall", icon: Layers3Icon },
    { id: "floor", label: "Floor", meta: "Diffuse floor", icon: Grid3X3Icon },
    { id: "ceiling", label: "Ceiling", meta: "Diffuse ceiling", icon: Layers3Icon },
    { id: "area-light", label: "Area Light", meta: "Emitter", icon: CircleDotDashedIcon },
  ];

  const roleLabels: Record<string, string> = {
    baseColor: "Base color",
    roughness: "Roughness",
    normal: "Normal",
    metallic: "Metallic",
    ambientOcclusion: "Occlusion",
    occlusionRoughnessMetallic: "ARM",
    displacement: "Displacement",
    specular: "Specular",
    auxiliary: "Auxiliary",
  };

  $effect(() => {
    saveWorkspaceLayout(layout);
  });

  $effect(() => {
    if (!sampleCountsReady) {
      return;
    }

    saveSampleCounts();
  });

  $effect(() => {
    if (!sampleCountsReady) {
      return;
    }

    const samples = previewSamples;
    if (samples === lastScheduledPreviewSamples) {
      return;
    }

    lastScheduledPreviewSamples = samples;
    if (viewportDisplayMode !== "preview") {
      schedulePreviewFrame(180);
    }
  });

  $effect(() => {
    if (previewCanvas && previewFrame) {
      paintPreviewFrame(previewCanvas, previewFrame);
    }
  });

  $effect(() => {
    if (!previewHost) {
      return;
    }

    const host = previewHost;
    const updatePreviewSize = () => {
      const next = measurePreviewSize(host);
      if (next.width === previewSize.width && next.height === previewSize.height) {
        return;
      }

      previewSize = next;
      if (viewportDisplayMode !== "preview") {
        schedulePreviewFrame(180);
      }
    };

    updatePreviewSize();

    const resizeObserver = new ResizeObserver(updatePreviewSize);
    resizeObserver.observe(host);

    return () => resizeObserver.disconnect();
  });

  $effect(() => {
    const mode = viewportDisplayMode;
    if (mode === lastRequestedViewportDisplayMode) {
      return;
    }

    lastRequestedViewportDisplayMode = mode;
    if (mode !== "preview") {
      schedulePreviewFrame(0);
    }
  });

  onMount(async () => {
    loadSampleCounts();
    sampleCountsReady = true;
    await refreshStatus();
    await loadCategories();
    await loadMaterials();
    await refreshPreviewFrame(viewportScene.revision);
  });

  onDestroy(() => {
    if (previewTimer) {
      clearTimeout(previewTimer);
    }
  });

  function updateLayout(next: Partial<typeof layout>) {
    layout = {
      ...layout,
      ...next,
    };
  }

  function setActivity(activeActivity: WorkspaceActivity) {
    updateLayout({ activeActivity, leftVisible: true });
  }

  function setPanel(activePanel: WorkspacePanel) {
    updateLayout({ activePanel, bottomVisible: true });
  }

  async function refreshStatus() {
    try {
      status = await rendererStatus();
    } catch (error) {
      errorMessage = String(error);
    }
  }

  async function loadMaterials() {
    isLoading = true;
    errorMessage = "";

    try {
      materials = await searchPolyHavenMaterials({
        query,
        category,
        sort,
        limit: 72,
      });

      if (!selectedMaterial || !materials.some((material) => material.id === selectedMaterial?.id)) {
        await selectMaterial(materials[0] ?? null);
      } else {
        await loadFiles(selectedMaterial.id);
      }
    } catch (error) {
      errorMessage = String(error);
    } finally {
      isLoading = false;
    }
  }

  async function loadCategories() {
    try {
      const categories = await polyHavenTextureCategories();
      materialCategories = categories.length ? categories : materialCategories;

      if (!materialCategories.some((item) => item.id === category)) {
        category = "all";
      }
    } catch (error) {
      errorMessage = String(error);
    }
  }

  async function loadFiles(id: string) {
    selectedFiles = [];
    downloaded = null;

    try {
      const result = await polyHavenMaterialFiles(id, resolution);
      resolution = result.resolution;
      selectedFiles = result.files;
    } catch (error) {
      errorMessage = String(error);
    }
  }

  async function selectMaterial(material: PolyHavenMaterial | null) {
    selectedMaterial = material;
    selectedFiles = [];
    downloaded = null;

    if (material) {
      await loadFiles(material.id);
    }
  }

  async function selectSurface(id: string, label: string) {
    try {
      status = await selectSceneSurface(id, label);
      viewportScene = withSceneRevision(viewportScene, { selection: id });
      schedulePreviewFrame(80);
    } catch (error) {
      errorMessage = String(error);
    }
  }

  async function applySelectedMaterial() {
    if (!selectedMaterial) {
      return;
    }

    isApplying = true;
    errorMessage = "";

    try {
      status = await applyMaterialToSelection({
        id: selectedMaterial.id,
        name: selectedMaterial.name,
        thumbnailUrl: selectedMaterial.thumbnailUrl,
        categories: selectedMaterial.categories,
        authors: selectedMaterial.authors,
        maps: selectedFiles,
      });
      schedulePreviewFrame(80);
    } catch (error) {
      errorMessage = String(error);
    } finally {
      isApplying = false;
    }
  }

  async function downloadSelectedMaterial() {
    if (!selectedMaterial) {
      return;
    }

    isApplying = true;
    errorMessage = "";

    try {
      downloaded = await downloadPolyHavenMaterial(selectedMaterial.id, resolution, [
        "baseColor",
        "roughness",
        "normal",
        "metallic",
        "displacement",
      ]);
    } catch (error) {
      errorMessage = String(error);
    } finally {
      isApplying = false;
    }
  }

  function formatResolution(material: PolyHavenMaterial) {
    if (!material.maxResolution?.length) {
      return "Unknown";
    }

    return material.maxResolution.map((value) => `${value / 1024}k`).join(" x ");
  }

  function formatDimensions(material: PolyHavenMaterial) {
    if (!material.dimensionsMm?.length) {
      return "Unscaled";
    }

    return material.dimensionsMm.map((value) => `${(value / 1000).toFixed(1)} m`).join(" x ");
  }

  function formatBytes(value?: number) {
    if (!value) {
      return "size n/a";
    }

    if (value > 1_000_000) {
      return `${(value / 1_000_000).toFixed(1)} MB`;
    }

    return `${Math.round(value / 1000)} KB`;
  }

  function appliedForSurface(surfaceId: string) {
    return status?.appliedMaterials.find((material) => material.surfaceId === surfaceId);
  }

  function schedulePreviewFrame(delayMs = 180) {
    if (previewTimer) {
      clearTimeout(previewTimer);
    }

    pathTraceStale = true;
    const revision = viewportScene.revision;
    previewTimer = setTimeout(() => {
      previewTimer = null;
      void refreshPreviewFrame(revision);
    }, delayMs);
  }

  async function refreshPreviewFrame(revision = viewportScene.revision) {
    const request = activePreviewRequest + 1;
    activePreviewRequest = request;
    isRendering = true;
    errorMessage = "";
    previewError = "";
    pathTraceStale = true;
    activePreviewRevision = revision;

    try {
      await streamPreviewFrame(
        {
          revision,
          width: previewSize.width,
          height: previewSize.height,
          samples: previewSamples,
          camera: viewportScene.camera,
        },
        (frame) => {
          if (request !== activePreviewRequest || frame.revision !== viewportScene.revision) {
            return;
          }

          const surfaceLabel = status?.selectedSurface.label ?? frame.surfaceLabel;
          const materialName = status
            ? appliedForSurface(status.selectedSurface.id)?.materialName
            : frame.materialName;

          previewFrame = {
            ...frame,
            surfaceLabel,
            materialName,
          };
          pathTraceStale = !frame.final;
        },
      );
    } catch (error) {
      if (request === activePreviewRequest) {
        previewError = String(error);
        errorMessage = previewError;
      }
    } finally {
      if (request === activePreviewRequest && activePreviewRevision === revision) {
        isRendering = false;
      }
    }
  }

  function handleViewportCameraChange(camera: CameraState, active: boolean) {
    viewportScene = withSceneRevision(viewportScene, { camera });
    pathTraceStale = true;
    schedulePreviewFrame(active ? 320 : 120);
  }

  function handleViewportInteractionChange(active: boolean) {
    isViewportInteracting = active;
    if (active) {
      pathTraceStale = true;
    }
  }

  function loadSampleCounts() {
    try {
      const saved = localStorage.getItem(sampleCountStorageKey);
      if (!saved) {
        return;
      }

      const parsed = JSON.parse(saved) as { previewSamples?: unknown; renderSamples?: unknown };
      previewSamples = readSampleCount(parsed.previewSamples, previewSamples, 1024);
      renderSamples = readSampleCount(parsed.renderSamples, renderSamples, 4096);
      lastScheduledPreviewSamples = previewSamples;
    } catch {
      // Ignore malformed local settings; defaults are cheap and predictable.
    }
  }

  function saveSampleCounts() {
    try {
      localStorage.setItem(sampleCountStorageKey, JSON.stringify({ previewSamples, renderSamples }));
    } catch {
      // Local storage can be unavailable in tests or restricted webviews.
    }
  }

  function readSampleCount(value: unknown, fallback: number, max: number) {
    return typeof value === "number" && Number.isFinite(value) ? clampSampleCount(value, max) : fallback;
  }

  function clampSampleCount(value: number, max: number) {
    return Math.min(max, Math.max(1, Math.round(value)));
  }

  function resetViewportCamera() {
    viewportScene = withSceneRevision(viewportScene, { camera: { ...defaultCameraState } });
    schedulePreviewFrame(0);
  }

  function measurePreviewSize(host: HTMLDivElement) {
    const rect = host.getBoundingClientRect();
    const cssWidth = Math.max(1, rect.width);
    const cssHeight = Math.max(1, rect.height);
    const maxWidth = 960;
    const maxHeight = 720;
    const scale = Math.min(1, maxWidth / cssWidth, maxHeight / cssHeight);

    return {
      width: Math.round(Math.max(160, cssWidth * scale)),
      height: Math.round(Math.max(120, cssHeight * scale)),
    };
  }

  function paintPreviewFrame(canvas: HTMLCanvasElement, frame: RenderPreviewFrame) {
    canvas.width = frame.width;
    canvas.height = frame.height;

    const context = canvas.getContext("2d");
    if (!context) {
      return;
    }

    if (frame.pixels.length !== frame.width * frame.height * 4) {
      paintBrowserPreview(context, frame);
      return;
    }

    const imageData = context.createImageData(frame.width, frame.height);
    imageData.data.set(frame.pixels);
    context.putImageData(imageData, 0, 0);
  }

  function paintBrowserPreview(context: CanvasRenderingContext2D, frame: RenderPreviewFrame) {
    const gradient = context.createLinearGradient(0, 0, frame.width, frame.height);
    gradient.addColorStop(0, "#8f958b");
    gradient.addColorStop(0.48, "#383d38");
    gradient.addColorStop(1, "#141817");
    context.fillStyle = gradient;
    context.fillRect(0, 0, frame.width, frame.height);

    context.fillStyle = "rgba(255,255,255,0.18)";
    context.beginPath();
    context.ellipse(frame.width * 0.5, frame.height * 0.5, frame.width * 0.26, frame.height * 0.34, -0.28, 0, Math.PI * 2);
    context.fill();

    context.fillStyle = "rgba(0,0,0,0.22)";
    context.beginPath();
    context.ellipse(frame.width * 0.52, frame.height * 0.76, frame.width * 0.34, frame.height * 0.09, 0, 0, Math.PI * 2);
    context.fill();
  }
</script>

<main class="flex h-screen min-h-0 flex-col overflow-hidden bg-background text-foreground">
  <div class="flex h-8 shrink-0 items-center justify-between border-b bg-card px-2">
    <div class="flex min-w-0 flex-1 items-center justify-center gap-2 px-4">
      <div class="hidden h-6 max-w-[46rem] flex-1 items-center gap-1.5 rounded-sm border bg-background px-1.5 lg:flex">
        <SearchIcon class="size-3.5 text-muted-foreground" />
        <input
          bind:value={query}
          class="h-5 min-w-0 flex-1 bg-transparent text-xs leading-none outline-none placeholder:text-muted-foreground"
          placeholder="Search"
          onkeydown={(event) => event.key === "Enter" && loadMaterials()}
        />
        <Button variant="ghost" size="xs" class="h-5 rounded-sm px-1.5 text-[0.7rem]" onclick={loadMaterials}>
          Search
        </Button>
      </div>
    </div>

    <div class="flex items-center gap-1">
      <Button
        variant="ghost"
        size="icon-sm"
        title={layout.leftVisible ? "Hide left sidebar" : "Show left sidebar"}
        aria-label={layout.leftVisible ? "Hide left sidebar" : "Show left sidebar"}
        onclick={() => updateLayout({ leftVisible: !layout.leftVisible })}
      >
        {#if layout.leftVisible}
          <PanelLeftCloseIcon data-icon="inline-start" />
        {:else}
          <PanelLeftOpenIcon data-icon="inline-start" />
        {/if}
      </Button>
      <Button
        variant="ghost"
        size="icon-sm"
        title={layout.bottomVisible ? "Hide bottom panel" : "Show bottom panel"}
        aria-label={layout.bottomVisible ? "Hide bottom panel" : "Show bottom panel"}
        onclick={() => updateLayout({ bottomVisible: !layout.bottomVisible })}
      >
        {#if layout.bottomVisible}
          <PanelBottomCloseIcon data-icon="inline-start" />
        {:else}
          <PanelBottomOpenIcon data-icon="inline-start" />
        {/if}
      </Button>
      <Button
        variant="ghost"
        size="icon-sm"
        title={layout.rightVisible ? "Hide inspector" : "Show inspector"}
        aria-label={layout.rightVisible ? "Hide inspector" : "Show inspector"}
        onclick={() => updateLayout({ rightVisible: !layout.rightVisible })}
      >
        {#if layout.rightVisible}
          <PanelRightCloseIcon data-icon="inline-start" />
        {:else}
          <PanelRightOpenIcon data-icon="inline-start" />
        {/if}
      </Button>
    </div>
  </div>

  <div class="flex min-h-0 flex-1">
    <nav class="flex w-12 shrink-0 flex-col items-center border-r bg-card py-2">
      {#each activities as activity}
        {@const ActivityIcon = activity.icon}
        <Button
          variant={layout.activeActivity === activity.id ? "secondary" : "ghost"}
          size="icon"
          class="mb-1"
          title={activity.label}
          aria-label={activity.label}
          aria-pressed={layout.activeActivity === activity.id}
          onclick={() => setActivity(activity.id)}
        >
          <ActivityIcon data-icon="inline-start" />
        </Button>
      {/each}
    </nav>

    <Resizable.PaneGroup direction="horizontal" autoSaveId="soyel-workspace-columns" class="min-w-0 flex-1">
      {#if layout.leftVisible}
        <Resizable.Pane id="left-sidebar" order={1} defaultSize={24} minSize={18} maxSize={34} class="min-w-72">
          <aside class="flex h-full flex-col bg-card">
            <div class="flex h-10 items-center justify-between border-b px-3">
              <span class="text-xs font-semibold uppercase tracking-normal text-muted-foreground">
                {activities.find((activity) => activity.id === layout.activeActivity)?.label}
              </span>
              <Button
                variant="ghost"
                size="icon-xs"
                title="Hide left sidebar"
                aria-label="Hide left sidebar"
                onclick={() => updateLayout({ leftVisible: false })}
              >
                <ChevronsRightLeftIcon data-icon="inline-start" />
              </Button>
            </div>

            <div class="flex min-h-0 flex-1 flex-col gap-3 overflow-auto p-3">
              <div class="grid gap-2">
                <div class="flex items-center gap-2 lg:hidden">
                  <div class="flex min-w-0 flex-1 items-center gap-2 rounded-md border bg-background px-2">
                    <SearchIcon class="size-4 text-muted-foreground" />
                    <input
                      bind:value={query}
                      class="h-8 min-w-0 flex-1 bg-transparent text-sm outline-none"
                      placeholder="Search materials"
                      onkeydown={(event) => event.key === "Enter" && loadMaterials()}
                    />
                  </div>
                  <Button variant="secondary" size="icon-sm" onclick={loadMaterials} title="Search" aria-label="Search">
                    <SearchIcon data-icon="inline-start" />
                  </Button>
                </div>

                <div class="grid grid-cols-3 gap-2">
                  <select
                    bind:value={category}
                    class="h-8 rounded-md border bg-background px-2 text-xs outline-none focus:border-ring"
                    onchange={loadMaterials}
                  >
                    {#each materialCategories as item}
                      <option value={item.id}>
                        {item.name}{item.count ? ` (${item.count})` : ""}
                      </option>
                    {/each}
                  </select>
                  <select
                    bind:value={sort}
                    class="h-8 rounded-md border bg-background px-2 text-xs outline-none focus:border-ring"
                    onchange={loadMaterials}
                  >
                    <option value="popular">Popular</option>
                    <option value="latest">Latest</option>
                    <option value="name">Name</option>
                  </select>
                  <select
                    bind:value={resolution}
                    class="h-8 rounded-md border bg-background px-2 text-xs outline-none focus:border-ring"
                    onchange={() => selectedMaterial && loadFiles(selectedMaterial.id)}
                  >
                    <option value="1k">1k</option>
                    <option value="2k">2k</option>
                    <option value="4k">4k</option>
                    <option value="8k">8k</option>
                  </select>
                </div>
              </div>

              {#if isLoading}
                <div class="grid gap-2">
                  {#each Array(8) as _}
                    <div class="h-20 animate-pulse rounded-md border bg-muted"></div>
                  {/each}
                </div>
              {:else if materials.length === 0}
                <div class="rounded-md border bg-background p-4 text-sm text-muted-foreground">
                  No matching materials.
                </div>
              {:else}
                <div class="grid gap-2">
                  {#each materials as material}
                    <button
                      class="grid grid-cols-[4.5rem_minmax(0,1fr)] gap-3 rounded-md border bg-background p-2 text-left transition hover:border-primary/60 hover:bg-muted"
                      class:border-primary={selectedMaterial?.id === material.id}
                      aria-pressed={selectedMaterial?.id === material.id}
                      onclick={() => selectMaterial(material)}
                    >
                      <div class="aspect-square overflow-hidden rounded-sm border bg-muted">
                        {#if material.thumbnailUrl}
                          <img src={material.thumbnailUrl} alt="" class="size-full object-cover" loading="lazy" />
                        {/if}
                      </div>
                      <div class="min-w-0">
                        <div class="flex items-center gap-2">
                          <span class="truncate text-sm font-medium">{material.name}</span>
                          {#if status?.appliedMaterials.some((item) => item.materialId === material.id)}
                            <CheckIcon class="size-3.5 shrink-0 text-primary" />
                          {/if}
                        </div>
                        <div class="mt-1 flex flex-wrap gap-1">
                          {#each material.categories.slice(0, 3) as item}
                            <span class="rounded-sm bg-secondary px-1.5 py-0.5 text-[0.65rem] text-secondary-foreground">
                              {item}
                            </span>
                          {/each}
                        </div>
                        <div class="mt-2 flex items-center justify-between text-[0.68rem] text-muted-foreground">
                          <span>{formatResolution(material)}</span>
                          <span>{formatDimensions(material)}</span>
                        </div>
                      </div>
                    </button>
                  {/each}
                </div>
              {/if}
            </div>
          </aside>
        </Resizable.Pane>
        <Resizable.Handle />
      {/if}

      <Resizable.Pane id="workspace" order={2} defaultSize={52} minSize={34}>
        <Resizable.PaneGroup direction="vertical" autoSaveId="soyel-workspace-rows" class="bg-background">
          <Resizable.Pane id="viewport" order={1} defaultSize={layout.bottomVisible ? 72 : 100} minSize={42}>
            <section class="flex h-full min-h-0 flex-col">
              <div class="flex h-10 shrink-0 items-center justify-between border-b bg-card px-3">
                <div class="flex items-center gap-1">
                  <Button variant="secondary" size="sm" disabled={isRendering} onclick={() => refreshPreviewFrame()}>
                    <PlayIcon data-icon="inline-start" />
                    {isRendering ? "Rendering" : "Preview"}
                  </Button>
                  <Button variant="ghost" size="icon-sm" title="Reset camera" aria-label="Reset camera" onclick={resetViewportCamera}>
                    <RotateCcwIcon data-icon="inline-start" />
                  </Button>
                  <Button variant="ghost" size="icon-sm" title="Viewport overlays" aria-label="Viewport overlays">
                    <Grid3X3Icon data-icon="inline-start" />
                  </Button>
                  <Button variant="ghost" size="icon-sm" title="Display settings" aria-label="Display settings">
                    <EyeIcon data-icon="inline-start" />
                  </Button>
                </div>
                <Select.Root type="single" bind:value={viewportDisplayMode} items={viewportDisplayModes}>
                  <Select.Trigger aria-label="Viewport display mode">
                    {selectedViewportDisplayMode}
                  </Select.Trigger>
                  <Select.Content align="end">
                    {#each viewportDisplayModes as mode}
                      <Select.Item value={mode.value} label={mode.label}>{mode.label}</Select.Item>
                    {/each}
                  </Select.Content>
                </Select.Root>
              </div>

              <div class="relative min-h-0 flex-1 overflow-hidden bg-black">
                <div class="absolute inset-0 opacity-45 [background-image:linear-gradient(rgba(255,255,255,.05)_1px,transparent_1px),linear-gradient(90deg,rgba(255,255,255,.05)_1px,transparent_1px)] [background-size:32px_32px]"></div>
                <div class="absolute inset-x-8 bottom-0 h-1/3 bg-[radial-gradient(ellipse_at_center,rgba(0,0,0,.45),transparent_68%)]"></div>
                <div bind:this={previewHost} class="absolute inset-0 overflow-hidden bg-black">
                  <canvas
                    bind:this={previewCanvas}
                    width={previewFrame?.width ?? previewSize.width}
                    height={previewFrame?.height ?? previewSize.height}
                    class="absolute inset-0 size-full object-cover transition-opacity duration-200"
                    class:opacity-0={!showPathTracedCanvas}
                    class:opacity-55={showPathTracedCanvas && isViewportInteracting}
                    aria-label="Lupin rendered preview"
                  ></canvas>
                  {#if showInteractiveViewport}
                    <InteractiveViewport
                      viewportScene={viewportScene}
                      selectedSurfaceId={status?.selectedSurface.id}
                      appliedMaterials={status?.appliedMaterials ?? []}
                      isRendering={isRendering}
                      isPathTraceStale={pathTraceStale || isViewportInteracting}
                      rasterFillOpacity={rasterFillOpacity}
                      rasterGuideOpacity={rasterGuideOpacity}
                      onCameraChange={handleViewportCameraChange}
                      onInteractionChange={handleViewportInteractionChange}
                      onSelectSurface={selectSurface}
                    />
                  {/if}
                  {#if showRayTraceNotice}
                    <div class="pointer-events-none absolute inset-0 grid place-items-center bg-black/75 px-8 text-center text-sm text-white/70">
                      <div class="max-w-md rounded-md border border-white/10 bg-black/40 px-4 py-3 backdrop-blur">
                        {rayTraceNotice}
                      </div>
                    </div>
                  {/if}
                </div>
              </div>
            </section>
          </Resizable.Pane>

          {#if layout.bottomVisible}
            <Resizable.Handle />
            <Resizable.Pane id="bottom-panel" order={2} defaultSize={28} minSize={18} maxSize={48} class="min-h-40">
              <section class="flex h-full min-h-0 flex-col border-t bg-card">
                <div class="flex h-9 shrink-0 items-center justify-between border-b px-2">
                  <div class="flex items-center gap-1">
                    {#each bottomPanels as panel}
                      {@const PanelIcon = panel.icon}
                      <Button
                        variant={layout.activePanel === panel.id ? "secondary" : "ghost"}
                        size="sm"
                        aria-pressed={layout.activePanel === panel.id}
                        onclick={() => setPanel(panel.id)}
                      >
                        <PanelIcon data-icon="inline-start" />
                        {panel.label}
                      </Button>
                    {/each}
                  </div>
                  <Button
                    variant="ghost"
                    size="icon-xs"
                    title="Hide bottom panel"
                    aria-label="Hide bottom panel"
                    onclick={() => updateLayout({ bottomVisible: false })}
                  >
                    <ChevronsDownUpIcon data-icon="inline-start" />
                  </Button>
                </div>

                <div class="min-h-0 flex-1 overflow-auto p-3 font-mono text-xs leading-6 text-muted-foreground">
                  {#if layout.activePanel === "console"}
                    <p><span class="text-foreground">[renderer]</span> {status?.engineName ?? "LupinPathTracer"} backend online</p>
                    <p><span class="text-foreground">[polyhaven]</span> {materials.length} materials loaded, {selectedFiles.length} texture maps visible</p>
                    {#if downloaded}
                      <p><span class="text-foreground">[download]</span> wrote {downloaded.files.length} files to {downloaded.directory}</p>
                    {/if}
                    {#if errorMessage}
                      <p class="text-destructive">[error] {errorMessage}</p>
                    {/if}
                  {:else if layout.activePanel === "render-log"}
                    {#each status?.notes ?? [] as note}
                      <p>{note}</p>
                    {/each}
                    <p>Material model: {status?.materialModel ?? "loading"}</p>
                  {:else}
                    {#if status?.appliedMaterials.length}
                      {#each status.appliedMaterials as item}
                        <p>{item.surfaceLabel}: {item.materialName} ({item.maps.length} maps)</p>
                      {/each}
                    {:else}
                      <p>No material application jobs yet.</p>
                    {/if}
                  {/if}
                </div>
              </section>
            </Resizable.Pane>
          {/if}
        </Resizable.PaneGroup>
      </Resizable.Pane>

      {#if layout.rightVisible}
        <Resizable.Handle />
        <Resizable.Pane id="right-sidebar" order={3} defaultSize={24} minSize={20} maxSize={36} class="min-w-72">
          <aside class="flex h-full flex-col bg-card">
            <div class="flex h-10 items-center justify-between border-b px-3">
              <span class="text-xs font-semibold uppercase tracking-normal text-muted-foreground">
                Inspector
              </span>
              <Button
                variant="ghost"
                size="icon-xs"
                title="Hide inspector"
                aria-label="Hide inspector"
                onclick={() => updateLayout({ rightVisible: false })}
              >
                <ChevronsLeftRightIcon data-icon="inline-start" />
              </Button>
            </div>

            <div class="flex min-h-0 flex-1 flex-col gap-4 overflow-auto p-3">
              <section class="grid gap-2">
                <div class="flex items-center gap-2">
                  <Layers3Icon class="size-4 text-muted-foreground" />
                  <span class="text-sm font-medium">Scene Surfaces</span>
                </div>
                <div class="grid gap-1">
                  {#each surfaces as surface}
                    {@const SurfaceIcon = surface.icon}
                    {@const applied = appliedForSurface(surface.id)}
                    <button
                      class="flex min-h-11 items-center gap-2 rounded-md border bg-background px-3 py-2 text-left text-sm hover:bg-muted"
                      class:border-primary={status?.selectedSurface.id === surface.id}
                      onclick={() => selectSurface(surface.id, surface.label)}
                    >
                      <SurfaceIcon class="size-4 text-muted-foreground" />
                      <span class="min-w-0 flex-1">
                        <span class="block truncate font-medium">{surface.label}</span>
                        <span class="block truncate text-[0.68rem] text-muted-foreground">
                          {applied?.materialName ?? surface.meta}
                        </span>
                      </span>
                      {#if applied}
                        <PackageCheckIcon class="size-4 text-primary" />
                      {/if}
                    </button>
                  {/each}
                </div>
              </section>

              <section class="grid gap-3">
                <div class="flex items-center justify-between gap-2">
                  <div class="flex items-center gap-2">
                    <SlidersHorizontalIcon class="size-4 text-muted-foreground" />
                    <span class="text-sm font-medium">Material Maps</span>
                  </div>
                  <Button
                    variant="ghost"
                    size="icon-xs"
                    title="Refresh material maps"
                    aria-label="Refresh material maps"
                    onclick={() => selectedMaterial && loadFiles(selectedMaterial.id)}
                  >
                    <RefreshCwIcon data-icon="inline-start" />
                  </Button>
                </div>

                {#if selectedMaterial}
                  <div class="grid gap-2 rounded-md border bg-background p-3">
                    <div class="flex gap-3">
                      <div class="size-16 shrink-0 overflow-hidden rounded-sm border bg-muted">
                        {#if selectedMaterial.thumbnailUrl}
                          <img src={selectedMaterial.thumbnailUrl} alt="" class="size-full object-cover" />
                        {/if}
                      </div>
                      <div class="min-w-0">
                        <div class="truncate text-sm font-semibold">{selectedMaterial.name}</div>
                        <div class="mt-1 text-xs text-muted-foreground">{formatDimensions(selectedMaterial)}</div>
                        <div class="mt-2 flex flex-wrap gap-1">
                          {#each selectedMaterial.categories.slice(0, 3) as item}
                            <span class="rounded-sm bg-secondary px-1.5 py-0.5 text-[0.65rem]">{item}</span>
                          {/each}
                        </div>
                      </div>
                    </div>

                    <div class="grid grid-cols-2 gap-2">
                      <Button variant="secondary" size="sm" disabled={isApplying} onclick={applySelectedMaterial}>
                        <CheckIcon data-icon="inline-start" />
                        Apply
                      </Button>
                      <Button variant="outline" size="sm" disabled={isApplying} onclick={downloadSelectedMaterial}>
                        <DownloadIcon data-icon="inline-start" />
                        Download
                      </Button>
                    </div>
                  </div>

                  <div class="grid gap-1">
                    {#each selectedFiles as file}
                      <div class="flex items-center justify-between gap-2 rounded-md border bg-background px-3 py-2 text-xs">
                        <div class="min-w-0">
                          <div class="truncate font-medium">{roleLabels[file.role] ?? file.role}</div>
                          <div class="truncate text-muted-foreground">{file.map} · {file.format.toUpperCase()}</div>
                        </div>
                        <span class="shrink-0 text-muted-foreground">{formatBytes(file.size)}</span>
                      </div>
                    {/each}
                  </div>
                {:else}
                  <div class="rounded-md border bg-background p-4 text-sm text-muted-foreground">
                    Select a material to inspect its maps.
                  </div>
                {/if}
              </section>

              <section class="grid gap-2">
                <div class="flex items-center gap-2">
                  <ImageIcon class="size-4 text-muted-foreground" />
                  <span class="text-sm font-medium">Renderer</span>
                </div>
                <div class="grid gap-2 rounded-md border bg-background p-3 text-sm">
                  <div class="flex justify-between gap-3">
                    <span class="text-muted-foreground">Engine</span>
                    <span class="truncate font-medium">{status?.engineName ?? "Loading"}</span>
                  </div>
                  <div class="flex justify-between gap-3">
                    <span class="text-muted-foreground">Backend</span>
                    <span class="truncate font-medium">{status?.gpuApi ?? "WGPU"}</span>
                  </div>
                  <div class="flex justify-between gap-3">
                    <span class="text-muted-foreground">Selected</span>
                    <span class="truncate font-medium">{status?.selectedSurface.label ?? "None"}</span>
                  </div>
                </div>
              </section>
            </div>
          </aside>
        </Resizable.Pane>
      {/if}
    </Resizable.PaneGroup>
  </div>

  <footer class="flex h-6 shrink-0 items-center justify-between border-t bg-primary px-3 text-[0.72rem] font-medium text-primary-foreground">
    <div class="flex min-w-0 items-center gap-4">
      <span class="truncate">Surface: {status?.selectedSurface.label ?? "Chair Shell"}</span>
      <span class="truncate">Materials: {materials.length}</span>
    </div>
    <div class="flex shrink-0 items-center gap-1">
      <SampleCountMenu
        label="Preview"
        bind:value={previewSamples}
        presets={sampleCountPresets}
        min={1}
        max={1024}
        step={1}
      />
      <SampleCountMenu
        label="Render"
        bind:value={renderSamples}
        presets={sampleCountPresets}
        min={1}
        max={4096}
        step={1}
      />
      {#if errorMessage}
        <span class="ml-2">Attention</span>
      {/if}
    </div>
  </footer>
</main>
