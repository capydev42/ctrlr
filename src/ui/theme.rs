use ratatui::style::Color;

#[derive(Clone, Debug)]
pub enum CatppuccinFlavor {
    Latte,
    Frappe,
    Macchiato,
    Mocha,
}

impl CatppuccinFlavor {
    pub const fn all() -> &'static [CatppuccinFlavor] {
        &[
            CatppuccinFlavor::Latte,
            CatppuccinFlavor::Frappe,
            CatppuccinFlavor::Macchiato,
            CatppuccinFlavor::Mocha,
        ]
    }
}

impl std::fmt::Display for CatppuccinFlavor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CatppuccinFlavor::Latte => write!(f, "Latte"),
            CatppuccinFlavor::Frappe => write!(f, "Frappe"),
            CatppuccinFlavor::Macchiato => write!(f, "Macchiato"),
            CatppuccinFlavor::Mocha => write!(f, "Mocha"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Theme {
    pub focus_border: Color,
    pub unfocus_border: Color,
    pub tab_active_fg: Color,
    pub tab_inactive_fg: Color,
    pub tag_bg: Color,
    pub tag_fg: Color,
    pub tag_selected_bg: Color,
    pub highlight_bg: Color,
    pub highlight_fg: Color,
    pub match_highlight_fg: Color,
    pub section_fg: Color,
    pub favorite_fg: Color,
    pub hint_fg: Color,
    pub popup_border: Color,
    pub create_fg: Color,
    pub scrollbar_fg: Color,
    pub header_bg: Color,
    pub header_fg: Color,
    pub help_keys_fg: Color,
    pub help_name_fg: Color,
    pub help_desc_fg: Color,
    pub help_search_border: Color,
    pub tag_popup_border: Color,
    pub input_text: Color,
}

impl Theme {
    pub fn mocha() -> Self {
        Self {
            focus_border: Color::Rgb(203, 166, 247),
            unfocus_border: Color::DarkGray,
            tab_active_fg: Color::Rgb(203, 166, 247),
            tab_inactive_fg: Color::Rgb(166, 173, 200),
            tag_bg: Color::Rgb(49, 50, 68),
            tag_fg: Color::Rgb(180, 185, 190),
            tag_selected_bg: Color::Rgb(100, 150, 100),
            highlight_bg: Color::Rgb(137, 180, 250),
            highlight_fg: Color::Rgb(30, 30, 46),
            match_highlight_fg: Color::Rgb(249, 226, 175),
            section_fg: Color::Rgb(137, 180, 250),
            favorite_fg: Color::Rgb(249, 226, 175),
            hint_fg: Color::DarkGray,
            popup_border: Color::Rgb(203, 166, 247),
            create_fg: Color::Rgb(166, 227, 161),
            scrollbar_fg: Color::DarkGray,
            header_bg: Color::Rgb(30, 30, 46),
            header_fg: Color::Rgb(137, 180, 250),
            help_keys_fg: Color::Rgb(249, 226, 175),
            help_name_fg: Color::Rgb(205, 214, 244),
            help_desc_fg: Color::DarkGray,
            help_search_border: Color::Rgb(116, 199, 236),
            tag_popup_border: Color::Rgb(249, 226, 175),
            input_text: Color::Rgb(205, 214, 244),
        }
    }

    pub fn macchiato() -> Self {
        Self {
            focus_border: Color::Rgb(198, 160, 245),
            unfocus_border: Color::DarkGray,
            tab_active_fg: Color::Rgb(198, 160, 245),
            tab_inactive_fg: Color::Rgb(166, 173, 199),
            tag_bg: Color::Rgb(54, 58, 79),
            tag_fg: Color::Rgb(175, 180, 185),
            tag_selected_bg: Color::Rgb(95, 140, 95),
            highlight_bg: Color::Rgb(138, 173, 244),
            highlight_fg: Color::Rgb(36, 39, 58),
            match_highlight_fg: Color::Rgb(238, 212, 159),
            section_fg: Color::Rgb(138, 173, 244),
            favorite_fg: Color::Rgb(238, 212, 159),
            hint_fg: Color::DarkGray,
            popup_border: Color::Rgb(198, 160, 245),
            create_fg: Color::Rgb(166, 218, 149),
            scrollbar_fg: Color::DarkGray,
            header_bg: Color::Rgb(36, 39, 58),
            header_fg: Color::Rgb(138, 173, 244),
            help_keys_fg: Color::Rgb(238, 212, 159),
            help_name_fg: Color::Rgb(202, 211, 245),
            help_desc_fg: Color::DarkGray,
            help_search_border: Color::Rgb(113, 188, 226),
            tag_popup_border: Color::Rgb(238, 212, 159),
            input_text: Color::Rgb(202, 211, 245),
        }
    }

    pub fn frappe() -> Self {
        Self {
            focus_border: Color::Rgb(202, 158, 230),
            unfocus_border: Color::DarkGray,
            tab_active_fg: Color::Rgb(202, 158, 230),
            tab_inactive_fg: Color::Rgb(165, 172, 199),
            tag_bg: Color::Rgb(65, 69, 89),
            tag_fg: Color::Rgb(170, 175, 180),
            tag_selected_bg: Color::Rgb(90, 130, 90),
            highlight_bg: Color::Rgb(140, 170, 238),
            highlight_fg: Color::Rgb(48, 52, 70),
            match_highlight_fg: Color::Rgb(229, 200, 144),
            section_fg: Color::Rgb(140, 170, 238),
            favorite_fg: Color::Rgb(229, 200, 144),
            hint_fg: Color::DarkGray,
            popup_border: Color::Rgb(202, 158, 230),
            create_fg: Color::Rgb(166, 209, 137),
            scrollbar_fg: Color::DarkGray,
            header_bg: Color::Rgb(48, 52, 70),
            header_fg: Color::Rgb(140, 170, 238),
            help_keys_fg: Color::Rgb(229, 200, 144),
            help_name_fg: Color::Rgb(198, 208, 245),
            help_desc_fg: Color::DarkGray,
            help_search_border: Color::Rgb(108, 178, 220),
            tag_popup_border: Color::Rgb(229, 200, 144),
            input_text: Color::Rgb(198, 208, 245),
        }
    }

    pub fn latte() -> Self {
        Self {
            focus_border: Color::Rgb(136, 57, 239),
            unfocus_border: Color::Rgb(156, 160, 176),
            tab_active_fg: Color::Rgb(136, 57, 239),
            tab_inactive_fg: Color::Rgb(100, 106, 130),
            tag_bg: Color::Rgb(204, 208, 218),
            tag_fg: Color::Rgb(76, 79, 105),
            tag_selected_bg: Color::Rgb(140, 180, 140),
            highlight_bg: Color::Rgb(30, 102, 245),
            highlight_fg: Color::Rgb(239, 241, 245),
            match_highlight_fg: Color::Rgb(223, 142, 29),
            section_fg: Color::Rgb(30, 102, 245),
            favorite_fg: Color::Rgb(223, 142, 29),
            hint_fg: Color::Rgb(156, 160, 176),
            popup_border: Color::Rgb(136, 57, 239),
            create_fg: Color::Rgb(64, 160, 43),
            scrollbar_fg: Color::Rgb(156, 160, 176),
            header_bg: Color::Rgb(239, 241, 245),
            header_fg: Color::Rgb(30, 102, 245),
            help_keys_fg: Color::Rgb(223, 142, 29),
            help_name_fg: Color::Rgb(76, 79, 105),
            help_desc_fg: Color::Rgb(156, 160, 176),
            help_search_border: Color::Rgb(29, 117, 183),
            tag_popup_border: Color::Rgb(223, 142, 29),
            input_text: Color::Rgb(76, 79, 105),
        }
    }
    pub fn name(&self) -> &str {
        if self.focus_border == Theme::mocha().focus_border {
            "Mocha"
        } else if self.focus_border == Theme::macchiato().focus_border {
            "Macchiato"
        } else if self.focus_border == Theme::frappe().focus_border {
            "Frappe"
        } else {
            "Latte"
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Theme::mocha()
    }
}

impl CatppuccinFlavor {
    pub fn theme(&self) -> Theme {
        match self {
            CatppuccinFlavor::Latte => Theme::latte(),
            CatppuccinFlavor::Frappe => Theme::frappe(),
            CatppuccinFlavor::Macchiato => Theme::macchiato(),
            CatppuccinFlavor::Mocha => Theme::mocha(),
        }
    }
}
