<script lang="ts">
  import { Select as SelectPrimitive } from "bits-ui";
  import CheckIcon from "lucide-svelte/icons/check";
  import type { ComponentProps } from "svelte";

  import { cn } from "$lib/utils.js";

  type SelectItemProps = ComponentProps<typeof SelectPrimitive.Item>;

  let {
    ref = $bindable(null),
    class: className,
    children,
    ...restProps
  }: SelectItemProps = $props();
</script>

<SelectPrimitive.Item
  bind:ref
  data-slot="select-item"
  class={cn(
    "data-[highlighted]:bg-muted data-[highlighted]:text-foreground data-[disabled]:pointer-events-none data-[disabled]:opacity-50 group/select-item relative flex h-7 cursor-default select-none items-center rounded-md py-1.5 pl-2 pr-7 text-sm outline-none",
    className,
  )}
  {...restProps}
>
  {#snippet children(item)}
    <span
      class="absolute right-2 flex size-3.5 items-center justify-center opacity-0"
      class:opacity-100={item.selected}
    >
      <CheckIcon class="size-3.5" />
    </span>
    {@render children?.(item)}
  {/snippet}
</SelectPrimitive.Item>
