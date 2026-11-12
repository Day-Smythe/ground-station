use crate::event::{AppEvent, Event, EventHandler};
use crate::map_data::MapData;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Color;
use ratatui::DefaultTerminal;

// Cape Peninsula initial viewport.
pub const CAPE_X_MIN: f64 = 18.180_419_613_829_898;
pub const CAPE_X_MAX: f64 = 18.937_943_422_789_093;
pub const CAPE_Y_MIN: f64 = -34.409_448_452_171_05;
pub const CAPE_Y_MAX: f64 = -33.774_412_014_829_124;

const MIN_VIEW_WIDTH: f64 = 0.000_5;
const MAX_VIEW_WIDTH: f64 = 60.0;

#[derive(Debug, Clone)]
pub struct Coordinate {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    pub bearing: f64,
}

#[derive(Debug, Clone)]
pub struct Group {
    pub name: String,
    pub color: Color,
    pub entries: Vec<Coordinate>,
    pub intersections: Vec<(f64, f64)>,
}

#[derive(Debug, Clone)]
pub struct FormState {
    /// Lat, Lon, Bearing, Group
    pub fields: [String; 4],
    pub active: usize,
}

impl Default for FormState {
    fn default() -> Self {
        Self {
            fields: [
                String::new(),
                String::new(),
                String::new(),
                String::new(),
            ],
            active: 0,
        }
    }
}

#[derive(Debug)]
pub struct App {
    pub running: bool,
    pub groups: Vec<Group>,
    pub events: EventHandler,
    pub map: MapData,

    pub view_x_min: f64,
    pub view_x_max: f64,
    pub view_y_min: f64,
    pub view_y_max: f64,

    pub input_mode: bool,
    pub form: FormState,
    pub status: String,
}

impl App {
    pub fn new(map: MapData) -> Self {
        Self {
            running: true,
            groups: Vec::new(),
            events: EventHandler::new(),
            map,
            view_x_min: CAPE_X_MIN,
            view_x_max: CAPE_X_MAX,
            view_y_min: CAPE_Y_MIN,
            view_y_max: CAPE_Y_MAX,
            input_mode: false,
            form: FormState::default(),
            status: String::new(),
        }
    }

    // ---------- viewport helpers ----------

    pub fn view_bbox(&self) -> (f64, f64, f64, f64) {
        (self.view_x_min, self.view_y_min, self.view_x_max, self.view_y_max)
    }

    pub fn pan(&mut self, dx_frac: f64, dy_frac: f64) {
        let dx = (self.view_x_max - self.view_x_min) * dx_frac;
        let dy = (self.view_y_max - self.view_y_min) * dy_frac;
        self.view_x_min += dx;
        self.view_x_max += dx;
        self.view_y_min += dy;
        self.view_y_max += dy;
    }

    pub fn zoom(&mut self, factor: f64) {
        let cx = (self.view_x_min + self.view_x_max) / 2.0;
        let cy = (self.view_y_min + self.view_y_max) / 2.0;
        let mut w = (self.view_x_max - self.view_x_min) * factor;
        let mut h = (self.view_y_max - self.view_y_min) * factor;

        if w > MAX_VIEW_WIDTH {
            let s = MAX_VIEW_WIDTH / w;
            w = MAX_VIEW_WIDTH;
            h *= s;
        }
        if w < MIN_VIEW_WIDTH {
            let s = MIN_VIEW_WIDTH / w;
            w = MIN_VIEW_WIDTH;
            h *= s;
        }

        self.view_x_min = cx - w / 2.0;
        self.view_x_max = cx + w / 2.0;
        self.view_y_min = cy - h / 2.0;
        self.view_y_max = cy + h / 2.0;
    }

    pub fn reset_view(&mut self) {
        self.view_x_min = CAPE_X_MIN;
        self.view_x_max = CAPE_X_MAX;
        self.view_y_min = CAPE_Y_MIN;
        self.view_y_max = CAPE_Y_MAX;
    }

    // ---------- group helpers ----------

    fn next_default_group_name(&self) -> String {
        const NAMES: [&str; 9] = [
            "Ärger", "Bertä", "Cäsar", "Dorä", "Emil",
            "Friedrich", "Gustav", "Heinrich", "Ida",
        ];
        let n = self.groups.len();
        if n < NAMES.len() {
            NAMES[n].to_string()
        } else {
            format!("Group {}", n + 1)
        }
    }

    fn next_group_color(&self) -> Color {
        const COLORS: [Color; 12] = [
            Color::LightRed, Color::LightGreen, Color::LightYellow, Color::LightCyan,
            Color::LightMagenta, Color::LightBlue, Color::Red, Color::Green,
            Color::Yellow, Color::Cyan, Color::Magenta, Color::Blue,
        ];
        COLORS[self.groups.len() % COLORS.len()]
    }

    // ---------- event loop ----------

    pub fn run(mut self, mut terminal: DefaultTerminal) -> color_eyre::Result<()> {
        let mut dirty = true;

        while self.running {
            if dirty {
                let t = std::time::Instant::now();
                terminal.draw(|frame| frame.render_widget(&self, frame.area()))?;
                let elapsed = t.elapsed();
                if elapsed.as_millis() > 100 {
                    // eprintln!("[perf] draw took {:?}", elapsed);
                }
                dirty = false;
            }

            // Block for one event, then drain the rest non-blocking.
            // Ticks return `false` (no state change) so a backlog of
            // them is consumed in microseconds with no redraws.
            dirty |= self.process_event(self.events.next()?)?;
            while self.running {
                match self.events.try_next() {
                    Some(ev) => dirty |= self.process_event(ev)?,
                    None => break,
                }
            }
        }
        Ok(())
    }

