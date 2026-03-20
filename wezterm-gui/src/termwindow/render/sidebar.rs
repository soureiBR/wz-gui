//! SoureiGate sidebar rendering
//!
//! Renders a sidebar panel on the left side of the terminal window
//! showing server categories and servers from the SoureiGate session.

use crate::quad::TripleLayerQuadAllocator;
use crate::termwindow::render::RenderScreenLineParams;
use mux::renderable::RenderableDimensions;
use termwiz::cell::{CellAttributes, Intensity};
use termwiz::surface::Line;
use wezterm_term::color::ColorAttribute;
use window::color::LinearRgba;

impl super::super::TermWindow {
    pub fn paint_sidebar(
        &mut self,
        layers: &mut TripleLayerQuadAllocator,
    ) -> anyhow::Result<()> {
        if !self.soureigate_sidebar_visible {
            return Ok(());
        }

        let sidebar_width = self.soureigate_sidebar_width;
        let border = self.get_os_border();
        let tab_bar_height = if self.show_tab_bar && !self.config.tab_bar_at_bottom {
            self.tab_bar_pixel_height()?
        } else {
            0.
        };

        let sidebar_x = border.left.get() as f32;
        let sidebar_y = border.top.get() as f32 + tab_bar_height;
        let sidebar_h =
            self.dimensions.pixel_height as f32 - sidebar_y - border.bottom.get() as f32;
        let cell_height = self.render_metrics.cell_size.height as f32;
        let cell_width = self.render_metrics.cell_size.width as f32;
        let max_cols = ((sidebar_width - 4.0) / cell_width) as usize;

        // Colors
        let bg_color =
            LinearRgba::with_components(17.0 / 255.0, 17.0 / 255.0, 27.0 / 255.0, 1.0);
        let sep_color =
            LinearRgba::with_components(69.0 / 255.0, 71.0 / 255.0, 90.0 / 255.0, 1.0);

        // Sidebar background
        self.filled_rectangle(
            layers,
            0,
            euclid::rect(sidebar_x, sidebar_y, sidebar_width, sidebar_h),
            bg_color,
        )?;

        // Separator line
        self.filled_rectangle(
            layers,
            1,
            euclid::rect(sidebar_x + sidebar_width - 1.0, sidebar_y, 1.0, sidebar_h),
            sep_color,
        )?;

        // Build lines for each sidebar row
        let session = crate::soureigate_auth::get_session();
        let mut lines: Vec<Line> = Vec::new();

        if let Some(session) = session {
            // Title line
            let title_line = Line::from_text(
                &pad_to("  SoureiGate", max_cols),
                &title_attr(),
                termwiz::surface::SEQ_ZERO,
                None,
            );
            lines.push(title_line);

            // Empty separator
            lines.push(Line::from_text(
                &pad_to("", max_cols),
                &default_attr(),
                termwiz::surface::SEQ_ZERO,
                None,
            ));

            for category in &session.categories {
                // Category header
                let header = format!(" {} ({})", category.name, category.servers.len());
                lines.push(Line::from_text(
                    &pad_to(&header, max_cols),
                    &category_attr(),
                    termwiz::surface::SEQ_ZERO,
                    None,
                ));

                // Server items
                for server in &category.servers {
                    let label = format!("   {}", server.name);
                    lines.push(Line::from_text(
                        &pad_to(&label, max_cols),
                        &server_attr(),
                        termwiz::surface::SEQ_ZERO,
                        None,
                    ));
                }

                // Gap after category
                lines.push(Line::from_text(
                    &pad_to("", max_cols),
                    &default_attr(),
                    termwiz::surface::SEQ_ZERO,
                    None,
                ));
            }
        } else {
            lines.push(Line::from_text(
                &pad_to("  No servers", max_cols),
                &default_attr(),
                termwiz::surface::SEQ_ZERO,
                None,
            ));
        }

        // Render each line using render_screen_line
        let palette = self.palette().clone();
        let window_is_transparent =
            !self.window_background.is_empty() || self.config.window_background_opacity != 1.0;
        let gl_state = self.render_state.as_ref().unwrap();
        let white_space = gl_state.util_sprites.white_space.texture_coords();
        let filled_box = gl_state.util_sprites.filled_box.texture_coords();
        let default_bg = bg_color;

        let max_visible = (sidebar_h / cell_height) as usize;

        for (i, line) in lines.iter().enumerate() {
            if i >= max_visible {
                break;
            }

            let top_pixel_y = sidebar_y + (i as f32 * cell_height);

            self.render_screen_line(
                RenderScreenLineParams {
                    top_pixel_y,
                    left_pixel_x: sidebar_x,
                    pixel_width: sidebar_width - 1.0,
                    stable_line_idx: None,
                    line,
                    selection: 0..0,
                    cursor: &Default::default(),
                    palette: &palette,
                    dims: &RenderableDimensions {
                        cols: max_cols,
                        physical_top: 0,
                        scrollback_rows: 0,
                        scrollback_top: 0,
                        viewport_rows: 1,
                        dpi: self.terminal_size.dpi,
                        pixel_height: self.render_metrics.cell_size.height as usize,
                        pixel_width: sidebar_width as usize,
                        reverse_video: false,
                    },
                    config: &self.config,
                    cursor_border_color: LinearRgba::default(),
                    foreground: LinearRgba::with_components(
                        205.0 / 255.0,
                        214.0 / 255.0,
                        244.0 / 255.0,
                        1.0,
                    ),
                    pane: None,
                    is_active: true,
                    selection_fg: LinearRgba::default(),
                    selection_bg: LinearRgba::default(),
                    cursor_fg: LinearRgba::default(),
                    cursor_bg: LinearRgba::default(),
                    cursor_is_default_color: true,
                    white_space,
                    filled_box,
                    window_is_transparent,
                    default_bg,
                    style: None,
                    font: None,
                    use_pixel_positioning: self.config.experimental_pixel_positioning,
                    render_metrics: self.render_metrics,
                    shape_key: None,
                    password_input: false,
                },
                layers,
            )?;
        }

        Ok(())
    }
}

