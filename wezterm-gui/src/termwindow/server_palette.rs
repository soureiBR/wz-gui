use crate::overlay::selector::{matcher_pattern, matcher_score};
use crate::termwindow::box_model::*;
use crate::termwindow::modal::Modal;
use crate::termwindow::render::corners::{
    BOTTOM_LEFT_ROUNDED_CORNER, BOTTOM_RIGHT_ROUNDED_CORNER, TOP_LEFT_ROUNDED_CORNER,
    TOP_RIGHT_ROUNDED_CORNER,
};
use crate::termwindow::DimensionContext;
use crate::utilsprites::RenderMetrics;
use crate::TermWindow;
use config::keyassignment::SpawnTabDomain;
use config::Dimension;
use std::cell::{Ref, RefCell};
use wezterm_term::{KeyCode, KeyModifiers, MouseEvent};
use window::color::LinearRgba;

struct ServerEntry {
    name: String,
    category: String,
    domain_name: String,
    status: String,
}

struct MatchResults {
    query: String,
    matches: Vec<usize>,
}

pub struct ServerPalette {
    servers: Vec<ServerEntry>,
    element: RefCell<Option<Vec<ComputedElement>>>,
    query: RefCell<String>,
    matches: RefCell<Option<MatchResults>>,
    selected_row: RefCell<usize>,
    top_row: RefCell<usize>,
    max_rows_on_screen: RefCell<usize>,
}

impl ServerPalette {
    pub fn new() -> Self {
        let mut servers = Vec::new();
        if let Some(session) = crate::soureigate_auth::get_session() {
            for cat in &session.categories {
                for srv in &cat.servers {
                    servers.push(ServerEntry {
                        name: srv.name.clone(),
                        category: cat.name.clone(),
                        domain_name: format!("sg:{}", srv.name),
                        status: srv.status.clone(),
                    });
                }
            }
        }

        Self {
            servers,
            element: RefCell::new(None),
            query: RefCell::new(String::new()),
            matches: RefCell::new(None),
            selected_row: RefCell::new(0),
            top_row: RefCell::new(0),
            max_rows_on_screen: RefCell::new(0),
        }
    }

    fn compute_matches(query: &str, servers: &[ServerEntry]) -> Vec<usize> {
        if query.is_empty() {
            (0..servers.len()).collect()
        } else {
            let pattern = matcher_pattern(query);
            let mut scored: Vec<(usize, u32)> = servers
                .iter()
                .enumerate()
                .filter_map(|(idx, srv)| {
                    let search_text = format!("{} {}", srv.name, srv.category);
                    matcher_score(&pattern, &search_text).map(|score| (idx, score))
                })
                .collect();
            scored.sort_by(|a, b| b.1.cmp(&a.1));
            scored.into_iter().map(|(idx, _)| idx).collect()
        }
    }

