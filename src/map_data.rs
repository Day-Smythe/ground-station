use std::fs;
use std::path::{Path, PathBuf};

pub type Point = (f64, f64);
pub type Polyline = Vec<Point>;
pub type BBox = (f64, f64, f64, f64); // minx, miny, maxx, maxy

#[derive(Debug, Clone)]
pub struct Building {
    pub bbox: BBox,
}

// ---------- Grid index ----------

#[derive(Debug)]
pub struct GridIndex {
    x_min: f64,
    y_min: f64,
    cell_w: f64,
    cell_h: f64,
    nx: usize,
    ny: usize,
    cells: Vec<Vec<u32>>,
}

impl GridIndex {
    pub fn empty() -> Self {
        Self {
            x_min: 0.0,
            y_min: 0.0,
            cell_w: 1.0,
            cell_h: 1.0,
            nx: 0,
            ny: 0,
            cells: Vec::new(),
        }
    }

    pub fn build(items: &[Building], extent: BBox, n: usize) -> Self {
        let (x0, y0, x1, y1) = extent;
        let nx = n.max(1);
        let ny = n.max(1);
        let cell_w = ((x1 - x0) / nx as f64).max(1e-9);
        let cell_h = ((y1 - y0) / ny as f64).max(1e-9);
        let mut cells = vec![Vec::new(); nx * ny];

        for (i, item) in items.iter().enumerate() {
            let (bx0, by0, bx1, by1) = item.bbox;
            let ix0 = (((bx0 - x0) / cell_w).floor() as i64).clamp(0, (nx - 1) as i64) as usize;
            let iy0 = (((by0 - y0) / cell_h).floor() as i64).clamp(0, (ny - 1) as i64) as usize;
            let ix1 = (((bx1 - x0) / cell_w).floor() as i64).clamp(0, (nx - 1) as i64) as usize;
            let iy1 = (((by1 - y0) / cell_h).floor() as i64).clamp(0, (ny - 1) as i64) as usize;
            for iy in iy0..=iy1 {
                for ix in ix0..=ix1 {
                    cells[iy * nx + ix].push(i as u32);
                }
            }
        }

        Self { x_min: x0, y_min: y0, cell_w, cell_h, nx, ny, cells }
    }

    pub fn query(&self, bbox: BBox) -> Vec<u32> {
        if self.nx == 0 || self.ny == 0 {
            return Vec::new();
        }
        let (qx0, qy0, qx1, qy1) = bbox;
        let ix0 = (((qx0 - self.x_min) / self.cell_w).floor() as i64)
            .clamp(0, (self.nx - 1) as i64) as usize;
        let iy0 = (((qy0 - self.y_min) / self.cell_h).floor() as i64)
            .clamp(0, (self.ny - 1) as i64) as usize;
        let ix1 = (((qx1 - self.x_min) / self.cell_w).floor() as i64)
            .clamp(0, (self.nx - 1) as i64) as usize;
        let iy1 = (((qy1 - self.y_min) / self.cell_h).floor() as i64)
            .clamp(0, (self.ny - 1) as i64) as usize;

        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for iy in iy0..=iy1 {
            for ix in ix0..=ix1 {
                for &idx in &self.cells[iy * self.nx + ix] {
                    if seen.insert(idx) {
                        out.push(idx);
                    }
                }
            }
        }
        out
    }
}

// ---------- Land / water mask ----------

#[derive(Debug)]
pub struct LandMask {
    x_min: f64,
    y_min: f64,
    resolution: f64,
    nx: usize,
    ny: usize,
    cells: Vec<u8>,
}

impl LandMask {
    pub fn empty() -> Self {
        Self {
            x_min: 0.0,
            y_min: 0.0,
            resolution: 1.0,
            nx: 0,
            ny: 0,
            cells: Vec::new(),
        }
    }

