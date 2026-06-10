<script lang="ts">
  import { onMount } from "svelte";
  import { FileTree, preparePresortedFileTreeInput, type ContextMenuItem } from "@pierre/trees";
  import EyeIcon from "lucide-svelte/icons/eye";
  import EyeOffIcon from "lucide-svelte/icons/eye-off";
  import LocateFixedIcon from "lucide-svelte/icons/locate-fixed";

  import { isNodeEffectivelyVisible, type SceneNode, type SoyelScene } from "$lib/viewport-scene";

  type Props = {
    scene: SoyelScene;
    onSelectNode?: (id: string, label: string) => void;
    onToggleVisibility?: (id: string, visible: boolean) => void;
  };

  type SceneTreeRow = {
    id: string;
    path: string;
    node: SceneNode;
    effectiveVisible: boolean;
  };

  let { scene, onSelectNode, onToggleVisibility }: Props = $props();

  let treeHost = $state<HTMLDivElement | null>(null);
  let fileTree = $state<FileTree | null>(null);

  let rows = $derived(buildSceneTreeRows(scene));
  let pathLookup = $derived(new Map(rows.map((row) => [row.path, row])));
  let selectedRow = $derived(rows.find((row) => row.id === scene.selection) ?? null);

  $effect(() => {
    const tree = fileTree;
    const paths = rows.map((row) => row.path);
    const selectedPath = selectedRow?.path ?? null;

    if (!tree) {
      return;
    }

    tree.resetPaths(paths, {
      initialExpandedPaths: paths,
      preparedInput: preparePresortedFileTreeInput(paths),
    });

    if (selectedPath) {
      tree.getItem(selectedPath)?.select();
      tree.scrollToPath(selectedPath, { focus: false, offset: "nearest" });
    }
  });

  onMount(() => {
    if (!treeHost) {
      return;
    }

    const paths = rows.map((row) => row.path);
    const tree = new FileTree({
      id: "soyel-scene-node-tree",
      paths,
      preparedInput: preparePresortedFileTreeInput(paths),
      initialExpansion: "open",
      initialExpandedPaths: paths,
      initialSelectedPaths: selectedRow ? [selectedRow.path] : [],
      itemHeight: 28,
      overscan: 12,
      search: false,
      flattenEmptyDirectories: false,
      onSelectionChange: handleTreeSelection,
      renderRowDecoration: ({ row }) => {
        const item = pathLookup.get(row.path);
        if (!item) {
          return null;
        }

        return item.effectiveVisible ? null : { text: "off", title: "Hidden in preview" };
      },
      composition: {
        contextMenu: {
          enabled: true,
          triggerMode: "button",
          buttonVisibility: "always",
          render: renderVisibilityMenu,
        },
      },
      unsafeCSS: `
        :host {
          --trees-selected-bg-override: color-mix(in oklab, hsl(151 55% 48%) 22%, transparent);
          --trees-border-color-override: hsl(150 7% 22%);
          --trees-fg-override: hsl(120 7% 86%);
          font: 12px ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
        }

        button[data-type='item'] {
          border-radius: 6px;
        }

        button[data-type='item'][data-item-selected] {
          outline: 1px solid color-mix(in oklab, hsl(151 55% 48%) 62%, transparent);
        }
      `,
    });

    tree.render({ containerWrapper: treeHost });
    fileTree = tree;

    return () => {
      tree.cleanUp();
      tree.unmount();
    };
  });

  function handleTreeSelection(paths: readonly string[]) {
    const row = paths.length ? pathLookup.get(paths[paths.length - 1]) : null;
    if (row) {
      onSelectNode?.(row.id, row.node.name);
    }
  }

  function renderVisibilityMenu(item: ContextMenuItem, context: { close: () => void }) {
    const row = pathLookup.get(item.path);
    if (!row) {
      return null;
    }

    const menu = document.createElement("div");
    menu.style.background = "hsl(120 7% 12%)";
    menu.style.border = "1px solid hsl(150 7% 24%)";
    menu.style.borderRadius = "6px";
    menu.style.boxShadow = "0 12px 32px rgb(0 0 0 / 0.32)";
    menu.style.color = "hsl(120 7% 88%)";
    menu.style.minWidth = "128px";
    menu.style.padding = "4px";

    const button = document.createElement("button");
    button.type = "button";
    button.textContent = row.node.visible ? "Hide" : "Show";
    button.style.alignItems = "center";
    button.style.background = "transparent";
    button.style.border = "0";
    button.style.borderRadius = "4px";
    button.style.color = "inherit";
    button.style.cursor = "pointer";
    button.style.display = "flex";
    button.style.font = "inherit";
    button.style.height = "30px";
    button.style.padding = "0 8px";
    button.style.textAlign = "left";
    button.style.width = "100%";
    button.addEventListener("click", (event) => {
      event.preventDefault();
      event.stopPropagation();
      onToggleVisibility?.(row.id, !row.node.visible);
      context.close();
    });

    menu.appendChild(button);
    return menu;
  }

  function toggleSelectedVisibility() {
    if (!selectedRow) {
      return;
    }

    onToggleVisibility?.(selectedRow.id, !selectedRow.node.visible);
  }

  function buildSceneTreeRows(currentScene: SoyelScene) {
    const children = new Map<string | null, SceneNode[]>();
    for (const node of Object.values(currentScene.nodes)) {
      const parentId = node.parentId && currentScene.nodes[node.parentId] ? node.parentId : null;
      const list = children.get(parentId) ?? [];
      list.push(node);
      children.set(parentId, list);
    }

    for (const list of children.values()) {
      list.sort((left, right) => left.name.localeCompare(right.name) || left.id.localeCompare(right.id));
    }

    const nextRows: SceneTreeRow[] = [];
    const visit = (node: SceneNode, parentPath: string | null, siblingNames: Set<string>) => {
      const segment = uniqueSegment(node.name || node.id, siblingNames);
      const path = parentPath ? `${parentPath}/${segment}` : segment;
      nextRows.push({
        id: node.id,
        path,
        node,
        effectiveVisible: isNodeEffectivelyVisible(currentScene, node),
      });

      const childRows = children.get(node.id) ?? [];
      const childNames = new Set<string>();
      for (const child of childRows) {
        visit(child, path, childNames);
      }
    };

    const rootNames = new Set<string>();
    for (const node of children.get(null) ?? []) {
      visit(node, null, rootNames);
    }

    return nextRows;
  }

  function uniqueSegment(name: string, used: Set<string>) {
    const base = name.replace(/\//gu, ":").trim() || "Node";
    let candidate = base;
    let suffix = 2;
    while (used.has(candidate)) {
      candidate = `${base} (${suffix})`;
      suffix += 1;
    }

    used.add(candidate);
    return candidate;
  }
</script>

<div class="flex h-full min-h-0 flex-1 flex-col gap-3">
  <div class="min-h-0 flex-1 overflow-hidden rounded-md border bg-background">
    <div bind:this={treeHost} class="h-full min-h-0"></div>
  </div>

  <div class="shrink-0 rounded-md border bg-background p-3">
    {#if selectedRow}
      <div class="flex items-center justify-between gap-3">
        <div class="min-w-0">
          <div class="flex items-center gap-2 text-sm font-medium">
            <LocateFixedIcon class="size-4 text-muted-foreground" />
            <span class="truncate">{selectedRow.node.name}</span>
          </div>
          <div class="mt-1 truncate text-xs text-muted-foreground">
            {selectedRow.node.meshId ? "Renderable node" : "Group node"}
          </div>
        </div>
        <button
          type="button"
          class="grid size-8 shrink-0 place-items-center rounded-md border hover:bg-muted"
          title={selectedRow.node.visible ? "Hide node" : "Show node"}
          aria-label={selectedRow.node.visible ? "Hide node" : "Show node"}
          aria-pressed={selectedRow.node.visible}
          onclick={toggleSelectedVisibility}
        >
          {#if selectedRow.node.visible}
            <EyeIcon class="size-4" />
          {:else}
            <EyeOffIcon class="size-4" />
          {/if}
        </button>
      </div>
    {:else}
      <div class="text-sm text-muted-foreground">No scene node selected.</div>
    {/if}
  </div>
</div>
