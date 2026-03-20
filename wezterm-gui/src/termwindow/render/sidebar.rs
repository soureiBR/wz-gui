//! SoureiGate sidebar rendering
//!
//! Renders a sidebar panel on the left side of the terminal window
//! showing server categories from the SoureiGate session.

use crate::quad::TripleLayerQuadAllocator;
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
        let sidebar_h = self.dimensions.pixel_height as f32
            - sidebar_y
            - border.bottom.get() as f32;

        let cell_height = self.render_metrics.cell_size.height as f32;

        // -- Colors (Catppuccin Mocha palette) --

        // Sidebar background: Crust (#11111b)
        let bg_color = LinearRgba::with_components(
            17.0 / 255.0,
            17.0 / 255.0,
            27.0 / 255.0,
            1.0,
        );

        // Separator line: Surface1 (#45475a)
        let sep_color = LinearRgba::with_components(
            69.0 / 255.0,
            71.0 / 255.0,
            90.0 / 255.0,
            1.0,
        );

        // Category header background: Surface0 (#313244)
        let category_bg = LinearRgba::with_components(
            49.0 / 255.0,
            50.0 / 255.0,
            68.0 / 255.0,
            1.0,
        );

        // Server item accent: Blue (#89b4fa) — thin left indicator
        let accent_color = LinearRgba::with_components(
            137.0 / 255.0,
            180.0 / 255.0,
            250.0 / 255.0,
            1.0,
        );

        // -- Draw sidebar background --
        self.filled_rectangle(
            layers,
            0,
            euclid::rect(sidebar_x, sidebar_y, sidebar_width, sidebar_h),
            bg_color,
        )?;

        // -- Draw separator line (right edge, 1px) --
        let sep_x = sidebar_x + sidebar_width - 1.0;
        self.filled_rectangle(
            layers,
            1,
            euclid::rect(sep_x, sidebar_y, 1.0, sidebar_h),
            sep_color,
        )?;

        // -- Draw category/server blocks from SoureiGate session --
        let session = crate::soureigate_auth::get_session();
        if let Some(session) = session {
            let mut y_offset = sidebar_y + 8.0; // top padding

            for category in &session.categories {
                if y_offset + cell_height > sidebar_y + sidebar_h {
                    break;
                }

                // Category header bar
                self.filled_rectangle(
                    layers,
                    1,
                    euclid::rect(
                        sidebar_x + 4.0,
                        y_offset,
                        sidebar_width - 5.0,
                        cell_height,
                    ),
                    category_bg,
                )?;

                y_offset += cell_height + 2.0;

                // Server items under this category
                for _server in &category.servers {
                    if y_offset + cell_height > sidebar_y + sidebar_h {
                        break;
                    }

                    // Small accent indicator on the left
                    self.filled_rectangle(
                        layers,
                        1,
                        euclid::rect(
                            sidebar_x + 8.0,
                            y_offset + 2.0,
                            3.0,
                            cell_height - 4.0,
                        ),
                        accent_color,
                    )?;

                    y_offset += cell_height;
                }

                // Gap between categories
                y_offset += 6.0;
            }
        }

        Ok(())
    }
}
