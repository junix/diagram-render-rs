use diagram_theme::cli::{LegacyTheme, RendererProfile};
use diagram_theme::{FONT_SANS, Mode, Resolved, Tokens};

/// The two names this binary answered to before the shared registry existed.
///
/// They are accepted forever, never listed by `themes`, and deliberately not
/// aliased onto `azure`/`azure-dark`: their twelve values were hand-picked and
/// `preview-rs` calls [`Theme::light`] and [`Theme::dark`] by name across a Git
/// URL, so the palettes are public API rather than a spelling of a family.
pub const LEGACY: &[LegacyTheme] = &[
    LegacyTheme::new("light", Mode::Light, "the original built-in light palette"),
    LegacyTheme::new("dark", Mode::Dark, "the original built-in dark palette"),
];

/// What this renderer publishes about its own use of the 24-token contract.
///
/// Emitted verbatim by `diagram-render-rs themes --json`, which is how the
/// cross-repository check compares palette *values* and not only names.
pub const PROFILE: RendererProfile = RendererProfile {
    renderer: "diagram-render-rs",
    version: crate::VERSION,
    // Unchanged on purpose: the shared vocabulary is what `--theme` accepts,
    // not what it does when absent.
    default_theme: "light",
    // `--paper` never reaches this renderer: the page colour is `--background`,
    // a caller decision the theme has no say in. `--line` and `--group-line`
    // collapse into `--edge` because a `Scene` stroke carries one colour, and
    // `--faint`/`--accent-ink` have no field to land in.
    unmapped: &[
        "--faint",
        "--paper",
        "--line",
        "--accent-ink",
        "--group-line",
    ],
    // `draw_connectors` (renderers/cards.rs) hides each routed polyline behind
    // its own label with a `surface_alt` pill; that pill is the only knock-out
    // this renderer paints.
    knockout_role: Some("--group"),
    legacy: LEGACY,
};

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
    /// Categorical ramp, read through [`Theme::series`].
    pub series: [String; 8],
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
            // The first four repeat accent/positive/warning/danger, which is
            // what the old five-slot cycle drew and what the checked-in gallery
            // is blessed against. The last four continue the same Tailwind 600
            // ramp instead of falling back on `muted`, so slot 4 is a category
            // rather than the grey the previous cycle produced there.
            series: [
                "#2563eb", "#059669", "#d97706", "#dc2626", "#7c3aed", "#0891b2", "#db2777",
                "#65a30d",
            ]
            .map(str::to_owned),
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
            series: [
                "#60a5fa", "#34d399", "#fbbf24", "#fb7185", "#a78bfa", "#22d3ee", "#f472b6",
                "#a3e635",
            ]
            .map(str::to_owned),
        }
    }

    /// Project the shared 24-token contract onto these twelve drawing colors.
    ///
    /// `surface` and `surface_alt` take `--panel` and `--group` **opaque**. A
    /// washed surface is not an option here and the reason is on disk:
    /// `draw_connectors` (renderers/cards.rs) runs before the card loop and
    /// routes polylines underneath the cards, then knocks each one out from
    /// behind its own label with a `surface_alt` pill; card bodies rely on
    /// `surface` to occlude the connectors they sit on. Washing either lets the
    /// connectors run straight through the text, and puts `--ink` over whatever
    /// page the file is dropped on rather than over the panel the registry
    /// gates at 10:1.
    #[must_use]
    pub fn from_tokens(tokens: &Tokens) -> Self {
        Self {
            font_family: FONT_SANS.to_owned(),
            foreground: tokens.ink.hex().to_owned(),
            muted: tokens.muted.hex().to_owned(),
            line: tokens.edge.hex().to_owned(),
            surface: tokens.panel.hex().to_owned(),
            surface_alt: tokens.group.hex().to_owned(),
            accent: tokens.accent.hex().to_owned(),
            accent_soft: tokens.accent_soft.hex().to_owned(),
            positive: tokens.pos.hex().to_owned(),
            warning: tokens.warn.hex().to_owned(),
            danger: tokens.neg.hex().to_owned(),
            grid: tokens.grid.hex().to_owned(),
            series: tokens.series().map(|token| token.hex().to_owned()),
        }
    }

    /// The palette a parsed `--theme` value names.
    ///
    /// The legacy half of the table lives here rather than in the shared crate:
    /// `light` and `dark` are this binary's own vocabulary, and their values
    /// differ measurably from every registry family.
    #[must_use]
    pub fn resolved(theme: Resolved) -> Self {
        match theme {
            Resolved::Canonical(canonical) => Self::from_tokens(canonical.tokens()),
            Resolved::Legacy("dark") => Self::dark(),
            Resolved::Legacy(_) => Self::light(),
        }
    }

    #[must_use]
    pub fn series(&self, index: usize) -> &str {
        &self.series[index % self.series.len()]
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::light()
    }
}
