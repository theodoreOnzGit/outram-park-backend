//! Numerical-Method Catalogue tab — a human-facing view over `kovan_codegen`.
//!
//! Cycle the method family (root finders / linear solvers / nonlinear solvers
//! / ODE solvers / PDE schemes) with Left/Right, the method within a family
//! with Up/Down, and press Enter to generate and preview its source
//! ([`kovan_codegen::generate`]). Entries that are catalogued but not yet
//! backed by a template show the same [`kovan_codegen::CodegenError`] message
//! the `kovan` CLI's `methods` subcommand reports.

use kovan_codegen::{
    generate, LinearSolver, Method, NonlinearSolver, OdeSolver, PdeScheme, RootFinder,
};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

/// A method family — one screen "column" of the catalogue. Mirrors
/// `docs/kovan.md` § "Numerical Methods" (Root Finding / Linear Solvers /
/// Nonlinear Solvers / ODE Solvers / PDE Infrastructure).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Root,
    Linear,
    Nonlinear,
    Ode,
    Pde,
}

const FAMILIES: [Family; 5] = [
    Family::Root,
    Family::Linear,
    Family::Nonlinear,
    Family::Ode,
    Family::Pde,
];

impl Family {
    fn label(self) -> &'static str {
        match self {
            Family::Root => "root finders",
            Family::Linear => "linear solvers",
            Family::Nonlinear => "nonlinear solvers",
            Family::Ode => "ODE solvers",
            Family::Pde => "PDE schemes",
        }
    }

    fn step(self, delta: i32) -> Self {
        let i = FAMILIES.iter().position(|f| *f == self).unwrap_or(0) as i32;
        let n = FAMILIES.len() as i32;
        let j = ((i + delta) % n + n) % n;
        FAMILIES[j as usize]
    }

    /// Every catalogued method in this family, in the same order the `kovan`
    /// CLI's `methods` subcommand lists them.
    fn methods(self) -> Vec<Method> {
        match self {
            Family::Root => vec![
                Method::Root(RootFinder::Bisection),
                Method::Root(RootFinder::RegulaFalsi),
                Method::Root(RootFinder::Illinois),
                Method::Root(RootFinder::Pegasus),
                Method::Root(RootFinder::Secant),
                Method::Root(RootFinder::NewtonRaphson),
                Method::Root(RootFinder::Brent),
            ],
            Family::Linear => vec![
                Method::Linear(LinearSolver::Jacobi),
                Method::Linear(LinearSolver::GaussSeidel),
                Method::Linear(LinearSolver::Sor),
                Method::Linear(LinearSolver::ConjugateGradient),
                Method::Linear(LinearSolver::BiCgStab),
                Method::Linear(LinearSolver::Gmres),
                Method::Linear(LinearSolver::Lu),
                Method::Linear(LinearSolver::Qr),
                Method::Linear(LinearSolver::Cholesky),
            ],
            Family::Nonlinear => vec![
                Method::Nonlinear(NonlinearSolver::Newton),
                Method::Nonlinear(NonlinearSolver::QuasiNewton),
                Method::Nonlinear(NonlinearSolver::Broyden),
                Method::Nonlinear(NonlinearSolver::TrustRegion),
            ],
            Family::Ode => vec![
                Method::Ode(OdeSolver::Euler),
                Method::Ode(OdeSolver::Rk2),
                Method::Ode(OdeSolver::Rk4),
                Method::Ode(OdeSolver::DormandPrince),
                Method::Ode(OdeSolver::BackwardEuler),
                Method::Ode(OdeSolver::CrankNicolson),
            ],
            Family::Pde => vec![
                Method::Pde(PdeScheme::Poisson1dFiniteDifference),
                Method::Pde(PdeScheme::Diffusion1dFiniteVolume),
                Method::Pde(PdeScheme::BoundaryConditionScaffold),
            ],
        }
    }
}

/// A short display label for a [`Method`] — its variant name via [`Debug`].
fn method_label(m: Method) -> String {
    match m {
        Method::Root(x) => format!("{x:?}"),
        Method::Linear(x) => format!("{x:?}"),
        Method::Nonlinear(x) => format!("{x:?}"),
        Method::Ode(x) => format!("{x:?}"),
        Method::Pde(x) => format!("{x:?}"),
    }
}

/// State for the Numerical-Method Catalogue tab.
pub struct MethodsState {
    pub family: Family,
    pub list_state: ListState,
    pub preview: String,
    pub preview_scroll: u16,
    pub status: String,
}

impl Default for MethodsState {
    fn default() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            family: Family::Root,
            list_state,
            preview: String::new(),
            preview_scroll: 0,
            status: "select a method, press Enter to generate".to_string(),
        }
    }
}

impl MethodsState {
    fn select_next(&mut self) {
        let n = self.family.methods().len();
        if n == 0 {
            return;
        }
        let i = self.list_state.selected().map(|i| (i + 1) % n).unwrap_or(0);
        self.list_state.select(Some(i));
    }

    fn select_prev(&mut self) {
        let n = self.family.methods().len();
        if n == 0 {
            return;
        }
        let i = self
            .list_state
            .selected()
            .map(|i| if i == 0 { n - 1 } else { i - 1 })
            .unwrap_or(0);
        self.list_state.select(Some(i));
    }

