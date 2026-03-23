//! SoureiGate sidebar rendering
//!
//! Renders a styled sidebar panel on the left side of the terminal window
//! showing server categories (collapsible) and servers with status indicators.

use crate::quad::TripleLayerQuadAllocator;
use crate::termwindow::render::RenderScreenLineParams;
use crate::termwindow::{UIItem, UIItemType};
use mux::renderable::RenderableDimensions;
use termwiz::cell::{CellAttributes, Intensity};
use termwiz::color::SrgbaTuple;
use termwiz::surface::Line;
use wezterm_term::color::ColorAttribute;
use window::color::LinearRgba;

// ── Catppuccin Mocha palette ──────────────────────────────────────────
const LAVENDER: SrgbaTuple = SrgbaTuple(180.0 / 255.0, 190.0 / 255.0, 254.0 / 255.0, 1.0);
const BLUE: SrgbaTuple = SrgbaTuple(137.0 / 255.0, 180.0 / 255.0, 250.0 / 255.0, 1.0);
const YELLOW: SrgbaTuple = SrgbaTuple(249.0 / 255.0, 226.0 / 255.0, 175.0 / 255.0, 1.0);
const GREEN: SrgbaTuple = SrgbaTuple(166.0 / 255.0, 227.0 / 255.0, 161.0 / 255.0, 1.0);
const RED: SrgbaTuple = SrgbaTuple(243.0 / 255.0, 139.0 / 255.0, 168.0 / 255.0, 1.0);
const TEXT: SrgbaTuple = SrgbaTuple(205.0 / 255.0, 214.0 / 255.0, 244.0 / 255.0, 1.0);
const OVERLAY0: SrgbaTuple = SrgbaTuple(108.0 / 255.0, 112.0 / 255.0, 134.0 / 255.0, 1.0);
const SURFACE1: SrgbaTuple = SrgbaTuple(69.0 / 255.0, 71.0 / 255.0, 90.0 / 255.0, 1.0);
const SURFACE0: SrgbaTuple = SrgbaTuple(49.0 / 255.0, 50.0 / 255.0, 68.0 / 255.0, 1.0);
const MANTLE: SrgbaTuple = SrgbaTuple(24.0 / 255.0, 24.0 / 255.0, 37.0 / 255.0, 1.0);
const CRUST: SrgbaTuple = SrgbaTuple(17.0 / 255.0, 17.0 / 255.0, 27.0 / 255.0, 1.0);