// Helper: pad or truncate string to fit sidebar width
fn pad_to(s: &str, width: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() >= width {
        chars[..width].iter().collect()
    } else {
        let mut result: String = chars.into_iter().collect();
        result.extend(std::iter::repeat(' ').take(width - result.len()));
        result
    }
}

// Catppuccin Mocha color attributes
fn title_attr() -> CellAttributes {
    let mut attr = CellAttributes::default();
    attr.set_foreground(ColorAttribute::TrueColorWithDefaultFallback(
        termwiz::color::SrgbaTuple(180.0 / 255.0, 190.0 / 255.0, 254.0 / 255.0, 1.0), // Lavender
    ));
    attr.set_background(ColorAttribute::TrueColorWithDefaultFallback(
        termwiz::color::SrgbaTuple(24.0 / 255.0, 24.0 / 255.0, 37.0 / 255.0, 1.0), // Mantle
    ));
    attr.set_intensity(Intensity::Bold);
    attr
}

fn category_attr() -> CellAttributes {
    let mut attr = CellAttributes::default();
    attr.set_foreground(ColorAttribute::TrueColorWithDefaultFallback(
        termwiz::color::SrgbaTuple(137.0 / 255.0, 180.0 / 255.0, 250.0 / 255.0, 1.0), // Blue
    ));
    attr.set_background(ColorAttribute::TrueColorWithDefaultFallback(
        termwiz::color::SrgbaTuple(49.0 / 255.0, 50.0 / 255.0, 68.0 / 255.0, 1.0), // Surface0
    ));
    attr.set_intensity(Intensity::Bold);
    attr
}

fn server_attr() -> CellAttributes {
    let mut attr = CellAttributes::default();
    attr.set_foreground(ColorAttribute::TrueColorWithDefaultFallback(
        termwiz::color::SrgbaTuple(205.0 / 255.0, 214.0 / 255.0, 244.0 / 255.0, 1.0), // Text
    ));
    attr.set_background(ColorAttribute::TrueColorWithDefaultFallback(
        termwiz::color::SrgbaTuple(17.0 / 255.0, 17.0 / 255.0, 27.0 / 255.0, 1.0), // Crust
    ));
    attr
}

fn default_attr() -> CellAttributes {
    let mut attr = CellAttributes::default();
    attr.set_foreground(ColorAttribute::TrueColorWithDefaultFallback(
        termwiz::color::SrgbaTuple(108.0 / 255.0, 112.0 / 255.0, 134.0 / 255.0, 1.0), // Overlay0
    ));
    attr.set_background(ColorAttribute::TrueColorWithDefaultFallback(
        termwiz::color::SrgbaTuple(17.0 / 255.0, 17.0 / 255.0, 27.0 / 255.0, 1.0), // Crust
    ));
    attr
}
