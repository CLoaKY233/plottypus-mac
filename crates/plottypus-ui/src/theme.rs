use plottypus_core::{Pressure, Thermal};
use ratatui::style::{Color, Modifier, Style};

/// One ink family. Graphs use the panel accent, faded by row — not a traffic-light LUT.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub dim: Color,
    pub title: Color,
    pub hi: Color,
    pub cpu: Color,
    pub gpu: Color,
    pub mem: Color,
    pub net: Color,
    pub disk: Color,
    pub fan: Color,
    pub temp: Color,
    pub border: Color,
    pub border_focus: Color,
    pub warn: Color,
    pub crit: Color,
    pub ok: Color,
}

impl Default for Theme {
    fn default() -> Self {
        // Greyscale chrome. Color is a scale, not a logo per box.
        Self {
            bg: Color::Rgb(0x00, 0x00, 0x00),
            fg: Color::Rgb(0xcc, 0xcc, 0xcc),
            dim: Color::Rgb(0x60, 0x60, 0x60),
            title: Color::Rgb(0xee, 0xee, 0xee),
            hi: Color::Rgb(0xa8, 0x4a, 0x42),
            cpu: Color::Rgb(0x6f, 0xbe, 0x96),
            gpu: Color::Rgb(0x6f, 0xbe, 0x96),
            mem: Color::Rgb(0xc2, 0xb6, 0x68),
            net: Color::Rgb(0xa8, 0xa2, 0xd2),
            disk: Color::Rgb(0xc2, 0xb6, 0x68),
            fan: Color::Rgb(0xc2, 0xb6, 0x68),
            temp: Color::Rgb(0x4a, 0x90, 0xc8),
            border: Color::Rgb(0x30, 0x30, 0x30),
            border_focus: Color::Rgb(0x58, 0x6e, 0x5c),
            warn: Color::Rgb(0xc2, 0xb6, 0x68),
            crit: Color::Rgb(0xd0, 0x52, 0x52),
            ok: Color::Rgb(0x6f, 0xbe, 0x96),
        }
    }
}

impl Theme {
    #[must_use]
    pub fn title(self) -> Style {
        Style::default().fg(self.title)
    }

    #[must_use]
    pub fn dim(self) -> Style {
        Style::default().fg(self.dim)
    }

    #[must_use]
    pub fn cpu(self) -> Style {
        Style::default().fg(self.cpu)
    }

    #[must_use]
    pub fn gpu(self) -> Style {
        Style::default().fg(self.gpu)
    }

    #[must_use]
    pub fn mem(self) -> Style {
        Style::default().fg(self.mem)
    }

    #[must_use]
    pub fn net(self) -> Style {
        Style::default().fg(self.net)
    }

    #[must_use]
    pub fn disk(self) -> Style {
        Style::default().fg(self.disk)
    }

    #[must_use]
    pub fn fan(self) -> Style {
        Style::default().fg(self.fan)
    }

    #[must_use]
    pub fn temp(self) -> Style {
        Style::default().fg(self.temp)
    }

    #[must_use]
    pub fn fg(self) -> Style {
        Style::default().fg(self.fg)
    }

    #[must_use]
    pub fn warn_style(self) -> Style {
        Style::default().fg(self.warn)
    }

    #[must_use]
    pub fn selected(self) -> Style {
        Style::default()
            .fg(self.title)
            .bg(Color::Rgb(0x5c, 0x2e, 0x2e))
            .add_modifier(Modifier::BOLD)
    }

    #[must_use]
    pub fn border(self, focused: bool) -> Style {
        Style::default().fg(if focused {
            self.border_focus
        } else {
            self.border
        })
    }

    #[must_use]
    pub fn pressure(self, pressure: Pressure) -> Style {
        match pressure {
            Pressure::Nominal => self.dim(),
            Pressure::Warn => Style::default().fg(self.warn),
            Pressure::Critical => Style::default().fg(self.crit),
        }
    }

