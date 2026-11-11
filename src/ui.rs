use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        canvas::{Canvas, Context, Line as CanvasLine},
        Block, BorderType, Paragraph, Widget,
    },
};

use crate::app::App;
use crate::map_data::{BBox, Polyline};

const BUILDING_LOD_DPX: f64 = 3000.0;
const ROAD_MIN_DOTS: f64 = 2.5;
const BUILDING_MIN_DOTS: f64 = 0.6;

/// Per-frame building cap. Lowered from 5000 to 2000 — dense urban views
/// can still fill in nicely because the loop drains properly now, but this
/// keeps the worst-case draw predictable on slow terminals.
const MAX_BUILDINGS_PER_FRAME: usize = 2_000;

/// Water dot spacing in braille dots (≈ 8 characters between dots).
/// Doubled from 8 to 16 — cuts water-layer draw volume by ~4×.
const WATER_DOT_SPACING: f64 = 16.0;

fn marker_radius(dpx: f64, dpy: f64, desired_dots: f64, max_deg: f64) -> f64 {
    let dp = dpx.min(dpy).max(1e-12);
    (desired_dots / (2.0 * dp)).clamp(1e-10, max_deg)
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let [left, middle_full, right] = Layout::horizontal([
            Constraint::Percentage(25),
            Constraint::Percentage(50),
            Constraint::Percentage(25),
        ])
        .spacing(1)
        .areas(area);

        let map_area = if self.input_mode {
            let [form_area, map_area] =
                Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).areas(middle_full);
            self.render_form(form_area, buf);
            map_area
        } else {
            middle_full
        };

        self.render_stations(left, buf);
        self.render_map(map_area, buf);
        self.render_intersections(right, buf);
    }
}

impl App {
    fn render_stations(&self, area: Rect, buf: &mut Buffer) {
        let mut lines: Vec<Line> = Vec::new();
        if self.groups.is_empty() {
            lines.push(Line::from(Span::styled(
                "No groups yet.",
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::from(Span::styled(
                "Press ` to add a station.",
                Style::default().fg(Color::DarkGray),
            )));
        }
        for group in &self.groups {
            lines.push(Line::from(Span::styled(
                format!("◆ {}", group.name),
                Style::default()
                    .fg(group.color)
                    .add_modifier(Modifier::BOLD),
            )));
            for e in &group.entries {
                lines.push(Line::from(vec![
                    Span::raw(format!("  {:<12}", e.name)),
                    Span::styled(
                        format!("{:>6.1}°", e.bearing),
                        Style::default().fg(group.color),
                    ),
                ]));
            }
            lines.push(Line::from(""));
        }

        Paragraph::new(lines)
            .block(
                Block::bordered()
                    .title("Stations")
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::DarkGray)),
            )
            .render(area, buf);
    }

