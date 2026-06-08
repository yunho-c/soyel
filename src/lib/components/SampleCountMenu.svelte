<script lang="ts">
  import ChevronDownIcon from "lucide-svelte/icons/chevron-down";

  import { Button } from "$lib/components/ui/button";
  import * as DropdownMenu from "$lib/components/ui/dropdown-menu";
  import { Slider } from "$lib/components/ui/slider";

  let {
    label,
    value = $bindable(64),
    presets,
    min = 1,
    max = 1024,
    step = 1,
  }: {
    label: string;
    value: number;
    presets: number[];
    min?: number;
    max?: number;
    step?: number;
  } = $props();

  let selectedPreset = $derived(presets.includes(value) ? String(value) : "");

  function setSampleCount(next: number) {
    value = Math.min(max, Math.max(min, Math.round(next)));
  }
</script>

<DropdownMenu.Root>
  <DropdownMenu.Trigger>
    {#snippet child({ props })}
      <Button
        {...props}
        variant="ghost"
        size="xs"
        class="h-5 rounded-sm px-1.5 text-[0.72rem] font-medium text-primary-foreground hover:bg-primary-foreground/15 hover:text-primary-foreground aria-expanded:bg-primary-foreground/20 aria-expanded:text-primary-foreground"
      >
        <span>{label}: {value} samples</span>
        <ChevronDownIcon class="size-3 opacity-75" />
      </Button>
    {/snippet}
  </DropdownMenu.Trigger>
  <DropdownMenu.Content class="w-56" align="end" side="top" sideOffset={6}>
    <DropdownMenu.Label>{label} samples</DropdownMenu.Label>
    <DropdownMenu.Separator />
    <DropdownMenu.RadioGroup value={selectedPreset}>
      {#each presets as preset}
        <DropdownMenu.RadioItem value={String(preset)} onclick={() => setSampleCount(preset)}>
          {preset}
        </DropdownMenu.RadioItem>
      {/each}
    </DropdownMenu.RadioGroup>
    <DropdownMenu.Separator />
    <div class="grid gap-3 px-2 py-2">
      <div class="flex items-center justify-between gap-3 text-xs">
        <span class="text-muted-foreground">Custom</span>
        <span class="font-medium tabular-nums">{value}</span>
      </div>
      <Slider type="single" bind:value min={min} max={max} step={step} aria-label={`${label} samples`} />
    </div>
  </DropdownMenu.Content>
</DropdownMenu.Root>
