# PolyPrint

A Windows-oriented Tauri application for browsing and printing folders of manual pages. Choose a drive or favourite, open a folder, and scan images in natural filename order. Choose **Two pages** for a two-column layout or **Grid** for a responsive layout with an adjustable thumbnail size. In the two-page layout, the first page appears on the right by default; turn off **First page on right** to start it on the left. Click a segment of the location breadcrumb to return to an ancestor folder. Star a folder to add or remove it from favourites; favourites are stored locally for the current user.

Use **Find** to filter file and folder names in the current folder, without searching subfolders. In two-page view, matching pages retain their original page numbers and left/right positions. Click a thumbnail to enlarge it, or click its footer to select it. **Print All** prints every page in the current folder; **Print Selected** prints selected pages in their original order, including selections hidden by Find. The find text and selection clear when navigating to a different folder.

The print dialog uses one image per sheet, scaled to fit, with zero document margins and no application headers or footers. Enable **Comb-binding allowance** and choose a width in millimetres to leave additional space on the left of odd-numbered original pages and the right of even-numbered original pages, including when printing a selection. The allowance is measured from the paper edge; the image is reduced as needed to fit the remaining area.

Choose the printer, paper size, orientation and duplex settings in the print dialog. Printers have different non-printable edges, and their dialogs may override document margins or add headers and footers; check the preview and select the printer's minimum margins and disable headers and footers if necessary. Borderless printing requires a printer and paper setting that support it. For double-sided comb binding, select the appropriate duplex edge for the paper orientation. The print job includes the images directly in the current folder, not images in subfolders.

Supported formats are JPEG, PNG, BMP, GIF, WebP and TIFF. Animated images and multipage TIFF files show and print only their first frame; split multipage TIFFs into individual files before using them as manual pages. TIFF conversion and printing large folders may require substantial memory. The view layout is for browsing only and does not change the order or layout of printed sheets.

Pages near the visible area are loaded in advance, with a limited number decoded at once. The browser displays reduced-size images held in memory; no thumbnail files are saved. JPEGs can be decoded at a reduced resolution, while other formats are decoded at full resolution before resizing. Enlarging and printing use the original resolution. Very large images or slow drives can still take time to appear.

## Development

Install Node.js LTS, Rust, Microsoft C++ Build Tools with the desktop C++ workload, and WebView2. In a new terminal:

```powershell
npm install
npm run tauri dev
```

Use `npm run check` for frontend type checking and `npm run tauri build` for a release executable. Bundling installers is disabled in this initial version. Files are read locally; the app's asset protocol is scoped to the local filesystem so that images can be opened from any drive.
