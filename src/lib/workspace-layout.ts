const STORAGE_KEY = "soyel:workspace-layout:v1";

export type WorkspaceActivity = "scene" | "assets" | "materials" | "render";
export type WorkspacePanel = "console" | "render-log" | "jobs";

export type WorkspaceLayoutState = {
  leftVisible: boolean;
  rightVisible: boolean;
  bottomVisible: boolean;
  activeActivity: WorkspaceActivity;
  activePanel: WorkspacePanel;
};

export const defaultWorkspaceLayout: WorkspaceLayoutState = {
  leftVisible: true,
  rightVisible: true,
  bottomVisible: true,
  activeActivity: "materials",
  activePanel: "console",
};

export function loadWorkspaceLayout(): WorkspaceLayoutState {
  if (typeof localStorage === "undefined") {
    return { ...defaultWorkspaceLayout };
  }

  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) {
      return { ...defaultWorkspaceLayout };
    }

    const parsed = JSON.parse(raw) as Partial<WorkspaceLayoutState>;

    return {
      ...defaultWorkspaceLayout,
      ...parsed,
    };
  } catch {
    return { ...defaultWorkspaceLayout };
  }
}

export function saveWorkspaceLayout(layout: WorkspaceLayoutState) {
  if (typeof localStorage === "undefined") {
    return;
  }

  localStorage.setItem(STORAGE_KEY, JSON.stringify(layout));
}
