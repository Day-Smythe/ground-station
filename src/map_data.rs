use std::fs;
use std::path::{Path, PathBuf};

pub type Point = (f64, f64);
pub type Polyline = Vec<Point>;
pub type BBox = (f64, f64, f64, f64); // minx, miny, maxx, maxy

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
            let mut crossings: Vec<f64> = Vec::new();
            for &(x1p, y1p, x2p, y2p) in &edges {
                if (y1p <= y && y2p > y) || (y2p <= y && y1p > y) {
                    let t = (y - y1p) / (y2p - y1p);
                    crossings.push(x1p + t * (x2p - x1p));
                }
            }
            crossings.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

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
            return false;
        }
        let ix = ((x - self.x_min) / self.resolution).floor() as i64;
        let iy = ((y - self.y_min) / self.resolution).floor() as i64;
        if ix < 0 || iy < 0 || (ix as usize) >= self.nx || (iy as usize) >= self.ny {
            return true;
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
            if ((yi > y) != (yj > y)) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
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

    /// Minor roads (TCT road centerlines).
    pub minor_roads: Vec<Polyline>,
    pub minor_road_bboxes: Vec<BBox>,

    /// Major roads (national roads).
    pub major_roads: Vec<Polyline>,
    pub major_road_bboxes: Vec<BBox>,
}

impl Default for MapData {
    fn default() -> Self {
        Self {
            coastline: Vec::new(),
            land_polygons: Vec::new(),
            land_mask: LandMask::empty(),
            minor_roads: Vec::new(),
            minor_road_bboxes: Vec::new(),
            major_roads: Vec::new(),
            major_road_bboxes: Vec::new(),
        }
    }
}

impl MapData {
    pub fn load(base: &Path) -> Self {
        let mut data = MapData::default();

        if let Some(path) = find_file(base, "south-africa", &["geojson", "json"]) {
            eprintln!("[map] coastline   ← {}", path.display());
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
        } else {
            eprintln!("[map] coastline   ← (missing south-africa.geojson)");
        }

        if let Some(path) = find_file(base, "TCT_Road_Centerline", &["geojson", "json", "csv"]) {
            eprintln!("[map] minor roads ← {}", path.display());
            data.minor_roads = load_lines(&path);
            data.minor_road_bboxes = data
                .minor_roads
                .iter()
                .map(|p| bbox_of_polyline(p))
                .collect();
            eprintln!("[map]   {} features", data.minor_roads.len());
        } else {
            eprintln!("[map] minor roads ← (missing TCT_Road_Centerline.geojson)");
        }

        if let Some(path) = find_file(
            base,
            "Public_Service_Infrastructure_National_Roads",
            &["geojson", "json", "csv"],
        ) {
            eprintln!("[map] major roads ← {}", path.display());
            data.major_roads = load_lines(&path);
            data.major_road_bboxes = data
                .major_roads
                .iter()
                .map(|p| bbox_of_polyline(p))
                .collect();
            eprintln!("[map]   {} features", data.major_roads.len());
        } else {
            eprintln!(
                "[map] major roads ← (missing Public_Service_Infrastructure_National_Roads.*)"
            );
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
        b = (16.0, -35.0, 33.0, -22.0);
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