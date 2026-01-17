use eframe::egui::{self, Color32};
use rustide_syntax::HighlightTag;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeId {
    #[default]
    Dark,
    Light,
    SolarizedDark,
    Monokai,
}

impl std::str::FromStr for ThemeId {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "dark" => Ok(Self::Dark),
            "light" => Ok(Self::Light),
            "solarized-dark" | "solarized_dark" | "solarizeddark" => Ok(Self::SolarizedDark),
            "monokai" => Ok(Self::Monokai),
            _ => Err(()),
        }
    }
}

impl std::fmt::Display for ThemeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dark => f.write_str("dark"),
            Self::Light => f.write_str("light"),
            Self::SolarizedDark => f.write_str("solarized-dark"),
            Self::Monokai => f.write_str("monokai"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SyntaxColors {
    pub comment: Color32,
    pub string: Color32,
    pub number: Color32,
    pub keyword: Color32,
    pub r#type: Color32,
    pub function: Color32,
    pub constant: Color32,
    pub variable: Color32,
    pub property: Color32,
    pub operator: Color32,
    pub punctuation: Color32,
    pub fallback: Color32,
}

impl SyntaxColors {
    pub fn for_tag(&self, tag: HighlightTag) -> Color32 {
        match tag {
            HighlightTag::Comment => self.comment,
            HighlightTag::String => self.string,
            HighlightTag::Number => self.number,
            HighlightTag::Keyword => self.keyword,
            HighlightTag::Type => self.r#type,
            HighlightTag::Function => self.function,
            HighlightTag::Constant => self.constant,
            HighlightTag::Variable => self.variable,
            HighlightTag::Property => self.property,
            HighlightTag::Operator => self.operator,
            HighlightTag::Punctuation => self.punctuation,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MinimapColors {
    pub background: Color32,
    pub border: Color32,
    pub text: Color32,
    pub viewport_fill: Color32,
    pub viewport_stroke: Color32,
    pub caret_marker: Color32,
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub visuals: egui::Visuals,
    pub syntax: SyntaxColors,
    pub minimap: MinimapColors,
}

pub fn build_theme(id: ThemeId) -> Theme {
    match id {
        ThemeId::Dark => Theme {
            visuals: egui::Visuals::dark(),
            syntax: SyntaxColors {
                comment: Color32::from_rgb(106, 153, 85),
                string: Color32::from_rgb(206, 145, 120),
                number: Color32::from_rgb(181, 206, 168),
                keyword: Color32::from_rgb(197, 134, 192),
                r#type: Color32::from_rgb(78, 201, 176),
                function: Color32::from_rgb(220, 220, 170),
                constant: Color32::from_rgb(156, 220, 254),
                variable: Color32::from_rgb(156, 220, 254),
                property: Color32::from_rgb(156, 220, 254),
                operator: Color32::from_rgb(212, 212, 212),
                punctuation: Color32::from_rgb(212, 212, 212),
                fallback: Color32::from_rgb(212, 212, 212),
            },
            minimap: MinimapColors {
                background: Color32::from_rgba_unmultiplied(30, 30, 30, 160),
                border: Color32::from_rgba_unmultiplied(60, 60, 60, 140),
                text: Color32::from_rgba_unmultiplied(220, 220, 220, 90),
                viewport_fill: Color32::from_rgba_unmultiplied(255, 255, 255, 24),
                viewport_stroke: Color32::from_rgba_unmultiplied(255, 255, 255, 80),
                caret_marker: Color32::from_rgb(0, 122, 204),
            },
        },
        ThemeId::Light => Theme {
            visuals: egui::Visuals::light(),
            syntax: SyntaxColors {
                comment: Color32::from_rgb(0, 128, 0),
                string: Color32::from_rgb(163, 21, 21),
                number: Color32::from_rgb(9, 134, 88),
                keyword: Color32::from_rgb(0, 0, 255),
                r#type: Color32::from_rgb(43, 145, 175),
                function: Color32::from_rgb(121, 94, 38),
                constant: Color32::from_rgb(0, 0, 255),
                variable: Color32::from_rgb(0, 0, 0),
                property: Color32::from_rgb(0, 0, 0),
                operator: Color32::from_rgb(0, 0, 0),
                punctuation: Color32::from_rgb(0, 0, 0),
                fallback: Color32::from_rgb(0, 0, 0),
            },
            minimap: MinimapColors {
                background: Color32::from_rgba_unmultiplied(245, 245, 245, 180),
                border: Color32::from_rgba_unmultiplied(210, 210, 210, 160),
                text: Color32::from_rgba_unmultiplied(0, 0, 0, 90),
                viewport_fill: Color32::from_rgba_unmultiplied(0, 0, 0, 18),
                viewport_stroke: Color32::from_rgba_unmultiplied(0, 0, 0, 60),
                caret_marker: Color32::from_rgb(0, 122, 204),
            },
        },
        ThemeId::SolarizedDark => Theme {
            visuals: {
                let mut v = egui::Visuals::dark();
                v.panel_fill = Color32::from_rgb(0, 43, 54);
                v.window_fill = Color32::from_rgb(0, 43, 54);
                v.extreme_bg_color = Color32::from_rgb(7, 54, 66);
                v.faint_bg_color = Color32::from_rgb(7, 54, 66);
                v
            },
            syntax: SyntaxColors {
                comment: Color32::from_rgb(88, 110, 117),
                string: Color32::from_rgb(42, 161, 152),
                number: Color32::from_rgb(211, 54, 130),
                keyword: Color32::from_rgb(203, 75, 22),
                r#type: Color32::from_rgb(38, 139, 210),
                function: Color32::from_rgb(181, 137, 0),
                constant: Color32::from_rgb(108, 113, 196),
                variable: Color32::from_rgb(131, 148, 150),
                property: Color32::from_rgb(131, 148, 150),
                operator: Color32::from_rgb(131, 148, 150),
                punctuation: Color32::from_rgb(131, 148, 150),
                fallback: Color32::from_rgb(131, 148, 150),
            },
            minimap: MinimapColors {
                background: Color32::from_rgba_unmultiplied(0, 43, 54, 160),
                border: Color32::from_rgba_unmultiplied(7, 54, 66, 140),
                text: Color32::from_rgba_unmultiplied(238, 232, 213, 90),
                viewport_fill: Color32::from_rgba_unmultiplied(238, 232, 213, 22),
                viewport_stroke: Color32::from_rgba_unmultiplied(238, 232, 213, 70),
                caret_marker: Color32::from_rgb(38, 139, 210),
            },
        },
        ThemeId::Monokai => Theme {
            visuals: {
                let mut v = egui::Visuals::dark();
                v.panel_fill = Color32::from_rgb(39, 40, 34);
                v.window_fill = Color32::from_rgb(39, 40, 34);
                v.extreme_bg_color = Color32::from_rgb(27, 28, 23);
                v.faint_bg_color = Color32::from_rgb(49, 50, 44);
                v
            },
            syntax: SyntaxColors {
                comment: Color32::from_rgb(117, 113, 94),
                string: Color32::from_rgb(230, 219, 116),
                number: Color32::from_rgb(174, 129, 255),
                keyword: Color32::from_rgb(249, 38, 114),
                r#type: Color32::from_rgb(102, 217, 239),
                function: Color32::from_rgb(166, 226, 46),
                constant: Color32::from_rgb(174, 129, 255),
                variable: Color32::from_rgb(248, 248, 242),
                property: Color32::from_rgb(248, 248, 242),
                operator: Color32::from_rgb(248, 248, 242),
                punctuation: Color32::from_rgb(248, 248, 242),
                fallback: Color32::from_rgb(248, 248, 242),
            },
            minimap: MinimapColors {
                background: Color32::from_rgba_unmultiplied(39, 40, 34, 160),
                border: Color32::from_rgba_unmultiplied(27, 28, 23, 140),
                text: Color32::from_rgba_unmultiplied(248, 248, 242, 90),
                viewport_fill: Color32::from_rgba_unmultiplied(255, 255, 255, 18),
                viewport_stroke: Color32::from_rgba_unmultiplied(255, 255, 255, 70),
                caret_marker: Color32::from_rgb(249, 38, 114),
            },
        },
    }
}

pub fn apply_theme(ctx: &egui::Context, theme: &Theme) {
    // Keep theme application small and explicit: visuals + selection tweaks.
    ctx.set_visuals(theme.visuals.clone());
}
