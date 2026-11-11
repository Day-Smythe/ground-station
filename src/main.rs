use std::path::Path;

use crate::app::App;

pub mod app;
pub mod event;
pub mod map_data;
pub mod ui;

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    // Load before entering the alternate screen so any progress / errors
    // are visible. Change `Path::new(".")` if your data lives elsewhere.
    let map = map_data::MapData::load(Path::new("."));
    let terminal = ratatui::init();
    let result = App::new(map).run(terminal);
    ratatui::restore();
    result
}