    /// Apply one event. Returns `true` if state changed (i.e. a redraw
    /// is warranted).
    fn process_event(&mut self, event: Event) -> color_eyre::Result<bool> {
        Ok(match event {
            Event::Tick => {
                self.tick();
                false
            }
            Event::Crossterm(crossterm::event::Event::Key(key_event))
                if key_event.kind == crossterm::event::KeyEventKind::Press =>
            {
                self.handle_key_event(key_event)?;
                true
            }
            Event::Crossterm(_) => false,
            Event::App(AppEvent::Quit) => {
                self.quit();
                true
            }
        })
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> color_eyre::Result<()> {
        if self.input_mode {
            return self.handle_form_key(key_event);
        }

        const PAN_STEP: f64 = 0.05;
        match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') => self.events.send(AppEvent::Quit),
            KeyCode::Char('c' | 'C') if key_event.modifiers == KeyModifiers::CONTROL => {
                self.events.send(AppEvent::Quit)
            }
            KeyCode::Char('`') => {
                self.input_mode = true;
                self.form = FormState::default();
                self.status.clear();
            }
            KeyCode::Left => self.pan(-PAN_STEP, 0.0),
            KeyCode::Right => self.pan(PAN_STEP, 0.0),
            KeyCode::Up => self.pan(0.0, PAN_STEP),
            KeyCode::Down => self.pan(0.0, -PAN_STEP),
            KeyCode::Char('+') | KeyCode::Char('=') | KeyCode::PageUp => self.zoom(0.8),
            KeyCode::Char('-') | KeyCode::Char('_') | KeyCode::PageDown => self.zoom(1.25),
            KeyCode::Char('r' | 'R') => self.reset_view(),
            _ => {}
        }
        Ok(())
    }

    fn handle_form_key(&mut self, key_event: KeyEvent) -> color_eyre::Result<()> {
        if key_event.code == KeyCode::Char('c')
            && key_event.modifiers.contains(KeyModifiers::CONTROL)
        {
            self.events.send(AppEvent::Quit);
            return Ok(());
        }

        match key_event.code {
            KeyCode::Esc => {
                self.input_mode = false;
                self.status.clear();
            }
            KeyCode::Tab => {
                self.form.active = (self.form.active + 1) % 4;
            }
            KeyCode::BackTab => {
                self.form.active = (self.form.active + 3) % 4;
            }
            KeyCode::Enter => self.submit_form(),
            KeyCode::Backspace => {
                self.form.fields[self.form.active].pop();
            }
            KeyCode::Char(c) => {
                self.form.fields[self.form.active].push(c);
            }
            _ => {}
        }
        Ok(())
    }

    fn submit_form(&mut self) {
        let parse = |s: &str| -> Option<f64> { s.trim().parse().ok() };

        let lat = match parse(&self.form.fields[0]) {
            Some(v) => v,
            None => { self.status = "Invalid latitude".into(); return; }
        };
        let lon = match parse(&self.form.fields[1]) {
            Some(v) => v,
            None => { self.status = "Invalid longitude".into(); return; }
        };
        let bearing = match parse(&self.form.fields[2]) {
            Some(v) => v,
            None => { self.status = "Invalid bearing".into(); return; }
        };

        let group_name = if self.form.fields[3].trim().is_empty() {
            self.next_default_group_name()
        } else {
            self.form.fields[3].trim().to_string()
        };

        let idx = match self.groups.iter().position(|g| g.name == group_name) {
            Some(i) => i,
            None => {
                let color = self.next_group_color();
                self.groups.push(Group {
                    name: group_name.clone(),
                    color,
                    entries: Vec::new(),
                    intersections: Vec::new(),
                });
                self.groups.len() - 1
            }
        };

        let base = self.groups[idx].name.clone();
        let n = self.groups[idx].entries.len();
        self.groups[idx].entries.push(Coordinate {
            name: format!("{}-{:02}", base, n + 1),
            lat,
            lon,
            bearing,
        });

        self.groups[idx].intersections = triangulate(&self.groups[idx].entries);

        self.input_mode = false;
        self.status.clear();
        self.form = FormState::default();
    }

    pub fn tick(&self) {}

    pub fn quit(&mut self) {
        self.running = false;
    }
}

/// Least-squares intersection of the group's bearing rays.
fn triangulate(entries: &[Coordinate]) -> Vec<(f64, f64)> {
    if entries.len() < 2 {
        return Vec::new();
    }
    let (mut a00, mut a01, mut a11) = (0.0f64, 0.0f64, 0.0f64);
    let (mut r0, mut r1) = (0.0f64, 0.0f64);
    for e in entries {
        let theta = e.bearing.to_radians();
        let ct = theta.cos();
        let st = theta.sin();
        let a = ct;
        let b = -st;
        let c = ct * e.lon - st * e.lat;
        a00 += a * a;
        a01 += a * b;
        a11 += b * b;
        r0 += a * c;
        r1 += b * c;
    }
    let det = a00 * a11 - a01 * a01;
    if det.abs() < 1e-14 {
        let n = entries.len() as f64;
        let lat = entries.iter().map(|e| e.lat).sum::<f64>() / n;
        let lon = entries.iter().map(|e| e.lon).sum::<f64>() / n;
        return vec![(lat, lon)];
    }
    let x = (a11 * r0 - a01 * r1) / det;
    let y = (a00 * r1 - a01 * r0) / det;
    vec![(y, x)]
}