<script lang="ts">
  import BoxIcon from "lucide-svelte/icons/box";
  import BracesIcon from "lucide-svelte/icons/braces";
  import ChevronsDownUpIcon from "lucide-svelte/icons/chevrons-down-up";
  import ChevronsLeftRightIcon from "lucide-svelte/icons/chevrons-left-right";
  import ChevronsRightLeftIcon from "lucide-svelte/icons/chevrons-right-left";
  import CircleIcon from "lucide-svelte/icons/circle";
  import CircleDotDashedIcon from "lucide-svelte/icons/circle-dot-dashed";
  import CuboidIcon from "lucide-svelte/icons/cuboid";
  import EyeIcon from "lucide-svelte/icons/eye";
  import FileStackIcon from "lucide-svelte/icons/file-stack";
  import FolderTreeIcon from "lucide-svelte/icons/folder-tree";
  import GaugeIcon from "lucide-svelte/icons/gauge";
  import Grid3X3Icon from "lucide-svelte/icons/grid-3x3";
  import ImageIcon from "lucide-svelte/icons/image";
  import Layers3Icon from "lucide-svelte/icons/layers-3";
  import PanelBottomCloseIcon from "lucide-svelte/icons/panel-bottom-close";
  import PanelBottomOpenIcon from "lucide-svelte/icons/panel-bottom-open";
  import PanelLeftCloseIcon from "lucide-svelte/icons/panel-left-close";
  import PanelLeftOpenIcon from "lucide-svelte/icons/panel-left-open";
  import PanelRightCloseIcon from "lucide-svelte/icons/panel-right-close";
  import PanelRightOpenIcon from "lucide-svelte/icons/panel-right-open";
  import PlayIcon from "lucide-svelte/icons/play";
  import SlidersHorizontalIcon from "lucide-svelte/icons/sliders-horizontal";
  import SparklesIcon from "lucide-svelte/icons/sparkles";
  import SquareStackIcon from "lucide-svelte/icons/square-stack";
  import TerminalIcon from "lucide-svelte/icons/terminal";
  import TimerIcon from "lucide-svelte/icons/timer";

  import { Button } from "$lib/components/ui/button";
  import * as Resizable from "$lib/components/ui/resizable";
  import {
    defaultWorkspaceLayout,
    loadWorkspaceLayout,
    saveWorkspaceLayout,
    type WorkspaceActivity,
    type WorkspacePanel,
  } from "$lib/workspace-layout";

  let layout = $state(defaultWorkspaceLayout);

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

  const sceneItems = [
    { name: "Studio Assembly", meta: "Root", icon: Layers3Icon },
    { name: "Chair Shell", meta: "Mesh", icon: CuboidIcon },
    { name: "Aluminum Base", meta: "Instance", icon: BoxIcon },
    { name: "Key Softbox", meta: "Area Light", icon: CircleIcon },
  ];

  const inspectorRows = [
    ["Material", "Brushed aluminum"],
    ["Roughness", "0.34"],
    ["Anisotropy", "0.68"],
    ["Samples", "256 spp"],
    ["Device", "GPU path tracer"],
  ];

  $effect(() => {
    layout = loadWorkspaceLayout();
  });

  $effect(() => {
    saveWorkspaceLayout(layout);
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
</script>

<main class="flex h-screen min-h-0 flex-col overflow-hidden bg-background text-foreground">
  <div class="flex h-10 shrink-0 items-center justify-between border-b bg-card px-2">
    <div class="flex items-center gap-2">
      <div class="flex size-7 items-center justify-center rounded-md bg-primary text-primary-foreground">
        <CuboidIcon class="size-4" />
      </div>
      <div class="flex flex-col leading-none">
        <span class="text-sm font-semibold">soyel</span>
        <span class="text-[0.68rem] text-muted-foreground">Industrial renderer workspace</span>
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

    <Resizable.PaneGroup
      direction="horizontal"
      autoSaveId="soyel-workspace-columns"
      class="min-w-0 flex-1"
    >
      {#if layout.leftVisible}
        <Resizable.Pane
          id="left-sidebar"
          order={1}
          defaultSize={20}
          minSize={14}
          maxSize={32}
          class="min-w-56"
        >
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
              <div class="rounded-md border bg-background">
                {#each sceneItems as item}
                  {@const ItemIcon = item.icon}
                  <button
                    class="flex w-full items-center gap-2 border-b px-3 py-2 text-left text-sm last:border-b-0 hover:bg-muted"
                  >
                    <ItemIcon class="size-4 text-muted-foreground" />
                    <span class="min-w-0 flex-1 truncate">{item.name}</span>
                    <span class="text-[0.68rem] text-muted-foreground">{item.meta}</span>
                  </button>
                {/each}
              </div>

              <div class="grid gap-2 text-xs text-muted-foreground">
                <div class="flex items-center justify-between rounded-md border bg-background px-3 py-2">
                  <span>Texture memory</span>
                  <span class="font-medium text-foreground">1.8 GB</span>
                </div>
                <div class="flex items-center justify-between rounded-md border bg-background px-3 py-2">
                  <span>Active camera</span>
                  <span class="font-medium text-foreground">Camera 01</span>
                </div>
              </div>
            </div>
          </aside>
        </Resizable.Pane>
        <Resizable.Handle />
      {/if}

      <Resizable.Pane id="workspace" order={2} defaultSize={56} minSize={32}>
        <Resizable.PaneGroup
          direction="vertical"
          autoSaveId="soyel-workspace-rows"
          class="bg-background"
        >
          <Resizable.Pane id="viewport" order={1} defaultSize={layout.bottomVisible ? 70 : 100} minSize={40}>
            <section class="flex h-full min-h-0 flex-col">
              <div class="flex h-10 shrink-0 items-center justify-between border-b bg-card px-3">
                <div class="flex items-center gap-1">
                  <Button variant="secondary" size="sm">
                    <PlayIcon data-icon="inline-start" />
                    Preview
                  </Button>
                  <Button variant="ghost" size="icon-sm" title="Viewport overlays" aria-label="Viewport overlays">
                    <Grid3X3Icon data-icon="inline-start" />
                  </Button>
                  <Button variant="ghost" size="icon-sm" title="Display settings" aria-label="Display settings">
                    <EyeIcon data-icon="inline-start" />
                  </Button>
                </div>
                <div class="flex items-center gap-2 text-xs text-muted-foreground">
                  <GaugeIcon class="size-4" />
                  <span>Path traced preview</span>
                </div>
              </div>

              <div class="relative min-h-0 flex-1 overflow-hidden bg-[#151816]">
                <div class="absolute inset-0 opacity-45 [background-image:linear-gradient(rgba(255,255,255,.055)_1px,transparent_1px),linear-gradient(90deg,rgba(255,255,255,.055)_1px,transparent_1px)] [background-size:32px_32px]"></div>
                <div class="absolute inset-6 rounded-md border border-white/10 bg-[radial-gradient(circle_at_50%_30%,rgba(107,143,111,.35),transparent_32%),linear-gradient(145deg,rgba(255,255,255,.08),rgba(255,255,255,.01))] shadow-2xl"></div>
                <div class="absolute left-6 top-6 rounded-md border border-white/10 bg-black/30 px-3 py-2 text-xs text-white/70 backdrop-blur">
                  1920 x 1080 · ACES · 256 spp
                </div>
                <div class="absolute bottom-6 right-6 flex items-center gap-2 rounded-md border border-white/10 bg-black/30 px-3 py-2 text-xs text-white/70 backdrop-blur">
                  <CircleDotDashedIcon class="size-4 text-emerald-300" />
                  Render preview ready
                </div>
              </div>
            </section>
          </Resizable.Pane>

          {#if layout.bottomVisible}
            <Resizable.Handle />
            <Resizable.Pane
              id="bottom-panel"
              order={2}
              defaultSize={30}
              minSize={16}
              maxSize={48}
              class="min-h-36"
            >
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
                    <p><span class="text-foreground">[renderer]</span> initialized device queue</p>
                    <p><span class="text-foreground">[scene]</span> loaded 4 objects, 2 lights, 6 materials</p>
                    <p><span class="text-foreground">[preview]</span> converged to 256 samples</p>
                  {:else if layout.activePanel === "render-log"}
                    <p>10:42:16 build acceleration structure</p>
                    <p>10:42:17 compile material graph: brushed_aluminum</p>
                    <p>10:42:18 denoise preview tile cache</p>
                  {:else}
                    <p>Queued jobs will appear here.</p>
                  {/if}
                </div>
              </section>
            </Resizable.Pane>
          {/if}
        </Resizable.PaneGroup>
      </Resizable.Pane>

      {#if layout.rightVisible}
        <Resizable.Handle />
        <Resizable.Pane
          id="right-sidebar"
          order={3}
          defaultSize={24}
          minSize={18}
          maxSize={36}
          class="min-w-64"
        >
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
              <div class="rounded-md border bg-background p-3">
                <div class="mb-3 flex items-center gap-2">
                  <SlidersHorizontalIcon class="size-4 text-muted-foreground" />
                  <span class="text-sm font-medium">Selected Surface</span>
                </div>
                <div class="grid gap-2">
                  {#each inspectorRows as row}
                    <div class="flex items-center justify-between gap-3 text-sm">
                      <span class="text-muted-foreground">{row[0]}</span>
                      <span class="truncate font-medium">{row[1]}</span>
                    </div>
                  {/each}
                </div>
              </div>

              <div class="rounded-md border bg-background p-3">
                <div class="mb-3 flex items-center gap-2">
                  <ImageIcon class="size-4 text-muted-foreground" />
                  <span class="text-sm font-medium">Output</span>
                </div>
                <div class="grid gap-2 text-sm text-muted-foreground">
                  <div class="flex justify-between"><span>Resolution</span><span>3840 x 2160</span></div>
                  <div class="flex justify-between"><span>Color</span><span>ACEScg</span></div>
                  <div class="flex justify-between"><span>Denoiser</span><span>Enabled</span></div>
                </div>
              </div>
            </div>
          </aside>
        </Resizable.Pane>
      {/if}
    </Resizable.PaneGroup>
  </div>

  <footer class="flex h-6 shrink-0 items-center justify-between border-t bg-primary px-3 text-[0.72rem] font-medium text-primary-foreground">
    <div class="flex items-center gap-4">
      <span>Renderer: GPU Path Tracer</span>
      <span>Scene: Studio Assembly</span>
      <span>Samples: 256</span>
    </div>
    <div class="flex items-center gap-4">
      <span>ACES</span>
      <span>Ready</span>
    </div>
  </footer>
</main>

