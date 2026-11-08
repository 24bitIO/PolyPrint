<script lang="ts">
  import { convertFileSrc, invoke } from "@tauri-apps/api/core";
  import { onMount, tick } from "svelte";
  import ImageCard from "./ImageCard.svelte";
  import { matchesFind, pageCells, pagesForPrinting } from "./pages";
  import type { Entry } from "./pages";

  type Directory = { path: string; parent: string | null; breadcrumbs: Entry[]; folders: Entry[]; images: Entry[] };
  const favouritesKey = "polyprint.favourites";

  let drives: string[] = [];
  let favourites: string[] = [];
  let directory: Directory | null = null;
  let error = "";
  let busy = false;
  let printing = false;
  let printSources: { name: string; src: string; index: number }[] = [];
  let printArea: HTMLElement;
  let breadcrumbNav: HTMLElement;
  let workspace: HTMLElement;
  let bindingEnabled = false;
  let bindingMm = 10;
  let printBindingMm = 0;
  let coverOnRight = true;
  let layout: "two-page" | "grid" = "two-page";
  let gridBasis = 240;
  let find = "";
  let selected = new Set<string>();
  let preview: { src: string; name: string } | null = null;
  let previewRequestId = 0;
  let requestId = 0;

  $: visibleFolders = (directory?.folders ?? []).filter((folder) => matchesFind(folder.name, find));
  $: visiblePages = pageCells(directory?.images ?? [], find, layout, coverOnRight);
  $: visiblePageCount = visiblePages.filter((cell) => cell.kind === "page").length;

  onMount(async () => {
    try {
      const saved = localStorage.getItem(favouritesKey);
      if (saved) {
        const parsed: unknown = JSON.parse(saved);
        if (!Array.isArray(parsed) || !parsed.every((item) => typeof item === "string")) {
          throw new Error("Saved favourites have an invalid format.");
        }
        favourites = parsed;
      }
    } catch (reason) {
      error = `Could not load favourites: ${reason}`;
    }
    await refreshDrives();
  });

  async function refreshDrives() {
    try {
      drives = await invoke<string[]>("list_drives");
    } catch (reason) {
      error = `Could not list drives: ${reason}`;
    }
  }

  async function openFolder(path: string) {
    if (!path.trim()) {
      error = "Invalid folder path.";
      return;
    }
    const id = ++requestId;
    busy = true;
    error = "";
    try {
      const result = await invoke<Directory>("read_directory", { path });
      if (id !== requestId) return;
      if (directory?.path !== result.path) {
        find = "";
        selected = new Set();
      } else {
        const available = new Set(result.images.map((image) => image.path));
        selected = new Set([...selected].filter((path) => available.has(path)));
      }
      directory = result;
      await tick();
      if (id === requestId && breadcrumbNav) breadcrumbNav.scrollLeft = breadcrumbNav.scrollWidth;
    } catch (reason) {
      if (id === requestId) error = String(reason);
    } finally {
      if (id === requestId) busy = false;
    }
  }

  function setFavourites(next: string[]) {
    try {
      localStorage.setItem(favouritesKey, JSON.stringify(next));
      favourites = next;
    } catch (reason) {
      error = `Could not save favourites: ${reason}`;
    }
  }

  function toggleFavourite(path: string) {
    setFavourites(favourites.includes(path) ? favourites.filter((item) => item !== path) : [...favourites, path]);
  }

  function setSelected(path: string, checked: boolean) {
    const next = new Set(selected);
    if (checked) next.add(path);
    else next.delete(path);
    selected = next;
  }

  function closePreview() {
    previewRequestId++;
    preview = null;
  }

  async function openPreview(image: Entry) {
    const id = ++previewRequestId;
    preview = { name: image.name, src: "" };
    try {
      const src = /\.tiff?$/i.test(image.path)
        ? await invoke<string>("tiff_as_png", { path: image.path, thumbnail: false })
        : convertFileSrc(image.path);
      if (id === previewRequestId) preview = { name: image.name, src };
    } catch (reason) {
      if (id === previewRequestId) {
        closePreview();
        error = `Cannot open ${image.name}: ${reason}`;
      }
    }
  }

  async function printPages(mode: "all" | "selected") {
    if (!directory || printing) return;
    const pages = pagesForPrinting(directory.images, mode === "all" ? null : selected);
    if (pages.length === 0) {
      error = mode === "all" ? "There are no pages to print." : "Select at least one page to print.";
      return;
    }
    if (bindingEnabled && (!Number.isFinite(bindingMm) || bindingMm < 0 || bindingMm > 50)) {
      error = "Enter a binding allowance between 0 and 50 mm.";
      return;
    }
    printing = true;
    error = "";
    printBindingMm = bindingEnabled ? bindingMm : 0;
    try {
      printSources = await Promise.all(pages.map(async ({ image, index }) => ({
        name: image.name,
        index,
        src: /\.tiff?$/i.test(image.path)
          ? await invoke<string>("tiff_as_png", { path: image.path, thumbnail: false })
          : convertFileSrc(image.path),
      })));
      await tick();
      await Promise.all(Array.from(printArea.querySelectorAll("img")).map((image) => {
        if (image.complete) {
          if (!image.naturalWidth) throw new Error(`Could not load ${image.alt} for printing.`);
          return Promise.resolve();
        }
        return new Promise<void>((resolve, reject) => {
          image.onload = () => resolve();
          image.onerror = () => reject(new Error(`Could not load ${image.alt} for printing.`));
        });
      }));
      const afterPrint = () => {
        printing = false;
        printSources = [];
      };
      window.addEventListener("afterprint", afterPrint, { once: true });
      try {
        window.print();
      } catch (reason) {
        window.removeEventListener("afterprint", afterPrint);
        throw reason;
      }
    } catch (reason) {
      error = `Cannot print: ${reason}`;
      printing = false;
      printSources = [];
    }
  }
