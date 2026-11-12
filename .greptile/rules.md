## What this project is

A terminal UI for amateur-radio **direction finding ("fox hunting")**.
An operator logs *stations* (a known lat/lon plus the bearing from which
a transmission was heard) and the app plots them on a Southern-Africa
map, then **triangulates** the transmitter's location from the bearings
using least squares. Each *group* is an independent hunt with its own
colour.

It is a single-binary Rust application. There is no server, no database,
no async runtime, no network I/O.

## Stack

- Rust 2021, `ratatui` + `crossterm` for the TUI
- `color_eyre` for error reporting
- `geojson` and `csv` for map data loading
- That's it. **Do not propose adding `rand`, `tracing`, `tokio`, `anyhow`,
  `serde` derive, `clap`, or `thiserror`.** All five are either
  unnecessary for the current scope or were explicitly rejected.

## Module responsibilities

| File | Owns |
|---|---|
| `src/main.rs` | Terminal init/restore, `MapData::load`, startup |
| `src/app.rs` | `App` state, event loop, key handling, form, triangulation |
| `src/event.rs` | Tick thread + crossterm poller, `mpsc` plumbing |
| `src/ui.rs` | `impl Widget for &App`, side panels, map renderer, all `draw_*` helpers |
| `src/map_data.rs` | Loading geojson/csv into memory, land mask, spatial bboxes |

**Do not propose moving rendering code out of `ui.rs` or event plumbing out
of `event.rs`.** The boundary is: `app.rs` never touches `ratatui::Buffer`,
`ui.rs` never constructs events, `event.rs` knows nothing about `App`.

## Data files

Loaded synchronously at startup, before `ratatui::init()` (so errors are
visible before the alt-screen is entered — **this ordering is deliberate**).
Missing files are **non-fatal**: the app starts with that layer empty and
logs `[map] <layer> ← (missing ...)` to stderr. Do not change `eprintln!`
into `expect`/`unwrap` on missing files, and do not propose a logging crate.

`find_file` searches `base/`, `base/data/`, and `base/../data/`. This is
intentional — it lets the user place data alongside the crate or in a
sibling folder without a config file.

| File | Layer |
|---|---|
| `south-africa.geojson` | Coastline (also used to build the land mask) |
| `TCT_Road_Centerline.geojson` | Minor roads (dark grey, thin) |
| `Public_Service_Infrastructure_National_Roads.geojson` | Major roads (white, 3 dots thick) |

CSV is accepted as a fallback for roads (only for the narrow
`start_lat/start_lon/end_lat/end_lon` column convention) — this is *not*
intended to be a general GIS reader.

## Non-obvious design (do not flag these)

### Event loop — dirty flag + queue drain
`App::run` **only redraws when state actually changed** (`dirty` flag).
Tick events return `false` from `process_event` and do not trigger a
redraw. After blocking for one event, the loop drains `EventHandler::try_next`
in a tight inner loop — a backlog of ticks is consumed in microseconds.
This was the fix for a multi-second input latency bug; **do not refactor
it back into "draw every iteration".** Do not remove `try_next`.

### Tick rate is 10 FPS
Nothing in the app animates on tick. `App::tick` is a no-op. Ticks exist
only as a heartbeat for future needs. Do not raise `TICK_FPS` without a
concrete reason, and do not propose `app.tick()` doing work "just because
it exists".

### Triangulation
`triangulate` in `app.rs` solves the normal equations for the least-squares
intersection of bearing *rays*. It uses plain `(lon, lat)` as `(x, y)` —
this is a local-planar approximation valid for hunt-scale distances, and
is a **known limitation**, not a bug. Do not propose UTM projection
without a concrete failing case. Degenerate (all-parallel) input falls
back to the centroid deliberately.

### Marker sizing
`marker_radius(dpx, dpy, desired_dots, max_deg)` is the only correct way
to size a screen marker. It makes markers **constant size on screen when
zoomed in**, and **clamped to a max world size when zoomed out** so they
never swallow the map. Do not replace it with a fixed degree radius, and
do not remove the `max_deg` clamp.

### Major road thickness
`MAJOR_ROAD_THICKNESS = 3` is drawn by `draw_thick_segment`, which offsets
parallel lines by **1 braille dot** in perpendicular *dot space* — so
thickness is a fixed *on-screen* width at every zoom level. Do not
convert this to a world-space width (the roads would disappear when
zoomed out). `MAJOR_ROAD_THICKNESS` should stay a small odd integer;
even values look asymmetric with the offset-from-centre implementation.

### Coordinate ordering
- `Polyline`, `BBox`, and every GeoJSON conversion use **`(x, y) = (lon, lat)`**.
- `triangulate` returns `(lat, lon)` and the caller / `ui.rs` reads it that way.
- `coastline`, `land_polygons`, `minor_roads`, `major_roads` all use
  `(lon, lat)` order in every vertex.

Do not "fix" either of these — they differ deliberately. When in doubt,
look at the call site.