    fn compute(
        term_window: &mut TermWindow,
        query: &str,
        servers: &[ServerEntry],
        matches: &MatchResults,
        max_rows_on_screen: usize,
        selected_row: usize,
        top_row: usize,
    ) -> anyhow::Result<Vec<ComputedElement>> {
        let font = term_window
            .fonts
            .char_select_font()
            .expect("to resolve char selection font");
        let metrics = RenderMetrics::with_font_metrics(&font.metrics());

        let top_bar_height = if term_window.show_tab_bar && !term_window.config.tab_bar_at_bottom {
            term_window.tab_bar_pixel_height().unwrap()
        } else {
            0.
        };
        let (padding_left, padding_top) = term_window.padding_left_top();
        let border = term_window.get_os_border();
        let top_pixel_y = top_bar_height + padding_top + border.top.get() as f32;

        // Colors — Catppuccin Mocha
        let bg = LinearRgba::with_components(30.0 / 255.0, 30.0 / 255.0, 46.0 / 255.0, 1.0);
        let fg = LinearRgba::with_components(205.0 / 255.0, 214.0 / 255.0, 244.0 / 255.0, 1.0);
        let selected_bg =
            LinearRgba::with_components(137.0 / 255.0, 180.0 / 255.0, 250.0 / 255.0, 1.0);
        let selected_fg =
            LinearRgba::with_components(30.0 / 255.0, 30.0 / 255.0, 46.0 / 255.0, 1.0);
        let dim_fg =
            LinearRgba::with_components(127.0 / 255.0, 132.0 / 255.0, 156.0 / 255.0, 1.0);
        let border_color =
            LinearRgba::with_components(69.0 / 255.0, 71.0 / 255.0, 90.0 / 255.0, 1.0);

        let prompt = if query.is_empty() {
            "Search servers...".to_string()
        } else {
            format!("{}_", query)
        };

        let mut elements = vec![Element::new(
            &font,
            ElementContent::Text(format!("\u{1F50D} {}", prompt)),
        )
        .colors(ElementColors {
            border: BorderColor::default(),
            bg: LinearRgba::TRANSPARENT.into(),
            text: if query.is_empty() { dim_fg.into() } else { fg.into() },
        })
        .padding(BoxDimension {
            left: Dimension::Cells(0.5),
            right: Dimension::Cells(0.5),
            top: Dimension::Cells(0.25),
            bottom: Dimension::Cells(0.25),
        })
        .display(DisplayType::Block)];

        // Separator
        elements.push(
            Element::new(&font, ElementContent::Text("\u{2500}".repeat(40)))
                .colors(ElementColors {
                    border: BorderColor::default(),
                    bg: LinearRgba::TRANSPARENT.into(),
                    text: border_color.into(),
                })
                .padding(BoxDimension {
                    left: Dimension::Cells(0.5),
                    right: Dimension::Cells(0.5),
                    top: Dimension::Cells(0.),
                    bottom: Dimension::Cells(0.),
                })
                .display(DisplayType::Block),
        );

        if matches.matches.is_empty() {
            elements.push(
                Element::new(&font, ElementContent::Text("No servers found".to_string()))
                    .colors(ElementColors {
                        border: BorderColor::default(),
                        bg: LinearRgba::TRANSPARENT.into(),
                        text: dim_fg.into(),
                    })
                    .padding(BoxDimension {
                        left: Dimension::Cells(0.5),
                        right: Dimension::Cells(0.5),
                        top: Dimension::Cells(0.25),
                        bottom: Dimension::Cells(0.25),
                    })
                    .display(DisplayType::Block),
            );
        } else {
            for (display_idx, srv) in matches
                .matches
                .iter()
                .map(|&idx| &servers[idx])
                .enumerate()
                .skip(top_row)
                .take(max_rows_on_screen)
            {
                let (row_bg, row_fg): (InheritableColor, InheritableColor) =
                    if display_idx == selected_row {
                        (selected_bg.into(), selected_fg.into())
                    } else {
                        (LinearRgba::TRANSPARENT.into(), fg.into())
                    };

                let status_icon = if srv.status == "online" {
                    "\u{25CF} " // ●
                } else {
                    "\u{25CB} " // ○
                };

                elements.push(
                    Element::new(
                        &font,
                        ElementContent::Text(format!(
                            "{}{} \u{2502} {}",
                            status_icon, srv.name, srv.category
                        )),
                    )
                    .colors(ElementColors {
                        border: BorderColor::default(),
                        bg: row_bg,
                        text: row_fg,
                    })
                    .padding(BoxDimension {
                        left: Dimension::Cells(0.5),
                        right: Dimension::Cells(0.5),
                        top: Dimension::Cells(0.),
                        bottom: Dimension::Cells(0.),
                    })
                    .display(DisplayType::Block),
                );
            }
        }

        // Footer
        elements.push(
            Element::new(
                &font,
                ElementContent::Text(format!(
                    "{} servers | Enter: connect | Esc: close",
                    matches.matches.len()
                )),
            )
            .colors(ElementColors {
                border: BorderColor::default(),
                bg: LinearRgba::TRANSPARENT.into(),
                text: dim_fg.into(),
            })
            .padding(BoxDimension {
                left: Dimension::Cells(0.5),
                right: Dimension::Cells(0.5),
                top: Dimension::Cells(0.25),
                bottom: Dimension::Cells(0.),
            })
            .display(DisplayType::Block),
        );

        let element = Element::new(&font, ElementContent::Children(elements))
            .colors(ElementColors {
                border: BorderColor::new(border_color.into()),
                bg: bg.into(),
                text: fg.into(),
            })
            .margin(BoxDimension {
                left: Dimension::Cells(3.0),
                right: Dimension::Cells(3.0),
                top: Dimension::Cells(2.0),
                bottom: Dimension::Cells(2.0),
            })
            .padding(BoxDimension {
                left: Dimension::Cells(0.5),
                right: Dimension::Cells(0.5),
                top: Dimension::Cells(0.5),
                bottom: Dimension::Cells(0.5),
            })
            .border(BoxDimension::new(Dimension::Pixels(1.)))
            .border_corners(Some(Corners {
                top_left: SizedPoly {
                    width: Dimension::Cells(0.25),
                    height: Dimension::Cells(0.25),
                    poly: TOP_LEFT_ROUNDED_CORNER,
                },
                top_right: SizedPoly {
                    width: Dimension::Cells(0.25),
                    height: Dimension::Cells(0.25),
                    poly: TOP_RIGHT_ROUNDED_CORNER,
                },
                bottom_left: SizedPoly {
                    width: Dimension::Cells(0.25),
                    height: Dimension::Cells(0.25),
                    poly: BOTTOM_LEFT_ROUNDED_CORNER,
                },
                bottom_right: SizedPoly {
                    width: Dimension::Cells(0.25),
                    height: Dimension::Cells(0.25),
                    poly: BOTTOM_RIGHT_ROUNDED_CORNER,
                },
            }));

        let dimensions = term_window.dimensions;
        let size = term_window.terminal_size;

        let computed = term_window.compute_element(
            &LayoutContext {
                height: DimensionContext {
                    dpi: dimensions.dpi as f32,
                    pixel_max: dimensions.pixel_height as f32,
                    pixel_cell: metrics.cell_size.height as f32,
                },
                width: DimensionContext {
                    dpi: dimensions.dpi as f32,
                    pixel_max: dimensions.pixel_width as f32,
                    pixel_cell: metrics.cell_size.width as f32,
                },
                bounds: euclid::rect(
                    padding_left,
                    top_pixel_y,
                    size.cols as f32 * term_window.render_metrics.cell_size.width as f32,
                    size.rows as f32 * term_window.render_metrics.cell_size.height as f32,
                ),
                metrics: &metrics,
                gl_state: term_window.render_state.as_ref().unwrap(),
                zindex: 100,
            },
            &element,
        )?;

        Ok(vec![computed])
    }

