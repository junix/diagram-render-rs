/// Built-in palette identity accepted by the CLI.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ThemePreset {
    #[default]
    Light,
    Dark,
}

impl ThemePreset {
    #[must_use]
    pub fn resolve(self) -> Theme {
        match self {
            Self::Light => Theme::light(),
            Self::Dark => Theme::dark(),
        }
    }
}

/// Semantic drawing colors consumed by format-specific layout modules.
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub font_family: String,
    pub foreground: String,
    pub muted: String,
    pub line: String,
    pub surface: String,
    pub surface_alt: String,
    pub accent: String,
    pub accent_soft: String,
    pub positive: String,
    pub warning: String,
    pub danger: String,
    pub grid: String,
}

impl Theme {
    #[must_use]
    pub fn light() -> Self {
        Self {
            font_family: "Inter, ui-sans-serif, system-ui, sans-serif".to_owned(),
            foreground: "#172033".to_owned(),
            muted: "#657089".to_owned(),
            line: "#334155".to_owned(),
            surface: "#ffffff".to_owned(),
            surface_alt: "#f5f7fb".to_owned(),
            accent: "#2563eb".to_owned(),
            accent_soft: "#dbeafe".to_owned(),
            positive: "#059669".to_owned(),
            warning: "#d97706".to_owned(),
            danger: "#dc2626".to_owned(),
            grid: "#d9e0ec".to_owned(),
        }
    }

    #[must_use]
    pub fn dark() -> Self {
        Self {
            font_family: "Inter, ui-sans-serif, system-ui, sans-serif".to_owned(),
            foreground: "#e6edf7".to_owned(),
            muted: "#9aa8bd".to_owned(),
            line: "#bac7d9".to_owned(),
            surface: "#182235".to_owned(),
            surface_alt: "#101827".to_owned(),
            accent: "#60a5fa".to_owned(),
            accent_soft: "#1e3a5f".to_owned(),
            positive: "#34d399".to_owned(),
            warning: "#fbbf24".to_owned(),
            danger: "#fb7185".to_owned(),
            grid: "#344258".to_owned(),
        }
    }

    #[must_use]
    pub fn series(&self, index: usize) -> &str {
        match index % 5 {
            0 => &self.accent,
            1 => &self.positive,
            2 => &self.warning,
            3 => &self.danger,
            _ => &self.muted,
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::light()
    }
}
