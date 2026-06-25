use eframe::egui::Color32;
use serde::{Deserialize, Serialize};

pub fn rgb(r: u8, g: u8, b: u8) -> Color32 { Color32::from_rgb(r, g, b) }
pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color32 { Color32::from_rgba_unmultiplied(r, g, b, a) }
pub fn tint(c: Color32, a: u8) -> Color32 { rgba(c.r(), c.g(), c.b(), a) }

// ─── Theme enum ────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub enum Theme {
    TokyoNight, Cyberpunk, Midnight, OneDark, CatppuccinMocha,
    Dracula, RosePine, Monokai, Kanagawa, Everforest,
    MaterialOcean, Oxocarbon, Ayu, Nord, Gruvbox, SolarizedDark,
    Light, GruvboxLight, CatppuccinLatte,
}

impl Theme {
    pub fn all() -> &'static [Theme] {
        use Theme::*;
        &[TokyoNight, Cyberpunk, Midnight, OneDark, CatppuccinMocha,
          Dracula, RosePine, Monokai, Kanagawa, Everforest,
          MaterialOcean, Oxocarbon, Ayu, Nord, Gruvbox, SolarizedDark,
          Light, GruvboxLight, CatppuccinLatte]
    }
    pub fn name(&self) -> &'static str {
        use Theme::*;
        match self {
            TokyoNight     => "Tokyo Night",
            Cyberpunk      => "Cyberpunk",
            Midnight       => "Midnight",
            OneDark        => "One Dark",
            CatppuccinMocha => "Catppuccin Mocha",
            Dracula        => "Dracula",
            RosePine       => "Rose Pine",
            Monokai        => "Monokai",
            Kanagawa       => "Kanagawa",
            Everforest     => "Everforest",
            MaterialOcean  => "Material Ocean",
            Oxocarbon      => "Oxocarbon",
            Ayu            => "Ayu Dark",
            Nord           => "Nord",
            Gruvbox        => "Gruvbox",
            SolarizedDark  => "Solarized Dark",
            Light          => "Light",
            GruvboxLight   => "Gruvbox Light",
            CatppuccinLatte => "Catppuccin Latte",
        }
    }
    pub fn is_light(&self) -> bool {
        matches!(self, Theme::Light | Theme::GruvboxLight | Theme::CatppuccinLatte)
    }
    pub fn accent(&self) -> Color32 {
        use Theme::*;
        match self {
            TokyoNight     => rgb(122,162,247),
            Cyberpunk      => rgb(6,182,212),
            Midnight       => rgb(192,132,252),
            OneDark        => rgb(97,175,239),
            CatppuccinMocha => rgb(203,166,247),
            Dracula        => rgb(189,147,249),
            RosePine       => rgb(196,167,231),
            Monokai        => rgb(166,226,46),
            Kanagawa       => rgb(127,180,202),
            Everforest     => rgb(131,192,146),
            MaterialOcean  => rgb(130,170,255),
            Oxocarbon      => rgb(120,190,255),
            Ayu            => rgb(255,182,109),
            Nord           => rgb(136,192,208),
            Gruvbox        => rgb(214,153,33),
            SolarizedDark  => rgb(42,161,152),
            Light          => rgb(59,130,246),
            GruvboxLight   => rgb(121,116,14),
            CatppuccinLatte => rgb(30,102,245),
        }
    }
}

// ─── Palette ───────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct Pal {
    pub bg: Color32, pub surface: Color32, pub surface2: Color32, pub hdr: Color32,
    pub accent: Color32, pub text: Color32, pub sub: Color32, pub dim: Color32,
    pub green: Color32, pub red: Color32, pub yellow: Color32, pub border: Color32,
    pub row_odd: Color32, pub row_even: Color32, pub row_sel: Color32, pub row_hov: Color32,
    pub light: bool,
}

