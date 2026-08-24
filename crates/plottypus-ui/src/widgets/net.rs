use plottypus_core::{History, Scale, bits_per_sec};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::chrome::{Axis, Graph, GraphInk, panel_block, render_scaled_graph};
use crate::layout::Panel;
use crate::theme::Theme;
use crate::widgets::AppView;

pub fn render(frame: &mut Frame, area: Rect, view: &AppView<'_>, theme: &Theme) {
    let block = panel_block(
        Panel::Net,
        title(view, theme),
        view.is_focused(Panel::Net),
        view.is_expanded(Panel::Net),
        theme,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let split_tx = inner.height >= 4;
    if split_tx {
        let rows = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).split(inner);
        render_net_graph(frame, rows[0], view.net_rx_history, theme);
        let tx = Layout::horizontal([Constraint::Length(3), Constraint::Fill(1)]).split(rows[1]);
        frame.render_widget(Paragraph::new(Span::styled(" ↑ ", theme.dim())), tx[0]);
        render_net_graph(frame, tx[1], view.net_tx_history, theme);
    } else {
        render_net_graph(frame, inner, view.net_rx_history, theme);
    }
}

fn render_net_graph(frame: &mut Frame, area: Rect, history: &History, theme: &Theme) {
    render_scaled_graph(
        frame,
        area,
        Graph {
            history,
            accent: theme.net,
            theme,
            scale: Scale::Auto { floor: 8_000.0 },
            axis: Axis::Bits,
            ink: GraphInk::Flat,
        },
    );
}

fn title(view: &AppView<'_>, theme: &Theme) -> Line<'static> {
    let net = &view.snapshot.network;
    let iface = if net.iface.is_empty() {
        "—"
    } else {
        net.iface.as_str()
    };
    Line::from(vec![
        Span::styled(" net  ", theme.dim()),
        Span::styled(format!("{iface}  "), theme.dim()),
        Span::styled(format!("↓{}", bits_per_sec(net.rx_bps)), theme.title()),
        Span::raw(" "),
        Span::styled(format!("↑{}", bits_per_sec(net.tx_bps)), theme.title()),
        Span::raw(" "),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::tests_support::fixture;

    #[test]
    fn title_shows_iface_and_rates() {
        let mut fx = fixture("");
        fx.snap.network.iface = String::from("en0");
        fx.snap.network.rx_bps = 12_400_000;
        fx.snap.network.tx_bps = 800;
        let text: String = title(&fx.view(), &Theme::default())
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(text.contains("net"));
        assert!(text.contains("en0"));
        assert!(text.contains("12.4Mb"));
        assert!(text.contains("800b"));
    }
}