/// Tracks the type of each rendered sidebar row for hit-testing
enum SidebarRow {
    Category(usize),
    Server { cat_idx: usize, srv_idx: usize },
    Static,
}

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

        // Guard against zero/tiny cell dimensions during early init
        if cell_width < 1.0 || cell_height < 1.0 || sidebar_h <= 0.0 {
            return Ok(());
        }

        let max_cols = (((sidebar_width - 4.0) / cell_width) as usize).min(256);

        // Background colors
        let bg_color = LinearRgba::with_components(CRUST.0, CRUST.1, CRUST.2, 1.0);
        let sep_color = LinearRgba::with_components(SURFACE1.0, SURFACE1.1, SURFACE1.2, 1.0);

        // Sidebar background
        self.filled_rectangle(
            layers,
            0,
            euclid::rect(sidebar_x, sidebar_y, sidebar_width, sidebar_h),
            bg_color,
        )?;

        // Right edge separator line
        self.filled_rectangle(
            layers,
            1,
            euclid::rect(sidebar_x + sidebar_width - 1.0, sidebar_y, 1.0, sidebar_h),
            sep_color,
        )?;

        // Build lines for each sidebar row
        let session = crate::soureigate_auth::get_session();
        let mut lines: Vec<Line> = Vec::new();
        let mut row_types: Vec<SidebarRow> = Vec::new();

        if let Some(session) = session {
            // ── Title ──
            lines.push(Line::from_text(
                &pad_to("  \u{25C7} SoureiGate", max_cols),
                &make_attr(LAVENDER, MANTLE, true),
                termwiz::surface::SEQ_ZERO,
                None,
            ));
            row_types.push(SidebarRow::Static);

            // Title underline separator
            let sep_text = format!("  {}", "\u{2500}".repeat(max_cols.saturating_sub(3)));
            lines.push(Line::from_text(
                &pad_to(&sep_text, max_cols),
                &make_attr(SURFACE1, CRUST, false),
                termwiz::surface::SEQ_ZERO,
                None,
            ));
            row_types.push(SidebarRow::Static);

            // Empty line after title
            lines.push(Line::from_text(
                &pad_to("", max_cols),
                &make_attr(OVERLAY0, CRUST, false),
                termwiz::surface::SEQ_ZERO,
                None,
            ));
            row_types.push(SidebarRow::Static);

            for (cat_idx, category) in session.categories.iter().enumerate() {
                let is_collapsed = self.soureigate_collapsed.contains(&cat_idx);
                let arrow = if is_collapsed { "\u{25B8}" } else { "\u{25BE}" };

                // Count online servers
                let online = category
                    .servers
                    .iter()
                    .filter(|s| {
                        matches!(
                            s.status.to_lowercase().as_str(),
                            "online" | "active" | "running"
                        )
                    })
                    .count();
                let total = category.servers.len();
                let count_str = format!("{}/{}", online, total);

                let header_text = format!(
                    " {} \u{25AA} {} ({})",
                    arrow, category.name, count_str,
                );

                lines.push(Line::from_text(
                    &pad_to(&header_text, max_cols),
                    &make_attr(BLUE, SURFACE0, true),
                    termwiz::surface::SEQ_ZERO,
                    None,
                ));
                row_types.push(SidebarRow::Category(cat_idx));

                // Server items (only if expanded)
                if !is_collapsed {
                    for (srv_idx, server) in category.servers.iter().enumerate() {
                        let dot_color = status_color(&server.status);
                        let server_text = format!("   \u{25CF} {}", server.name);

                        lines.push(Line::from_text(
                            &pad_to(&server_text, max_cols),
                            &make_attr(dot_color, CRUST, false),
                            termwiz::surface::SEQ_ZERO,
                            None,
                        ));
                        row_types.push(SidebarRow::Server { cat_idx, srv_idx });
                    }

                    // Dashed separator after expanded category
                    let dash = "\u{2500} ".repeat((max_cols.saturating_sub(2)) / 2);
                    let sep = format!("  {}", dash);
                    lines.push(Line::from_text(
                        &pad_to(&sep, max_cols),
                        &make_attr(SURFACE0, CRUST, false),
                        termwiz::surface::SEQ_ZERO,
                        None,
                    ));
                    row_types.push(SidebarRow::Static);
                }
            }
        } else {
            lines.push(Line::from_text(
                &pad_to("  \u{25C7} SoureiGate", max_cols),
                &make_attr(LAVENDER, MANTLE, true),
                termwiz::surface::SEQ_ZERO,
                None,
            ));
            row_types.push(SidebarRow::Static);

            lines.push(Line::from_text(
                &pad_to("", max_cols),
                &make_attr(OVERLAY0, CRUST, false),
                termwiz::surface::SEQ_ZERO,
                None,
            ));
            row_types.push(SidebarRow::Static);

            lines.push(Line::from_text(
                &pad_to("  No servers connected", max_cols),
                &make_attr(OVERLAY0, CRUST, false),
                termwiz::surface::SEQ_ZERO,
                None,
            ));
            row_types.push(SidebarRow::Static);
        }

        // ── Render each line and register UIItems ──
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

            // Register UIItem for clickable rows
            if let Some(row_type) = row_types.get(i) {
                let item_type = match row_type {
                    SidebarRow::Category(idx) => Some(UIItemType::SidebarCategory(*idx)),
                    SidebarRow::Server { cat_idx, srv_idx } => {
                        Some(UIItemType::SidebarServer {
                            cat_idx: *cat_idx,
                            srv_idx: *srv_idx,
                        })
                    }
                    SidebarRow::Static => None,
                };
                if let Some(item_type) = item_type {
                    self.ui_items.push(UIItem {
                        x: sidebar_x as usize,
                        y: top_pixel_y as usize,
                        width: sidebar_width as usize,
                        height: cell_height as usize,
                        item_type,
                    });
                }
            }

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
                    foreground: LinearRgba::with_components(TEXT.0, TEXT.1, TEXT.2, 1.0),
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

// ── Helper functions ──────────────────────────────────────────────────

/// Pad or truncate string to fit sidebar width
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

/// Build CellAttributes with fg, bg, and optional bold
fn make_attr(fg: SrgbaTuple, bg: SrgbaTuple, bold: bool) -> CellAttributes {
    let mut attr = CellAttributes::default();
    attr.set_foreground(ColorAttribute::TrueColorWithDefaultFallback(fg));
    attr.set_background(ColorAttribute::TrueColorWithDefaultFallback(bg));
    if bold {
        attr.set_intensity(Intensity::Bold);
    }
    attr
}

/// Status color based on server status string
fn status_color(status: &str) -> SrgbaTuple {
    match status.to_lowercase().as_str() {
        "online" | "active" | "running" => GREEN,
        "pending" | "provisioning" | "deploying" => YELLOW,
        "offline" | "stopped" | "error" | "failed" => RED,
        _ => OVERLAY0,
    }
}

