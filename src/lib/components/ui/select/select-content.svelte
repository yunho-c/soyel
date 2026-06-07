<script lang="ts">
  import { Select as SelectPrimitive } from "bits-ui";
  import type { ComponentProps } from "svelte";

  import { cn } from "$lib/utils.js";

  type SelectContentProps = ComponentProps<typeof SelectPrimitive.Content> & {
    viewportClass?: string;
  };

  let {
    ref = $bindable(null),
    class: className,
    viewportClass,
    children,
    sideOffset = 4,
    ...restProps
  }: SelectContentProps = $props();
</script>

<SelectPrimitive.Portal>
  <SelectPrimitive.Content
    bind:ref
    data-slot="select-content"
    {sideOffset}
    class={cn(
      "bg-popover text-popover-foreground z-50 min-w-[8rem] overflow-hidden rounded-md border shadow-md",
      className,
    )}
    {...restProps}
  >
    <SelectPrimitive.Viewport class={cn("p-1", viewportClass)}>
      {@render children?.()}
    </SelectPrimitive.Viewport>
  </SelectPrimitive.Content>
</SelectPrimitive.Portal>