    pub fn build(polygons: &[Polyline], bbox: BBox, resolution: f64) -> Self {
        if polygons.is_empty() {
            return Self::empty();
        }
        let (x0, y0, x1, y1) = bbox;
        let nx = ((x1 - x0) / resolution).ceil() as usize + 1;
        let ny = ((y1 - y0) / resolution).ceil() as usize + 1;
        let mut cells = vec![0u8; nx * ny];

        // Flatten all polygon edges once.
        let mut edges: Vec<(f64, f64, f64, f64)> = Vec::new();
        for poly in polygons {
            if poly.len() < 2 {
                continue;
            }
            for w in poly.windows(2) {
                edges.push((w[0].0, w[0].1, w[1].0, w[1].1));
            }
            let first = poly[0];
            let last = *poly.last().unwrap();
            if first != last {
                edges.push((last.0, last.1, first.0, first.1));
            }
        }

        for iy in 0..ny {
            let y = y0 + iy as f64 * resolution;

            // Cast a horizontal ray and collect x-crossings.
            let mut crossings: Vec<f64> = Vec::new();
            for &(x1p, y1p, x2p, y2p) in &edges {
                if (y1p <= y && y2p > y) || (y2p <= y && y1p > y) {
                    let t = (y - y1p) / (y2p - y1p);
                    crossings.push(x1p + t * (x2p - x1p));
                }
            }
            crossings.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            // Fill this row with land / water by even-odd rule.
            let mut inside = false;
            let mut ci = 0usize;
            for ix in 0..nx {
                let x = x0 + ix as f64 * resolution;
                while ci < crossings.len() && crossings[ci] < x {
                    inside = !inside;
                    ci += 1;
                }
                cells[iy * nx + ix] = if inside { 1 } else { 0 };
            }

            // Mark the cells adjacent to each crossing as "edge".
            for xc in &crossings {
                let ix_c = ((xc - x0) / resolution).round() as isize;
                for d in -1..=1i32 {
                    let ix = ix_c + d as isize;
                    if ix >= 0 && (ix as usize) < nx {
                        cells[iy * nx + ix as usize] = 2;
                    }
                }
            }
        }

        Self { x_min: x0, y_min: y0, resolution, nx, ny, cells }
    }

    pub fn is_water(&self, x: f64, y: f64, polygons: &[Polyline]) -> bool {
        if self.nx == 0 || self.ny == 0 {
            return false; // no mask → treat as land (don't paint dots)
        }
        let ix = ((x - self.x_min) / self.resolution).floor() as i64;
        let iy = ((y - self.y_min) / self.resolution).floor() as i64;
        if ix < 0 || iy < 0 || (ix as usize) >= self.nx || (iy as usize) >= self.ny {
            return true; // outside SA bbox → water
        }
        match self.cells[iy as usize * self.nx + ix as usize] {
            1 => false,
            2 => !point_in_polygons(x, y, polygons),
            _ => true,
        }
    }
}

fn point_in_polygons(x: f64, y: f64, polygons: &[Polyline]) -> bool {
    let mut inside = false;
    for poly in polygons {
        if poly.len() < 3 {
            continue;
        }
        let mut j = poly.len() - 1;
        for i in 0..poly.len() {
            let (xi, yi) = poly[i];
            let (xj, yj) = poly[j];
            if ((yi > y) != (yj > y))
                && x < (xj - xi) * (y - yi) / (yj - yi) + xi
            {
                inside = !inside;
            }
            j = i;
        }
    }
    inside
}

// ---------- MapData ----------

#[derive(Debug)]
pub struct MapData {
    pub coastline: Vec<Polyline>,
    pub land_polygons: Vec<Polyline>,
    pub land_mask: LandMask,
    pub roads: Vec<Polyline>,
    pub road_bboxes: Vec<BBox>,
    pub buildings: Vec<Building>,
    pub building_grid: GridIndex,
}

impl Default for MapData {
    fn default() -> Self {
        Self {
            coastline: Vec::new(),
            land_polygons: Vec::new(),
            land_mask: LandMask::empty(),
            roads: Vec::new(),
            road_bboxes: Vec::new(),
            buildings: Vec::new(),
            building_grid: GridIndex::empty(),
        }
    }
}

impl MapData {
    pub fn load(base: &Path) -> Self {
        let mut data = MapData::default();

        if let Some(path) = find_file(base, "south-africa", &["geojson", "json"]) {
            eprintln!("[map] coastline ← {}", path.display());
            let (lines, polygons) = load_lines_and_polygons(&path);
            eprintln!(
                "[map]   {} line features, {} polygon rings",
                lines.len(),
                polygons.len()
            );
            data.coastline = lines;
            data.land_polygons = polygons;

            if !data.land_polygons.is_empty() {
                let extent = compute_extent(&data);
                data.land_mask = LandMask::build(&data.land_polygons, extent, 0.005);
                eprintln!(
                    "[map]   land mask {}×{} @ 0.005°",
                    data.land_mask.nx, data.land_mask.ny
                );
            }
        }

        if let Some(path) = find_file(
            base,
            "Public_Service_Infrastructure_National_Roads",
            &["geojson", "json", "csv"],
        ) {
            eprintln!("[map] roads     ← {}", path.display());
            data.roads = load_lines(&path);
            data.road_bboxes = data.roads.iter().map(|p| bbox_of_polyline(p)).collect();
            eprintln!("[map]   {} features", data.roads.len());
        }

        if let Some(path) = find_file(base, "2D_Building_Footprints", &["geojson", "json", "csv"]) {
            eprintln!("[map] buildings ← {}", path.display());
            let is_csv = path
                .extension()
                .map_or(false, |e| e.eq_ignore_ascii_case("csv"));
            data.buildings = if is_csv {
                load_buildings_csv(&path)
            } else {
                load_buildings_geojson(&path)
            };
            eprintln!("[map]   {} footprints", data.buildings.len());

            let extent = compute_extent(&data);
            data.building_grid = GridIndex::build(&data.buildings, extent, 128);
        }

        data
    }
}