    #[must_use]
    pub fn thermal(self, thermal: Thermal) -> Style {
        match thermal {
            Thermal::Nominal => self.dim(),
            Thermal::Fair => Style::default().fg(self.warn),
            Thermal::Serious | Thermal::Critical => Style::default().fg(self.crit),
        }
    }

    /// Tall graphs: top = accent, bottom = dark start. Idle stays quiet.
    #[must_use]
    pub fn series_row(self, accent: Color, row: usize, rows: usize) -> Color {
        if rows <= 1 {
            return accent;
        }
        let t = row as f32 / (rows.saturating_sub(1) as f32);
        lerp(accent, self.border, t.clamp(0.0, 1.0))
    }

    #[must_use]
    pub fn temp_color(self, celsius: f32) -> Color {
        if celsius >= 90.0 {
            self.crit
        } else if celsius >= 75.0 {
            self.warn
        } else {
            self.temp
        }
    }

    #[must_use]
    pub fn stain(self, accent: Color, intensity: f32, thermal: Thermal) -> Style {
        let t = intensity.clamp(0.0, 1.0);
        let hot = match thermal {
            Thermal::Nominal => accent,
            Thermal::Fair => self.warn,
            Thermal::Serious | Thermal::Critical => self.crit,
        };
        Style::default().fg(lerp(self.dim, hot, t))
    }

    #[must_use]
    pub fn gradient(self, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        lerp(self.cpu, self.dim, t * 0.4)
    }

    #[must_use]
    pub fn graph(self, intensity: f32, thermal: Thermal) -> Color {
        match thermal {
            Thermal::Nominal => self.gradient(intensity),
            Thermal::Fair => self.warn,
            Thermal::Serious | Thermal::Critical => self.crit,
        }
    }
}

fn lerp(a: Color, b: Color, t: f32) -> Color {
    let Color::Rgb(ar, ag, ab) = a else {
        return a;
    };
    let Color::Rgb(br, bg, bb) = b else {
        return b;
    };
    Color::Rgb(mix(ar, br, t), mix(ag, bg, t), mix(ab, bb, t))
}

fn mix(a: u8, b: u8, t: f32) -> u8 {
    let a = f32::from(a);
    let b = f32::from(b);
    (a + (b - a) * t).round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn series_row_fades_toward_dim() {
        let theme = Theme::default();
        assert_eq!(theme.series_row(theme.cpu, 0, 4), theme.cpu);
        assert_ne!(theme.series_row(theme.cpu, 3, 4), theme.cpu);
    }

    #[test]
    fn gradient_stays_in_family() {
        let theme = Theme::default();
        assert_eq!(theme.gradient(0.0), theme.cpu);
    }

    #[test]
    fn pressure_nominal_is_dim() {
        let theme = Theme::default();
        assert_eq!(theme.pressure(Pressure::Nominal), theme.dim());
        assert_eq!(theme.pressure(Pressure::Warn).fg, Some(theme.warn));
    }

    #[test]
    fn hot_temp_turns_warn() {
        let theme = Theme::default();
        assert_eq!(theme.temp_color(40.0), theme.temp);
        assert_eq!(theme.temp_color(80.0), theme.warn);
        assert_eq!(theme.temp_color(95.0), theme.crit);
    }

    #[test]
    fn stain_ramps_to_accent_when_nominal() {
        let theme = Theme::default();
        assert_eq!(
            theme.stain(theme.cpu, 1.0, Thermal::Nominal).fg,
            Some(theme.cpu)
        );
        assert_eq!(
            theme.stain(theme.cpu, 0.0, Thermal::Nominal).fg,
            Some(theme.dim)
        );
    }

    #[test]
    fn fair_thermal_stains_gold() {
        let theme = Theme::default();
        let style = theme.stain(theme.cpu, 1.0, Thermal::Fair);
        assert_eq!(style.fg, Some(theme.warn));
        let style = theme.stain(theme.cpu, 1.0, Thermal::Serious);
        assert_eq!(style.fg, Some(theme.crit));
    }
}