### Land mask
`LandMask` is a precomputed **0.005° grid** built once at startup from the
`land_polygons` outer rings. Cells are `0 = water`, `1 = land`,
`2 = edge`. `is_water` does O(1) grid hits and falls back to an exact
point-in-polygon test **only for edge cells**. 0.005° is a deliberate
startup-time / precision tradeoff. `is_water` returns `false` (land) when
the mask is empty, so a missing coastline doesn't paint the whole map
blue — this is intentional.

## Performance contract

**Every new per-frame drawing or per-frame query must consider all three
of these, or it will be rejected:**

1. **Spatial culling** — bbox test against the current viewport before
   iterating over features. Per-segment for polylines.
2. **LOD gate** — skip features that would be smaller than ~1–2 braille
   dots on screen at the current zoom. See `MINOR_ROAD_MIN_DOTS`,
   `MAJOR_ROAD_MIN_DOTS`.
3. **Per-frame cap** — a `MAX_*_PER_FRAME` constant as a safety net. These
   are not expected to fire in normal use; they protect against pathological
   views on slow terminals. Do not remove them.

Per-frame cost target is **< 100 ms**. `App::run` logs `[perf] draw took
...` to stderr when this is exceeded. **Do not remove this instrumentation.**
If you see the log line in a report, that is the reviewer's cue to suspect
the terminal write itself, not the draw code — do not respond to it by
adding more work.

The water-dot grid is deliberately sparse (`WATER_DOT_SPACING = 16`,
~8 characters between dots). It's tuned by eye; do not propose denser
water without benchmarking the frame time.

## Style conventions

- **Free functions in `ui.rs`**, not methods on `App`: `draw_station_marker`,
  `draw_polyline_culled`, `marker_radius`, `bbox_intersects`, etc. These
  do not touch `App` state and are deliberately kept outside the `impl`.
- **No `unwrap` / `expect` on user-facing I/O.** Loaders `return Vec::new()`
  on failure. `let _ = self.sender.send(...)` is deliberate — the receiver
  can be gone at shutdown.
- **`eprintln!` for user-visible startup diagnostics.** Not `log`/`tracing`.
- **`pub` on the minimum necessary.** Cross-module fields (`App::groups`,
  `MapData::coastline`, etc.) are `pub` because `ui.rs` reads them; helpers
  are private.

## User-visible conventions (do not regress)

- Initial viewport is the **Cape Peninsula** (`CAPE_X_MIN` … `CAPE_Y_MAX`
  constants in `app.rs`).
- Zoom is **centre-anchored**, clamped to `MIN_VIEW_WIDTH = 0.0005°` and
  `MAX_VIEW_WIDTH = 60.0°`.
- Keybinds: arrows pan, `+`/`=`/PageUp zoom in, `-`/`_`/PageDown zoom out,
  `` ` `` opens the station form, `r`/`R` resets the viewport,
  `q`/`Esc`/`Ctrl+C` quit.
- Default group names, in order, are:
  `Ärger`, `Bertä`, `Cäsar`, `Dorä`, `Emil`, `Friedrich`, `Gustav`,
  `Heinrich`, `Ida`, then `Group N`. (German-style callsign alphabet; the
  umlauts are intentional.)
- Group colours cycle through a 12-colour palette in `next_group_color`.
  A group keeps its colour across the app session.
- A group gets a fix as soon as it has **≥ 2 stations**. The right panel
  shows `(need ≥2 stations)` until then.

## Out of scope for review

The following are **not** review comments:

- "Add unit tests." Test coverage is a later milestone.
- "Use `serde` derive instead of `geojson::GeoJson`." The DOM style is
  deliberate — the feature set varies across the three data files and
  a small manual `match` is more readable than 40 unused struct fields.
- "Split `ui.rs` into one file per widget." It is one screen.
- "Use `tokio` / async I/O for the loaders." Load is synchronous and
  happens once, before the alt-screen; startup time is dominated by
  filesystem reads and cannot be made meaningfully faster with async.
- "Consider `clap` for CLI args." There are none.
- "The event thread leaks / never terminates." It is detached and ends
  with the process. This is accepted.

## What TO flag

- Any new `for`-loop over a `Vec<Polyline>` in `ui.rs` **without** a bbox
  cull and a `*_MIN_DOTS` LOD gate.
- Any new per-frame work not gated by the dirty flag (i.e. anything that
  runs unconditionally inside `render`).
- Any change that weakens the `try_next` drain loop in `App::run`, or
  that reintroduces a `terminal.draw()` call outside the dirty branch.
- Any code that reads a world-coordinate offset as a screen-pixel offset,
  or vice versa — the two are kept separate by `dpx`/`dpy` and by
  `marker_radius` / `draw_thick_segment`.
- Any `unwrap()` on user I/O (file load, channel send/recv on shutdown).
- Any change that makes a missing data file fatal.
- Any new layer that doesn't document its own LOD thresholds and
  per-frame cap next to the constants at the top of `ui.rs`.
- Removal of `[perf] draw took ...` instrumentation.