fn find_file(base: &Path, stem: &str, exts: &[&str]) -> Option<PathBuf> {
    let candidates = [
        base.to_path_buf(),
        base.join("data"),
        base.join("..").join("data"),
    ];
    for dir in &candidates {
        for ext in exts {
            let p = dir.join(format!("{}.{}", stem, ext));
            if p.exists() {
                return Some(p);
            }
        }
    }
    None
}

// ---------- GeoJSON helpers ----------

fn load_lines(path: &Path) -> Vec<Polyline> {
    if path
        .extension()
        .map_or(false, |e| e.eq_ignore_ascii_case("csv"))
    {
        return load_lines_csv(path);
    }
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    let gj: geojson::GeoJson = match text.parse() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("[map] failed to parse {}: {}", path.display(), e);
            return Vec::new();
        }
    };
    let fc = match gj {
        geojson::GeoJson::FeatureCollection(fc) => fc,
        _ => return Vec::new(),
    };
    let mut out = Vec::new();
    for feat in fc.features {
        if let Some(geom) = feat.geometry {
            collect_lines(&geom.value, &mut out);
        }
    }
    out
}

/// Loads both the drawable line strings *and* the outer rings of any
/// polygons (used to build the land mask).
fn load_lines_and_polygons(path: &Path) -> (Vec<Polyline>, Vec<Polyline>) {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return (Vec::new(), Vec::new()),
    };
    let gj: geojson::GeoJson = match text.parse() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("[map] failed to parse {}: {}", path.display(), e);
            return (Vec::new(), Vec::new());
        }
    };
    let fc = match gj {
        geojson::GeoJson::FeatureCollection(fc) => fc,
        _ => return (Vec::new(), Vec::new()),
    };
    let mut lines = Vec::new();
    let mut polys = Vec::new();
    for feat in fc.features {
        if let Some(geom) = feat.geometry {
            collect_lines(&geom.value, &mut lines);
            collect_outer_rings(&geom.value, &mut polys);
        }
    }
    (lines, polys)
}

fn collect_lines(value: &geojson::Value, out: &mut Vec<Polyline>) {
    use geojson::Value;
    match value {
        Value::LineString(coords) => out.push(coords.iter().map(|c| (c[0], c[1])).collect()),
        Value::MultiLineString(lines) => {
            for l in lines {
                out.push(l.iter().map(|c| (c[0], c[1])).collect());
            }
        }
        Value::Polygon(rings) => {
            for r in rings {
                out.push(r.iter().map(|c| (c[0], c[1])).collect());
            }
        }
        Value::MultiPolygon(polys) => {
            for p in polys {
                for r in p {
                    out.push(r.iter().map(|c| (c[0], c[1])).collect());
                }
            }
        }
        Value::GeometryCollection(geoms) => {
            for g in geoms {
                collect_lines(&g.value, out);
            }
        }
        _ => {}
    }
}

fn collect_outer_rings(value: &geojson::Value, out: &mut Vec<Polyline>) {
    use geojson::Value;
    match value {
        Value::Polygon(rings) => {
            if let Some(outer) = rings.first() {
                out.push(outer.iter().map(|c| (c[0], c[1])).collect());
            }
        }
        Value::MultiPolygon(polys) => {
            for rings in polys {
                if let Some(outer) = rings.first() {
                    out.push(outer.iter().map(|c| (c[0], c[1])).collect());
                }
            }
        }
        Value::GeometryCollection(geoms) => {
            for g in geoms {
                collect_outer_rings(&g.value, out);
            }
        }
        _ => {}
    }
}

fn load_buildings_geojson(path: &Path) -> Vec<Building> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    let gj: geojson::GeoJson = match text.parse() {
        Ok(g) => g,
        Err(_) => return Vec::new(),
    };
    let fc = match gj {
        geojson::GeoJson::FeatureCollection(fc) => fc,
        _ => return Vec::new(),
    };
    let mut out = Vec::new();
    for feat in fc.features {
        if let Some(geom) = feat.geometry {
            collect_buildings(&geom.value, &mut out);
        }
    }
    out
}