impl Pal {
    pub fn from(t: &Theme) -> Self {
        use Theme::*;
        match t {
            TokyoNight => Self {
                bg:rgb(26,27,38), surface:rgb(36,40,59), surface2:rgb(41,46,66),
                hdr:rgb(20,21,32), accent:rgb(122,162,247),
                text:rgb(192,202,245), sub:rgb(169,177,214), dim:rgb(86,95,137),
                green:rgb(158,206,106), red:rgb(247,118,142), yellow:rgb(224,175,104),
                border:rgb(41,46,66),
                row_odd:rgba(36,40,59,255), row_even:rgba(30,34,53,255),
                row_sel:rgba(122,162,247,55), row_hov:rgba(122,162,247,18), light:false,
            },
            Cyberpunk => Self {
                bg:rgb(10,14,28), surface:rgb(16,24,48), surface2:rgb(24,36,64),
                hdr:rgb(8,12,22), accent:rgb(6,182,212),
                text:rgb(226,232,240), sub:rgb(148,163,184), dim:rgb(71,85,105),
                green:rgb(34,197,94), red:rgb(239,68,68), yellow:rgb(245,158,11),
                border:rgb(30,41,59),
                row_odd:rgba(16,24,48,255), row_even:rgba(20,30,58,255),
                row_sel:rgba(6,182,212,55), row_hov:rgba(6,182,212,18), light:false,
            },
            Midnight => Self {
                bg:rgb(5,5,16), surface:rgb(12,12,26), surface2:rgb(18,18,36),
                hdr:rgb(4,4,14), accent:rgb(192,132,252),
                text:rgb(226,232,240), sub:rgb(148,163,184), dim:rgb(60,60,90),
                green:rgb(52,211,153), red:rgb(248,113,113), yellow:rgb(251,191,36),
                border:rgb(20,20,40),
                row_odd:rgba(12,12,26,255), row_even:rgba(8,8,20,255),
                row_sel:rgba(192,132,252,55), row_hov:rgba(192,132,252,15), light:false,
            },
            OneDark => Self {
                bg:rgb(40,44,52), surface:rgb(33,37,43), surface2:rgb(50,56,66),
                hdr:rgb(26,29,35), accent:rgb(97,175,239),
                text:rgb(171,178,191), sub:rgb(130,137,151), dim:rgb(92,99,112),
                green:rgb(152,195,121), red:rgb(224,108,117), yellow:rgb(229,192,123),
                border:rgb(60,68,80),
                row_odd:rgba(33,37,43,255), row_even:rgba(40,44,52,255),
                row_sel:rgba(97,175,239,55), row_hov:rgba(97,175,239,15), light:false,
            },
            CatppuccinMocha => Self {
                bg:rgb(30,30,46), surface:rgb(24,24,37), surface2:rgb(49,50,68),
                hdr:rgb(17,17,27), accent:rgb(203,166,247),
                text:rgb(205,214,244), sub:rgb(166,173,200), dim:rgb(108,112,134),
                green:rgb(166,227,161), red:rgb(243,139,168), yellow:rgb(249,226,175),
                border:rgb(49,50,68),
                row_odd:rgba(24,24,37,255), row_even:rgba(30,30,46,255),
                row_sel:rgba(203,166,247,55), row_hov:rgba(203,166,247,15), light:false,
            },
            Dracula => Self {
                bg:rgb(40,42,54), surface:rgb(50,53,68), surface2:rgb(60,63,80),
                hdr:rgb(30,32,44), accent:rgb(189,147,249),
                text:rgb(248,248,242), sub:rgb(139,155,180), dim:rgb(80,90,110),
                green:rgb(80,250,123), red:rgb(255,85,85), yellow:rgb(241,250,140),
                border:rgb(60,63,80),
                row_odd:rgba(50,53,68,255), row_even:rgba(44,47,62,255),
                row_sel:rgba(189,147,249,55), row_hov:rgba(189,147,249,15), light:false,
            },
            RosePine => Self {
                bg:rgb(25,23,36), surface:rgb(31,29,46), surface2:rgb(64,61,82),
                hdr:rgb(18,17,26), accent:rgb(196,167,231),
                text:rgb(224,222,244), sub:rgb(144,140,170), dim:rgb(86,82,110),
                green:rgb(156,207,216), red:rgb(235,111,146), yellow:rgb(246,193,119),
                border:rgb(64,61,82),
                row_odd:rgba(31,29,46,255), row_even:rgba(25,23,36,255),
                row_sel:rgba(196,167,231,55), row_hov:rgba(196,167,231,15), light:false,
            },
            Monokai => Self {
                bg:rgb(39,40,34), surface:rgb(47,49,40), surface2:rgb(61,62,50),
                hdr:rgb(30,31,26), accent:rgb(166,226,46),
                text:rgb(248,248,242), sub:rgb(200,200,190), dim:rgb(117,113,94),
                green:rgb(166,226,46), red:rgb(249,38,114), yellow:rgb(230,219,116),
                border:rgb(73,72,62),
                row_odd:rgba(47,49,40,255), row_even:rgba(39,40,34,255),
                row_sel:rgba(166,226,46,50), row_hov:rgba(166,226,46,15), light:false,
            },
            Kanagawa => Self {
                bg:rgb(22,22,30), surface:rgb(31,31,40), surface2:rgb(42,42,58),
                hdr:rgb(15,15,24), accent:rgb(127,180,202),
                text:rgb(220,215,186), sub:rgb(150,147,127), dim:rgb(84,84,109),
                green:rgb(118,185,0), red:rgb(195,64,67), yellow:rgb(220,180,70),
                border:rgb(54,54,74),
                row_odd:rgba(31,31,40,255), row_even:rgba(22,22,30,255),
                row_sel:rgba(127,180,202,55), row_hov:rgba(127,180,202,15), light:false,
            },
            Everforest => Self {
                bg:rgb(45,53,59), surface:rgb(52,61,70), surface2:rgb(60,73,79),
                hdr:rgb(35,43,46), accent:rgb(131,192,146),
                text:rgb(211,198,170), sub:rgb(157,153,136), dim:rgb(105,103,95),
                green:rgb(131,192,146), red:rgb(230,126,128), yellow:rgb(219,188,127),
                border:rgb(74,82,90),
                row_odd:rgba(52,61,70,255), row_even:rgba(45,53,59,255),
                row_sel:rgba(131,192,146,55), row_hov:rgba(131,192,146,15), light:false,
            },
            MaterialOcean => Self {
                bg:rgb(15,17,26), surface:rgb(13,14,22), surface2:rgb(30,34,54),
                hdr:rgb(10,11,18), accent:rgb(130,170,255),
                text:rgb(198,212,254), sub:rgb(137,148,184), dim:rgb(72,82,113),
                green:rgb(195,232,141), red:rgb(255,85,114), yellow:rgb(255,203,107),
                border:rgb(30,34,54),
                row_odd:rgba(13,14,22,255), row_even:rgba(15,17,26,255),
                row_sel:rgba(130,170,255,55), row_hov:rgba(130,170,255,15), light:false,
            },
            Oxocarbon => Self {
                bg:rgb(15,15,15), surface:rgb(22,22,22), surface2:rgb(32,32,32),
                hdr:rgb(10,10,10), accent:rgb(120,190,255),
                text:rgb(244,244,244), sub:rgb(180,180,180), dim:rgb(100,100,100),
                green:rgb(66,190,101), red:rgb(255,84,80), yellow:rgb(243,196,0),
                border:rgb(45,45,45),
                row_odd:rgba(22,22,22,255), row_even:rgba(15,15,15,255),
                row_sel:rgba(120,190,255,55), row_hov:rgba(120,190,255,15), light:false,
            },
            Ayu => Self {
                bg:rgb(15,20,25), surface:rgb(20,27,33), surface2:rgb(26,34,44),
                hdr:rgb(11,15,20), accent:rgb(255,182,109),
                text:rgb(203,215,232), sub:rgb(139,155,175), dim:rgb(75,90,112),
                green:rgb(166,213,146), red:rgb(245,110,110), yellow:rgb(255,182,109),
                border:rgb(33,43,54),
                row_odd:rgba(20,27,33,255), row_even:rgba(15,20,25,255),
                row_sel:rgba(255,182,109,50), row_hov:rgba(255,182,109,15), light:false,
            },
            Nord => Self {
                bg:rgb(46,52,64), surface:rgb(59,66,82), surface2:rgb(67,76,94),
                hdr:rgb(36,42,54), accent:rgb(136,192,208),
                text:rgb(236,239,244), sub:rgb(144,153,166), dim:rgb(76,86,106),
                green:rgb(163,190,140), red:rgb(191,97,106), yellow:rgb(235,203,139),
                border:rgb(67,76,94),
                row_odd:rgba(59,66,82,255), row_even:rgba(52,60,76,255),
                row_sel:rgba(136,192,208,55), row_hov:rgba(136,192,208,15), light:false,
            },
            Gruvbox => Self {
                bg:rgb(40,40,40), surface:rgb(60,56,54), surface2:rgb(80,73,69),
                hdr:rgb(29,32,33), accent:rgb(214,153,33),
                text:rgb(235,219,178), sub:rgb(168,153,132), dim:rgb(102,92,84),
                green:rgb(184,187,38), red:rgb(251,73,52), yellow:rgb(250,189,47),
                border:rgb(80,73,69),
                row_odd:rgba(60,56,54,255), row_even:rgba(54,50,48,255),
                row_sel:rgba(214,153,33,55), row_hov:rgba(214,153,33,15), light:false,
            },
            SolarizedDark => Self {
                bg:rgb(0,43,54), surface:rgb(7,54,66), surface2:rgb(0,60,80),
                hdr:rgb(0,32,42), accent:rgb(42,161,152),
                text:rgb(131,148,150), sub:rgb(101,123,131), dim:rgb(55,83,98),
                green:rgb(133,153,0), red:rgb(220,50,47), yellow:rgb(181,137,0),
                border:rgb(0,60,80),
                row_odd:rgba(7,54,66,255), row_even:rgba(0,48,60,255),
                row_sel:rgba(42,161,152,55), row_hov:rgba(42,161,152,15), light:false,
            },
            Light => Self {
                bg:rgb(249,250,251), surface:rgb(243,244,246), surface2:rgb(229,231,235),
                hdr:rgb(255,255,255), accent:rgb(59,130,246),
                text:rgb(17,24,39), sub:rgb(75,85,99), dim:rgb(156,163,175),
                green:rgb(22,163,74), red:rgb(220,38,38), yellow:rgb(217,119,6),
                border:rgb(209,213,219),
                row_odd:rgba(255,255,255,255), row_even:rgba(249,250,251,255),
                row_sel:rgba(59,130,246,40), row_hov:rgba(59,130,246,12), light:true,
            },
            GruvboxLight => Self {
                bg:rgb(251,241,199), surface:rgb(242,229,188), surface2:rgb(213,196,161),
                hdr:rgb(255,248,212), accent:rgb(121,116,14),
                text:rgb(60,56,54), sub:rgb(102,92,84), dim:rgb(168,153,132),
                green:rgb(121,116,14), red:rgb(157,0,6), yellow:rgb(181,118,20),
                border:rgb(213,196,161),
                row_odd:rgba(255,248,212,255), row_even:rgba(251,241,199,255),
                row_sel:rgba(121,116,14,45), row_hov:rgba(121,116,14,12), light:true,
            },
            CatppuccinLatte => Self {
                bg:rgb(239,241,245), surface:rgb(230,233,239), surface2:rgb(204,208,218),
                hdr:rgb(255,255,255), accent:rgb(30,102,245),
                text:rgb(76,79,105), sub:rgb(100,104,132), dim:rgb(156,160,176),
                green:rgb(64,160,43), red:rgb(210,15,57), yellow:rgb(223,142,29),
                border:rgb(188,192,204),
                row_odd:rgba(255,255,255,255), row_even:rgba(239,241,245,255),
                row_sel:rgba(30,102,245,40), row_hov:rgba(30,102,245,10), light:true,
            },
        }
    }

