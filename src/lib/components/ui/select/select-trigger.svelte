<script lang="ts">
  import { Select as SelectPrimitive } from "bits-ui";
  import ChevronDownIcon from "lucide-svelte/icons/chevron-down";
  import type { ComponentProps } from "svelte";

  import { cn } from "$lib/utils.js";

  type SelectTriggerProps = ComponentProps<typeof SelectPrimitive.Trigger>;

  let {
    ref = $bindable(null),
    class: className,
    children,
    ...restProps
  }: SelectTriggerProps = $props();
</script>

<SelectPrimitive.Trigger
  bind:ref
  data-slot="select-trigger"
  class={cn(
    "border-input bg-background hover:bg-muted focus-visible:border-ring focus-visible:ring-ring/50 flex h-7 min-w-32 items-center justify-between gap-2 rounded-md border px-2.5 text-[0.8rem] font-medium outline-none transition focus-visible:ring-3 disabled:pointer-events-none disabled:opacity-50",
    className,
  )}
  {...restProps}
>
  <span class="truncate">
    {@render children?.()}
  </span>
  <ChevronDownIcon class="size-3.5 shrink-0 opacity-60" />
</SelectPrimitive.Trigger>