    fn render_intersections(&self, area: Rect, buf: &mut Buffer) {
        let mut lines: Vec<Line> = Vec::new();
        if self.groups.is_empty() {
            lines.push(Line::from(Span::styled(
                "No fixes yet.",
                Style::default().fg(Color::DarkGray),
            )));
        }
        for group in &self.groups {
            lines.push(Line::from(Span::styled(
                format!("◆ {}", group.name),
                Style::default()
                    .fg(group.color)
                    .add_modifier(Modifier::BOLD),
            )));
            if group.intersections.is_empty() {
                lines.push(Line::from(Span::styled(
                    "  (need ≥2 stations)",
                    Style::default().fg(Color::DarkGray),
                )));
            } else {
                for (lat, lon) in &group.intersections {
                    lines.push(Line::from(vec![
                        Span::styled("  + ", Style::default().fg(group.color)),
                        Span::raw(format!("{:.5}, {:.5}", lat, lon)),
                    ]));
                }
            }
            lines.push(Line::from(""));
        }

        Paragraph::new(lines)
            .block(
                Block::bordered()
                    .title("Fixes")
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::DarkGray)),
            )
            .render(area, buf);
    }

    fn render_form(&self, area: Rect, buf: &mut Buffer) {
        let labels = ["Lat", "Lon", "Brg", "Grp"];
        let mut spans: Vec<Span> = Vec::new();
        for (i, label) in labels.iter().enumerate() {
            let content = &self.form.fields[i];
            let text = format!("{}:{:<15}", label, content);
            let style = if i == self.form.active {
                Style::default().bg(Color::LightCyan).fg(Color::Black)
            } else {
                Style::default().fg(Color::Gray)
            };
            spans.push(Span::styled(text, style));
            spans.push(Span::raw(" "));
        }

        let title = if self.status.is_empty() {
            "New station  ·  Tab next · Enter add · Esc cancel".to_string()
        } else {
            format!("New station  ·  {}", self.status)
        };

        Paragraph::new(Line::from(spans))
            .block(
                Block::bordered()
                    .title(title)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::LightCyan)),
            )
            .render(area, buf);
    }

    fn render_map(&self, area: Rect, buf: &mut Buffer) {
        let view = self.view_bbox();
        let width_deg = self.view_x_max - self.view_x_min;
        let height_deg = self.view_y_max - self.view_y_min;

        let dpx = (area.width as f64 * 2.0) / width_deg.max(1e-12);
        let dpy = (area.height as f64 * 4.0) / height_deg.max(1e-12);

        let station_r = marker_radius(dpx, dpy, 4.0, 0.05);
        let fix_r = marker_radius(dpx, dpy, 6.0, 0.08);

        let title = format!(
            "Map  ·  ←→↑↓ pan  ·  +/− zoom  ·  ` new station  ·  r reset  ·  width {:.5}°",
            width_deg
        );

        let block = Block::bordered()
            .title(title)
            .title_alignment(Alignment::Center)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray));

        let has_land = !self.map.land_polygons.is_empty();

        let canvas = Canvas::default()
            .block(block)
            .x_bounds([self.view_x_min, self.view_x_max])
            .y_bounds([self.view_y_min, self.view_y_max])
            .paint(|ctx| {
                if has_land {
                    let dots_x = (area.width as f64 * 2.0 / WATER_DOT_SPACING).max(1.0);
                    let dots_y = (area.height as f64 * 4.0 / WATER_DOT_SPACING).max(1.0);
                    let step_x = width_deg / dots_x;
                    let step_y = height_deg / dots_y;
                    let water = Color::Rgb(30, 50, 100);

                    let mut y = self.view_y_min;
                    while y <= self.view_y_max {
                        let mut x = self.view_x_min;
                        while x <= self.view_x_max {
                            if self
                                .map
                                .land_mask
                                .is_water(x, y, &self.map.land_polygons)
                            {
                                ctx.draw(&CanvasLine {
                                    x1: x,
                                    y1: y,
                                    x2: x,
                                    y2: y,
                                    color: water,
                                });
                            }
                            x += step_x;
                        }
                        y += step_y;
                    }
                }

                for line in &self.map.coastline {
                    draw_polyline_culled(ctx, line, view, Color::Gray);
                }

                for (line, bbox) in self.map.roads.iter().zip(&self.map.road_bboxes) {
                    let w_dots = (bbox.2 - bbox.0) * dpx;
                    let h_dots = (bbox.3 - bbox.1) * dpy;
                    if w_dots.max(h_dots) < ROAD_MIN_DOTS {
                        continue;
                    }
                    if !bbox_intersects(*bbox, view) {
                        continue;
                    }
                    draw_polyline_culled(ctx, line, view, Color::Indexed(240));
                }

                if dpx >= BUILDING_LOD_DPX {
                    let mut drawn = 0usize;
                    for idx in self.map.building_grid.query(view) {
                        if drawn >= MAX_BUILDINGS_PER_FRAME {
                            break;
                        }
                        let b = &self.map.buildings[idx as usize];
                        let w_dots = (b.bbox.2 - b.bbox.0) * dpx;
                        let h_dots = (b.bbox.3 - b.bbox.1) * dpy;
                        if w_dots < BUILDING_MIN_DOTS && h_dots < BUILDING_MIN_DOTS {
                            let (x, y) = (b.bbox.0, b.bbox.1);
                            ctx.draw(&CanvasLine {
                                x1: x, y1: y, x2: x, y2: y,
                                color: Color::Indexed(238),
                            });
                        } else {
                            let (x0, y0, x1, y1) = b.bbox;
                            let c = Color::Indexed(238);
                            ctx.draw(&CanvasLine { x1: x0, y1: y0, x2: x1, y2: y0, color: c });
                            ctx.draw(&CanvasLine { x1: x1, y1: y0, x2: x1, y2: y1, color: c });
                            ctx.draw(&CanvasLine { x1: x1, y1: y1, x2: x0, y2: y1, color: c });
                            ctx.draw(&CanvasLine { x1: x0, y1: y1, x2: x0, y2: y0, color: c });
                        }
                        drawn += 1;
                    }
                }

                for group in &self.groups {
                    for (lat, lon) in &group.intersections {
                        for e in &group.entries {
                            draw_dotted_line(ctx, e.lon, e.lat, *lon, *lat, group.color, dpx, dpy);
                        }
                    }
                    for e in &group.entries {
                        draw_station_marker(ctx, e.lon, e.lat, group.color, station_r);
                    }
                    for (lat, lon) in &group.intersections {
                        draw_fix_marker(ctx, *lon, *lat, group.color, fix_r);
                    }
                }
            });

        canvas.render(area, buf);
    }
}