    pub fn apply_to_ctx(&self, ctx: &eframe::egui::Context) {
        use eframe::egui::{Visuals, Stroke, Rounding};
        let p = self;
        let mut vis = if p.light { Visuals::light() } else { Visuals::dark() };
        vis.panel_fill            = p.bg;
        vis.window_fill           = p.bg;
        vis.faint_bg_color        = p.surface;
        vis.extreme_bg_color      = p.hdr;
        vis.widgets.noninteractive.bg_fill     = p.surface;
        vis.widgets.inactive.bg_fill           = p.surface;
        vis.widgets.hovered.bg_fill            = p.surface2;
        vis.widgets.active.bg_fill             = p.accent;
        vis.selection.bg_fill                  = tint(p.accent, 50);
        vis.override_text_color                = Some(p.text);
        vis.widgets.noninteractive.fg_stroke   = Stroke::new(1.0, p.dim);
        vis.widgets.inactive.fg_stroke         = Stroke::new(1.0, p.sub);
        vis.widgets.noninteractive.bg_stroke   = Stroke::new(1.0, p.border);
        vis.widgets.inactive.bg_stroke         = Stroke::new(1.0, p.border);
        let rn = Rounding::same(6.0);
        vis.widgets.noninteractive.rounding = rn;
        vis.widgets.inactive.rounding        = rn;
        vis.widgets.hovered.rounding         = rn;
        vis.widgets.active.rounding          = rn;
        ctx.set_visuals(vis);
    }
}
