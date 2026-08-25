mod braille;
mod chrome;
mod layout;
mod spark;
mod theme;
mod widgets;

pub use braille::{BrailleCell, braille_cell, render_cells, render_history};
pub use chrome::{
    Axis, Graph, GraphInk, cell, panel_title, push_kv, push_token, render_scaled_graph,
};
pub use layout::{Degrade, Hit, LayoutFlags, LayoutPlan, Panel, Region, hit_test, plan};
pub use theme::Theme;
pub use widgets::{
    AppView, DetailAction, Focus, ProcView, detail_actions, detail_rect, filtered_processes,
    footer_hit, inner_process_area, render_app,
};