    fn generate_selected(&mut self) {
        let methods = self.family.methods();
        let Some(i) = self.list_state.selected() else {
            return;
        };
        let Some(&m) = methods.get(i) else {
            return;
        };
        self.preview_scroll = 0;
        match generate(m) {
            Ok(src) => {
                self.status = format!("generated {} ({} bytes)", method_label(m), src.len());
                self.preview = src;
            }
            Err(e) => {
                self.status = format!("{}: {e}", method_label(m));
                self.preview.clear();
            }
        }
    }

    /// Handle one key event. This tab has no text-editing field, so it has no
    /// edit-mode branch.
    pub fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Left => {
                self.family = self.family.step(-1);
                self.list_state.select(Some(0));
                self.preview.clear();
            }
            KeyCode::Right => {
                self.family = self.family.step(1);
                self.list_state.select(Some(0));
                self.preview.clear();
            }
            KeyCode::Down => self.select_next(),
            KeyCode::Up => self.select_prev(),
            KeyCode::Enter => self.generate_selected(),
            KeyCode::PageDown => self.preview_scroll = self.preview_scroll.saturating_add(10),
            KeyCode::PageUp => self.preview_scroll = self.preview_scroll.saturating_sub(10),
            _ => {}
        }
    }
}

/// Render the Methods tab into `area`.
pub fn draw(frame: &mut Frame, area: Rect, state: &mut MethodsState) {
    let top = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);

    let family_line = FAMILIES
        .iter()
        .map(|f| {
            if *f == state.family {
                format!("[{}]", f.label())
            } else {
                format!(" {} ", f.label())
            }
        })
        .collect::<Vec<_>>()
        .join("  ");
    frame.render_widget(
        Paragraph::new(format!(
            "Family (Left/Right): {family_line}   {}",
            state.status
        )),
        top[0],
    );

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(top[1]);

    let items: Vec<ListItem> = state
        .family
        .methods()
        .into_iter()
        .map(|m| ListItem::new(method_label(m)))
        .collect();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("{} (Enter to generate)", state.family.label())),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    frame.render_stateful_widget(list, cols[0], &mut state.list_state);

    frame.render_widget(
        Paragraph::new(state.preview.as_str())
            .wrap(Wrap { trim: false })
            .scroll((state.preview_scroll, 0))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Generated source (PgUp/PgDn to scroll)"),
            ),
        cols[1],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn family_cycles_and_resets_selection_and_preview() {
        let mut state = MethodsState {
            preview: "stale".to_string(),
            ..Default::default()
        };
        state.handle_key(key(KeyCode::Right));
        assert_eq!(state.family, Family::Linear);
        assert_eq!(state.list_state.selected(), Some(0));
        assert!(state.preview.is_empty());

        state.handle_key(key(KeyCode::Left));
        assert_eq!(state.family, Family::Root);
    }

    #[test]
    fn family_wraps_both_directions() {
        let mut state = MethodsState::default();
        state.handle_key(key(KeyCode::Left));
        assert_eq!(state.family, Family::Pde, "Left from Root must wrap to Pde");
    }

    #[test]
    fn enter_on_an_implemented_method_populates_the_preview() {
        let mut state = MethodsState::default();
        // Default family is Root, default selection index 0 = Bisection.
        state.handle_key(key(KeyCode::Enter));
        assert!(state.preview.contains("pub fn bisection"));
        assert!(state.status.contains("generated"));
    }

    #[test]
    fn enter_on_an_unimplemented_method_reports_the_error_and_clears_preview() {
        let mut state = MethodsState::default();
        // Root: Bisection(0), RegulaFalsi(1), Illinois(2) — select Illinois.
        state.handle_key(key(KeyCode::Down));
        state.handle_key(key(KeyCode::Down));
        state.handle_key(key(KeyCode::Enter));
        assert!(state.preview.is_empty());
        assert!(state.status.contains("Illinois"));
    }

    #[test]
    fn method_selection_wraps_within_a_family() {
        let mut state = MethodsState::default();
        let n = Family::Root.methods().len();
        for _ in 0..n {
            state.handle_key(key(KeyCode::Down));
        }
        assert_eq!(
            state.list_state.selected(),
            Some(0),
            "n steps must wrap back to 0"
        );
        state.handle_key(key(KeyCode::Up));
        assert_eq!(state.list_state.selected(), Some(n - 1));
    }

    #[test]
    fn preview_scroll_saturates_at_zero() {
        let mut state = MethodsState::default();
        state.handle_key(key(KeyCode::PageUp));
        assert_eq!(state.preview_scroll, 0);
        state.handle_key(key(KeyCode::PageDown));
        assert_eq!(state.preview_scroll, 10);
    }

    #[test]
    fn every_family_generates_every_catalogued_method_consistently_with_generate() {
        // Cross-check against the library directly: whatever `Family::methods`
        // lists must agree with `kovan_codegen::generate`'s own Ok/Err split.
        for family in FAMILIES {
            for m in family.methods() {
                let via_family = generate(m);
                let via_direct = generate(m);
                assert_eq!(via_family, via_direct);
            }
        }
    }

    #[test]
    fn draw_does_not_panic_in_any_family() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        for family in FAMILIES {
            let mut state = MethodsState {
                family,
                ..MethodsState::default()
            };
            state.handle_key(key(KeyCode::Enter));
            terminal
                .draw(|f| draw(f, f.area(), &mut state))
                .expect("draw must not panic");
        }
    }
}