    fn updated_input(&self) {
        *self.selected_row.borrow_mut() = 0;
        *self.top_row.borrow_mut() = 0;
    }

    fn nav_selection(&self) {
        let max_rows_on_screen = *self.max_rows_on_screen.borrow();
        let limit = self
            .matches
            .borrow()
            .as_ref()
            .map(|m| m.matches.len())
            .unwrap_or_else(|| self.servers.len());
        let mut row = self.selected_row.borrow_mut();
        let mut top_row = self.top_row.borrow_mut();
        *row = (*row).min(limit.saturating_sub(1));
        if *row < *top_row {
            *top_row = *row;
        }
        if *row + *top_row > max_rows_on_screen / 2 {
            *top_row = row.saturating_sub(max_rows_on_screen / 2);
        }
    }
}

impl Modal for ServerPalette {
    fn mouse_event(
        &self,
        _event: MouseEvent,
        _term_window: &mut TermWindow,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn key_down(
        &self,
        key: KeyCode,
        mods: KeyModifiers,
        term_window: &mut TermWindow,
    ) -> anyhow::Result<bool> {
        match (key, mods) {
            (KeyCode::Escape, KeyModifiers::NONE) | (KeyCode::Char('g'), KeyModifiers::CTRL) => {
                term_window.cancel_modal();
            }
            (KeyCode::UpArrow, KeyModifiers::NONE) | (KeyCode::Char('p'), KeyModifiers::CTRL) => {
                let current = *self.selected_row.borrow();
                *self.selected_row.borrow_mut() = current.saturating_sub(1);
                self.nav_selection();
            }
            (KeyCode::DownArrow, KeyModifiers::NONE)
            | (KeyCode::Char('n'), KeyModifiers::CTRL) => {
                let current = *self.selected_row.borrow();
                *self.selected_row.borrow_mut() = current.saturating_add(1);
                self.nav_selection();
            }
            (KeyCode::PageUp, KeyModifiers::NONE) => {
                let page = *self.max_rows_on_screen.borrow();
                let current = *self.selected_row.borrow();
                *self.selected_row.borrow_mut() = current.saturating_sub(page);
                self.nav_selection();
            }
            (KeyCode::PageDown, KeyModifiers::NONE) => {
                let page = *self.max_rows_on_screen.borrow();
                let current = *self.selected_row.borrow();
                *self.selected_row.borrow_mut() = current.saturating_add(page);
                self.nav_selection();
            }
            (KeyCode::Char(c), KeyModifiers::NONE) | (KeyCode::Char(c), KeyModifiers::SHIFT) => {
                self.query.borrow_mut().push(c);
                self.updated_input();
            }
            (KeyCode::Backspace, KeyModifiers::NONE) => {
                self.query.borrow_mut().pop();
                self.updated_input();
            }
            (KeyCode::Char('u'), KeyModifiers::CTRL) => {
                self.query.borrow_mut().clear();
                self.updated_input();
            }
            (KeyCode::Enter, KeyModifiers::NONE) => {
                let selected_idx = *self.selected_row.borrow();
                let server_idx = match self.matches.borrow().as_ref() {
                    None => return Ok(true),
                    Some(results) => match results.matches.get(selected_idx) {
                        Some(i) => *i,
                        None => return Ok(true),
                    },
                };
                let domain_name = self.servers[server_idx].domain_name.clone();
                term_window.spawn_tab(&SpawnTabDomain::DomainName(domain_name));
                term_window.cancel_modal();
                return Ok(true);
            }
            _ => return Ok(false),
        }
        term_window.invalidate_modal();
        Ok(true)
    }

    fn computed_element(
        &self,
        term_window: &mut TermWindow,
    ) -> anyhow::Result<Ref<'_, [ComputedElement]>> {
        let query = self.query.borrow();
        let query = query.as_str();

        let mut results = self.matches.borrow_mut();

        let font = term_window
            .fonts
            .char_select_font()
            .expect("to resolve char selection font");
        let metrics = RenderMetrics::with_font_metrics(&font.metrics());

        let max_rows_on_screen = ((term_window.dimensions.pixel_height * 6 / 10)
            / metrics.cell_size.height as usize)
            .max(3)
            - 2;
        *self.max_rows_on_screen.borrow_mut() = max_rows_on_screen;

        let rebuild = results
            .as_ref()
            .map(|m| m.query != query)
            .unwrap_or(true);
        if rebuild {
            results.replace(MatchResults {
                query: query.to_string(),
                matches: Self::compute_matches(query, &self.servers),
            });
        }
        let matches = results.as_ref().unwrap();

        if self.element.borrow().is_none() {
            let element = Self::compute(
                term_window,
                query,
                &self.servers,
                matches,
                max_rows_on_screen,
                *self.selected_row.borrow(),
                *self.top_row.borrow(),
            )?;
            self.element.borrow_mut().replace(element);
        }
        Ok(Ref::map(self.element.borrow(), |v| {
            v.as_ref().unwrap().as_slice()
        }))
    }

    fn reconfigure(&self, _term_window: &mut TermWindow) {
        self.element.borrow_mut().take();
    }
}
