<script lang="ts">
  import { Chart, type ChartConfiguration } from "chart.js/auto";
  import { onDestroy, untrack } from "svelte";

  // Callers must pass plain objects/arrays (e.g. built with .map), not $state proxies.
  let { config, height = 280 }: { config: ChartConfiguration<any, any, any>; height?: number } = $props();

  let canvas: HTMLCanvasElement | undefined = $state();
  let chart: Chart | undefined;

  $effect(() => {
    const el = canvas;
    const c = config;
    if (!el) return;
    untrack(() => {
      chart?.destroy();
      chart = new Chart(el, c);
    });
  });

  onDestroy(() => chart?.destroy());
</script>

<div class="chart" style="height: {height}px">
  <canvas bind:this={canvas}></canvas>
</div>

<style>
  .chart {
    position: relative;
    width: 100%;
  }
</style>
