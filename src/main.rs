use std::path::Path;

use crate::app::App;

pub mod app;
pub mod event;
pub mod map_data;
pub mod ui;

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let map = map_data::MapData::load(Path::new("."));
    let terminal = ratatui::init();
    let result = App::new(map).run(terminal);
    ratatui::restore();
    result
}
