<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { onMount, tick } from "svelte";
  import { thumbnailQueue } from "./thumbnailQueue";

  export let image: { name: string; path: string };
  export let index: number;
  export let scrollRoot: HTMLElement;
  export let onOpen: (image: { name: string; path: string }) => void;
  export let selected: boolean;
  export let onSelect: (path: string, selected: boolean) => void;

  let host: HTMLElement;
  let imageButton: HTMLButtonElement;
  let src = "";
  let error = "";

  onMount(() => {
    let disposed = false;
    let requested = false;
    let nearViewport = false;
    let loadedSize = 0;
    let targetSize = 256;
    let cancel: (() => boolean) | undefined;

    const priority = () => {
      const card = host.getBoundingClientRect();
      const viewport = scrollRoot.getBoundingClientRect();
      return Math.max(viewport.top - card.bottom, card.top - viewport.bottom, 0);
    };

    function schedule() {
      if (disposed || !nearViewport || requested || error || loadedSize >= targetSize) return;
      requested = true;
      cancel = thumbnailQueue.enqueue(priority, async () => {
        const size = targetSize;
        let nextUrl = "";
        try {
          const url = new URL(convertFileSrc("/", "thumbnail"));
          url.search = new URLSearchParams({ path: image.path, size: String(size) }).toString();
          const response = await fetch(url);
          if (!response.ok) throw new Error(await response.text());
          const blob = await response.blob();
          if (disposed) return;
          nextUrl = URL.createObjectURL(blob);
          const decoded = new Image();
          decoded.src = nextUrl;
          await decoded.decode();
          if (disposed) return;
          const previousUrl = src;
          src = nextUrl;
          nextUrl = "";
          loadedSize = size;
          await tick();
          if (previousUrl) URL.revokeObjectURL(previousUrl);
        } catch (reason) {
          if (!disposed) error = String(reason);
        } finally {
          if (nextUrl) URL.revokeObjectURL(nextUrl);
          requested = false;
          schedule();
        }
      }, (reason) => {
        requested = false;
        if (!disposed) error = String(reason);
      });
    }

    const observer = new IntersectionObserver((entries) => {
      nearViewport = entries.some((entry) => entry.isIntersecting);
      if (nearViewport) {
        schedule();
      } else if (cancel?.()) {
        requested = false;
      }
    }, { root: scrollRoot, rootMargin: "1000px 0px 2400px 0px" });
    function updateTargetSize() {
      const bounds = imageButton.getBoundingClientRect();
      const pixels = Math.max(bounds.width, bounds.height) * window.devicePixelRatio;
      targetSize = Math.min(2048, Math.max(256, 2 ** Math.ceil(Math.log2(Math.max(1, pixels)))));
      schedule();
    }
    const resizeObserver = new ResizeObserver(updateTargetSize);
    updateTargetSize();
    resizeObserver.observe(imageButton);
    observer.observe(host);
    return () => {
      disposed = true;
      cancel?.();
      observer.disconnect();
      resizeObserver.disconnect();
      if (src) URL.revokeObjectURL(src);
    };
  });
</script>

<article class="page-card" bind:this={host}>
  <button class="page-image" type="button" bind:this={imageButton} disabled={!src || !!error} onclick={() => onOpen(image)} aria-label="Enlarge {image.name}">
    {#if error}
      <span class="image-error">Unable to load image: {error}</span>
    {:else if src}
      <img {src} alt={image.name} decoding="async" onerror={() => error = "The image could not be decoded."} />
    {:else}
      <span class="image-placeholder">Loading page…</span>
    {/if}
  </button>
  <label class="page-caption" class:selected>
    <input type="checkbox" checked={selected} aria-label="Select {image.name}" onchange={(event) => onSelect(image.path, event.currentTarget.checked)} />
    <span class="page-number">{String(index + 1).padStart(2, "0")}</span><span title={image.name}>{image.name}</span>
  </label>
</article>
