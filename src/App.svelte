<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import ArrowRightIcon from "lucide-svelte/icons/arrow-right";
  import CheckCircle2Icon from "lucide-svelte/icons/check-circle-2";
  import { Button } from "$lib/components/ui/button";

  let name = $state("Tauri");
  let greeting = $state("");
  let isLoading = $state(false);

  async function greet() {
    isLoading = true;

    try {
      greeting = await invoke<string>("greet", { name });
    } finally {
      isLoading = false;
    }
  }
</script>

<main class="grid min-h-screen place-items-center px-6 py-10">
  <section
    class="w-full max-w-xl rounded-lg border bg-card p-8 text-card-foreground shadow-sm"
  >
    <div class="flex flex-col gap-7">
      <div class="flex flex-col gap-3">
        <div
          class="inline-flex w-fit items-center gap-2 rounded-md border bg-secondary px-3 py-1 text-sm font-medium text-secondary-foreground"
        >
          <CheckCircle2Icon class="size-4" />
          Svelte + Vite + shadcn-svelte
        </div>
        <div class="flex flex-col gap-2">
          <h1 class="text-4xl font-semibold tracking-normal text-balance">
            soyel
          </h1>
          <p class="max-w-md text-sm leading-6 text-muted-foreground">
            A Tauri app shell wired for Bun, Cargo, Svelte, Tailwind CSS, and
            shadcn-svelte components.
          </p>
        </div>
      </div>

      <form class="flex flex-col gap-3" onsubmit={(event) => {
        event.preventDefault();
        void greet();
      }}>
        <label class="text-sm font-medium" for="name">Name</label>
        <div class="flex flex-col gap-3 sm:flex-row">
          <input
            id="name"
            bind:value={name}
            autocomplete="name"
            class="h-10 min-w-0 flex-1 rounded-md border bg-background px-3 text-sm outline-none transition-colors focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/20"
          />
          <Button type="submit" disabled={isLoading} class="sm:w-32">
            {isLoading ? "Calling" : "Greet"}
            <ArrowRightIcon class="size-4" />
          </Button>
        </div>
      </form>

      <p
        class="min-h-6 rounded-md bg-muted px-3 py-2 text-sm font-medium text-muted-foreground"
        aria-live="polite"
      >
        {greeting || "Rust command output will appear here."}
      </p>
    </div>
  </section>
</main>