fn bbox_intersects(a: BBox, b: BBox) -> bool {
    !(a.2 < b.0 || a.0 > b.2 || a.3 < b.1 || a.1 > b.3)
}

fn draw_polyline_culled(ctx: &mut Context<'_>, line: &Polyline, view: BBox, color: Color) {
    if line.len() < 2 {
        return;
    }
    let (vx0, vy0, vx1, vy1) = view;
    for w in line.windows(2) {
        let (x1, y1) = w[0];
        let (x2, y2) = w[1];
        let sx0 = x1.min(x2);
        let sy0 = y1.min(y2);
        let sx1 = x1.max(x2);
        let sy1 = y1.max(y2);
        if sx1 < vx0 || sx0 > vx1 || sy1 < vy0 || sy0 > vy1 {
            continue;
        }
        ctx.draw(&CanvasLine { x1, y1, x2, y2, color });
    }
}

fn draw_station_marker(ctx: &mut Context<'_>, x: f64, y: f64, color: Color, r: f64) {
    let c = [
        (x - r, y - r),
        (x + r, y - r),
        (x + r, y + r),
        (x - r, y + r),
    ];
    for i in 0..4 {
        ctx.draw(&CanvasLine {
            x1: c[i].0,
            y1: c[i].1,
            x2: c[(i + 1) % 4].0,
            y2: c[(i + 1) % 4].1,
            color,
        });
    }
}

fn draw_fix_marker(ctx: &mut Context<'_>, x: f64, y: f64, color: Color, r: f64) {
    ctx.draw(&CanvasLine { x1: x - r, y1: y, x2: x + r, y2: y, color });
    ctx.draw(&CanvasLine { x1: x, y1: y - r, x2: x, y2: y + r, color });
    let d = r * 0.6;
    ctx.draw(&CanvasLine { x1: x - d, y1: y - d, x2: x + d, y2: y + d, color });
    ctx.draw(&CanvasLine { x1: x - d, y1: y + d, x2: x + d, y2: y - d, color });
}

fn draw_dotted_line(
    ctx: &mut Context<'_>,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    color: Color,
    dpx: f64,
    dpy: f64,
) {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-12 {
        return;
    }
    let dp = dpx.min(dpy).max(1e-12);
    let n = (len * dp / 4.0).clamp(1.0, 100.0) as usize;
    let seg = len / n as f64;
    let dash = seg * 0.5;
    let ux = dx / len;
    let uy = dy / len;
    for i in 0..n {
        let t0 = i as f64 * seg;
        let t1 = (t0 + dash).min(len);
        ctx.draw(&CanvasLine {
            x1: x1 + ux * t0,
            y1: y1 + uy * t0,
            x2: x1 + ux * t1,
            y2: y1 + uy * t1,
            color,
        });
    }
}