</script>

<svelte:head><title>PolyPrint</title></svelte:head>
<svelte:window onkeydown={(event) => { if (event.key === "Escape") closePreview(); }} />

<div class="app-shell">
  <aside class="sidebar">
    <div class="brand"><div class="brand-icon">P<span>.</span></div><div><strong>POLYPRINT</strong><small>MANUAL WORKSPACE</small></div></div>
    <div class="sidebar-content">
      <div class="section-label"><span>FAVOURITES</span><span>{favourites.length}</span></div>
      {#if favourites.length === 0}
        <p class="sidebar-hint">Star a folder to keep it close.</p>
      {:else}
        {#each favourites as favourite}
          <div class="favourite-row">
            <button type="button" class:active={directory?.path === favourite} class="nav-item" title={favourite} onclick={() => openFolder(favourite)}><span>☆</span><span>{favourite}</span></button>
            <button type="button" class="remove-favourite" aria-label="Remove {favourite} from favourites" title="Remove favourite" onclick={() => toggleFavourite(favourite)}>×</button>
          </div>
        {/each}
      {/if}

      <div class="section-label"><span>THIS COMPUTER</span><button type="button" class="refresh-drives" title="Refresh drives" aria-label="Refresh drives" onclick={refreshDrives}>↻</button></div>
      {#each drives as drive}
        <button type="button" class:active={directory?.path === drive} class="nav-item" onclick={() => openFolder(drive)}><span>▤</span><span>{drive}</span></button>
      {/each}
      {#if drives.length === 0}<p class="sidebar-hint">No drives found.</p>{/if}
    </div>
  </aside>

  <main>
    <header class="topbar">
      <div class="location">
        <span class="location-label">LOCATION</span>
        <nav class="breadcrumbs" aria-label="Folder path" bind:this={breadcrumbNav}>
          {#if directory}
            {#each directory.breadcrumbs as crumb, index (crumb.path)}
              {#if index > 0}<span class="breadcrumb-separator" aria-hidden="true">/</span>{/if}
              <button type="button" class="breadcrumb" title={crumb.path} aria-current={index === directory.breadcrumbs.length - 1 ? "location" : undefined} onclick={() => openFolder(crumb.path)}>{crumb.name}</button>
            {/each}
          {:else}
            <span class="breadcrumb-placeholder">Choose a drive or favourite</span>
          {/if}
        </nav>
      </div>
      <div class="top-actions">
        <button type="button" class="icon-button" disabled={!directory} title="Add or remove favourite" aria-label="Toggle favourite" onclick={() => directory && toggleFavourite(directory.path)}>{directory && favourites.includes(directory.path) ? "★" : "☆"}</button>
        <button type="button" class="print-button secondary-print" disabled={!selected.size || printing || busy} onclick={() => printPages("selected")}>Print Selected ({selected.size})</button>
        <button type="button" class="print-button" disabled={!directory?.images.length || printing || busy} onclick={() => printPages("all")}>{printing ? "Preparing…" : "Print All"}</button>
      </div>
    </header>

    {#if error}<div class="error-banner" role="alert">{error}<button type="button" aria-label="Dismiss error" onclick={() => error = ""}>×</button></div>{/if}
    <div class="print-settings">
      <span class="print-settings-title">PRINT SETUP</span>
      <label class="binding-option"><input type="checkbox" bind:checked={bindingEnabled} disabled={printing} /> Comb-binding allowance</label>
      <label class="binding-size"><span>Width</span><input type="number" min="0" max="50" step="1" bind:value={bindingMm} disabled={!bindingEnabled || printing} aria-label="Comb-binding allowance in millimetres" /> mm</label>
      <span class="print-settings-hint">Odd pages: left · Even pages: right</span>
    </div>
    <div class="workspace" bind:this={workspace}>
      <div class="workspace-head">
        <div><h1>{directory ? (directory.path.split(/[\\/]/).filter(Boolean).pop() || directory.path) : "Your manuals"}</h1><p>{directory ? directory.path : "Browse a folder to get started."}</p></div>
        <div class="layout-controls">
          <div class="layout-selector" role="group" aria-label="Page layout">
            <button type="button" aria-pressed={layout === "two-page"} onclick={() => layout = "two-page"}>Two pages</button>
            <button type="button" aria-pressed={layout === "grid"} onclick={() => layout = "grid"}>Grid</button>
          </div>
          {#if layout === "grid"}
            <label class="grid-size">Thumbnail size <input type="range" min="140" max="420" step="10" bind:value={gridBasis} /> <span>{gridBasis}px</span></label>
          {/if}
        </div>
      </div>

      {#if directory}
        <div class="find-bar">
          <label for="find-input">Find</label>
          <input id="find-input" type="search" bind:value={find} placeholder="Filter files and folders" disabled={busy} autocomplete="off" />
          {#if find}<button type="button" onclick={() => find = ""}>Clear</button>{/if}
        </div>
        <section class="folders-section">
          <div class="section-heading"><div><span class="heading-mark"></span> FOLDERS <span class="count">{visibleFolders.length}{find ? ` / ${directory.folders.length}` : ""}</span></div><span class="hint">Select a folder to open it</span></div>
          <div class="folder-list">
            {#if directory.parent}<button type="button" class="folder-chip parent" onclick={() => directory?.parent && openFolder(directory.parent)}><span>↑</span> Up one level</button>{/if}
            {#each visibleFolders as folder}
              <button type="button" class="folder-chip" title={folder.path} onclick={() => openFolder(folder.path)}><span>▰</span> {folder.name}</button>
            {/each}
            {#if visibleFolders.length === 0 && find}<span class="muted">No folders match.</span>
            {:else if !directory.parent && directory.folders.length === 0}<span class="muted">No subfolders</span>{/if}
          </div>
        </section>
        <section class="pages-section">
          <div class="section-heading"><div><span class="heading-mark"></span> PAGES <span class="count">{visiblePageCount}{find ? ` / ${directory.images.length}` : ""}</span></div>{#if layout === "two-page"}<label class="cover-toggle"><input type="checkbox" bind:checked={coverOnRight} /> First page on right</label>{/if}</div>
          {#if busy}<div class="loading">Opening folder…</div>
          {:else if directory.images.length === 0}<div class="empty-small">No supported images in this folder.</div>
          {:else if visiblePageCount === 0}<div class="empty-small">No pages match.</div>
          {:else}
            <div class="page-grid" class:grid-view={layout === "grid"} style:--grid-basis="{gridBasis}px">
              {#each visiblePages as cell (cell.key)}
                {#if cell.kind === "spacer"}
                  <div class="page-spacer" aria-hidden="true"></div>
                {:else}
                  <ImageCard image={cell.page.image} index={cell.page.index} selected={selected.has(cell.page.image.path)} onSelect={setSelected} scrollRoot={workspace} onOpen={openPreview} />
                {/if}
              {/each}
            </div>
          {/if}
        </section>
      {:else}
        <div class="welcome"><div class="welcome-icon">▧</div><h2>Every page in its place.</h2><p>Choose a drive or favourite on the left to browse your manual pages side by side.</p></div>
      {/if}
    </div>
    <footer>{#if busy}<span role="status">OPENING FOLDER</span>{/if}<span>{directory ? `${visiblePageCount}${find ? ` / ${directory.images.length}` : ""} PAGES  ·  ${selected.size} SELECTED` : "SELECT A FOLDER"}</span></footer>
  </main>
</div>

{#if preview}
  <div class="modal-backdrop" role="presentation" onclick={closePreview}>
    <div class="preview-modal" role="dialog" aria-modal="true" tabindex="-1" aria-label={preview.name} onclick={(event) => { if (event.target === event.currentTarget) closePreview(); event.stopPropagation(); }} onkeydown={(event) => { if (event.key === "Escape") closePreview(); }}>
      <div><strong>{preview.name}</strong><button type="button" aria-label="Close preview" onclick={closePreview}>×</button></div>
      {#if preview.src}
        <img src={preview.src} alt={preview.name} onerror={() => { error = `Cannot open ${preview?.name}: the image could not be decoded.`; closePreview(); }} />
      {:else}
        <p role="status">Loading full-size image…</p>
      {/if}
    </div>
  </div>
{/if}

<div class="print-area" bind:this={printArea} style:--binding-allowance="{printBindingMm}mm">
  {#each printSources as image}
    <div class="print-page" class:binding-left={image.index % 2 === 0} class:binding-right={image.index % 2 === 1}><img src={image.src} alt={image.name} /></div>
  {/each}
</div>