fn collect_buildings(value: &geojson::Value, out: &mut Vec<Building>) {
    use geojson::Value;
    match value {
        Value::Polygon(rings) => {
            if let Some(bbox) = bbox_of_rings(rings) {
                out.push(Building { bbox });
            }
        }
        Value::MultiPolygon(polys) => {
            for rings in polys {
                if let Some(bbox) = bbox_of_rings(rings) {
                    out.push(Building { bbox });
                }
            }
        }
        Value::Point(c) => {
            if c.len() >= 2 {
                out.push(Building { bbox: (c[0], c[1], c[0], c[1]) });
            }
        }
        Value::MultiPoint(cs) => {
            for c in cs {
                if c.len() >= 2 {
                    out.push(Building { bbox: (c[0], c[1], c[0], c[1]) });
                }
            }
        }
        Value::GeometryCollection(geoms) => {
            for g in geoms {
                collect_buildings(&g.value, out);
            }
        }
        _ => {}
    }
}

fn bbox_of_rings(rings: &[Vec<Vec<f64>>]) -> Option<BBox> {
    let mut b: BBox = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for ring in rings {
        for c in ring {
            if c.len() >= 2 {
                b.0 = b.0.min(c[0]);
                b.1 = b.1.min(c[1]);
                b.2 = b.2.max(c[0]);
                b.3 = b.3.max(c[1]);
            }
        }
    }
    if b.0.is_finite() { Some(b) } else { None }
}

pub fn bbox_of_polyline(line: &Polyline) -> BBox {
    let mut b: BBox = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for &(x, y) in line {
        b.0 = b.0.min(x);
        b.1 = b.1.min(y);
        b.2 = b.2.max(x);
        b.3 = b.3.max(y);
    }
    b
}

fn compute_extent(data: &MapData) -> BBox {
    let mut b: BBox = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for line in &data.coastline {
        for &(x, y) in line {
            b.0 = b.0.min(x);
            b.1 = b.1.min(y);
            b.2 = b.2.max(x);
            b.3 = b.3.max(y);
        }
    }
    if !b.0.is_finite() {
        b = (16.0, -35.0, 33.0, -22.0); // SA fallback bbox
    }
    b
}

// ---------- CSV helpers ----------

fn load_lines_csv(path: &Path) -> Vec<Polyline> {
    let mut out = Vec::new();
    let mut reader = match csv::Reader::from_path(path) {
        Ok(r) => r,
        Err(_) => return out,
    };
    let headers = match reader.headers() {
        Ok(h) => h.clone(),
        Err(_) => return out,
    };
    let find = |names: &[&str]| -> Option<usize> {
        headers.iter().position(|h| {
            let hl = h.to_lowercase();
            names.iter().any(|n| hl == *n)
        })
    };
    let sla = find(&["start_lat", "start_y"]);
    let slo = find(&["start_lon", "start_x"]);
    let ela = find(&["end_lat", "end_y"]);
    let elo = find(&["end_lon", "end_x"]);
    if let (Some(sla), Some(slo), Some(ela), Some(elo)) = (sla, slo, ela, elo) {
        for rec in reader.records().flatten() {
            let a = rec.get(slo).and_then(|s| s.parse::<f64>().ok()).zip(
                rec.get(sla).and_then(|s| s.parse::<f64>().ok()),
            );
            let b = rec.get(elo).and_then(|s| s.parse::<f64>().ok()).zip(
                rec.get(ela).and_then(|s| s.parse::<f64>().ok()),
            );
            if let (Some((x0, y0)), Some((x1, y1))) = (a, b) {
                out.push(vec![(x0, y0), (x1, y1)]);
            }
        }
    }
    out
}

fn load_buildings_csv(path: &Path) -> Vec<Building> {
    let mut out = Vec::new();
    let mut reader = match csv::Reader::from_path(path) {
        Ok(r) => r,
        Err(_) => return out,
    };
    let headers = match reader.headers() {
        Ok(h) => h.clone(),
        Err(_) => return out,
    };
    let find = |names: &[&str]| -> Option<usize> {
        headers
            .iter()
            .position(|h| names.iter().any(|n| h.eq_ignore_ascii_case(n)))
    };
    let lat_idx = find(&["latitude", "lat", "y"]);
    let lon_idx = find(&["longitude", "lon", "lng", "x"]);
    let (lat_idx, lon_idx) = match (lat_idx, lon_idx) {
        (Some(a), Some(b)) => (a, b),
        _ => return out,
    };
    for rec in reader.records().flatten() {
        let y = rec.get(lat_idx).and_then(|s| s.parse::<f64>().ok());
        let x = rec.get(lon_idx).and_then(|s| s.parse::<f64>().ok());
        if let (Some(y), Some(x)) = (y, x) {
            out.push(Building { bbox: (x, y, x, y) });
        }
    }
    out
}
