<script lang="ts">
	import { Slider as SliderPrimitive } from "bits-ui";
	import { cn } from "$lib/utils.js";

	type SliderProps = {
		ref?: HTMLElement | null;
		value?: number;
		type?: "single";
		min?: number;
		max?: number;
		step?: number;
		disabled?: boolean;
		orientation?: "horizontal" | "vertical";
		class?: string;
		"aria-label"?: string;
	};

	let {
		ref = $bindable(null),
		value = $bindable(0),
		type = "single",
		min = 0,
		max = 100,
		step = 1,
		disabled = false,
		orientation = "horizontal",
		class: className,
		"aria-label": ariaLabel,
	}: SliderProps = $props();
</script>

<SliderPrimitive.Root
	bind:ref
	bind:value
	data-slot="slider"
	{type}
	{min}
	{max}
	{step}
	{disabled}
	{orientation}
	aria-label={ariaLabel}
	class={cn(
		"data-vertical:min-h-40 relative flex w-full touch-none items-center select-none data-disabled:opacity-50 data-vertical:h-full data-vertical:w-auto data-vertical:flex-col",
		className
	)}
>
	{#snippet children({ thumbItems })}
		<span
			data-slot="slider-track"
			data-orientation={orientation}
			class={cn(
				"bg-muted rounded-full data-horizontal:h-1 data-horizontal:w-full data-vertical:h-full data-vertical:w-1 bg-muted relative grow overflow-hidden data-horizontal:w-full data-vertical:h-full"
			)}
		>
			<SliderPrimitive.Range
				data-slot="slider-range"
				class={cn(
					"bg-primary absolute select-none data-horizontal:h-full data-vertical:w-full"
				)}
			/>
		</span>
		{#each thumbItems as thumb (thumb.index)}
			<SliderPrimitive.Thumb
				data-slot="slider-thumb"
				index={thumb.index}
				class="border-ring ring-ring/50 relative size-3 rounded-full border bg-white transition-[color,box-shadow] after:absolute after:-inset-2 hover:ring-3 focus-visible:ring-3 focus-visible:outline-hidden active:ring-3 block shrink-0 select-none disabled:pointer-events-none disabled:opacity-50"
			/>
		{/each}
	{/snippet}
</SliderPrimitive.Root>
