#![windows_subsystem = "windows"]

mod field;
mod layout;
mod notify;
mod sections;
mod single_instance;
mod startup;

use std::sync::Arc;

use gpui::{
    App, Application, Bounds, Context, Image, ImageFormat, SharedString, Window, WindowBackgroundAppearance,
    WindowBounds, WindowOptions, div, img, prelude::*, px, rgba, size, transparent_black,
};
use gpui_component::{
    ActiveTheme as _, IconName, Root, Theme, TitleBar, h_flex,
    sidebar::{Sidebar, SidebarMenu, SidebarMenuItem},
    v_flex,
};

use layout::Page;

const SIDEBAR_WIDTH: f32 = 200.0;
const MINIMUM_WINDOW_SIZE: (f32, f32) = (720.0, 480.0);
const DEFAULT_WINDOW_SIZE: (f32, f32) = (980.0, 620.0);
const REPOSITORY_URL: &str = "https://github.com/hibatillah/notch";
const REPOSITORY_LABEL: &str = "Repository";
const LOGO_SIZE: f32 = 22.0;
// The notch's own font files (SIL Open Font License, assets/fonts/Geist-LICENSE.txt).
const GEIST_FONTS: [&[u8]; 2] =
    [include_bytes!("../../assets/fonts/Geist-Regular.ttf"), include_bytes!("../../assets/fonts/Geist-Medium.ttf")];
const FONT_FAMILY: &str = "Geist";
// A fixed strip at the top: the sidebar and the content start below it so nothing sits under the window buttons.
const HEADER_HEIGHT: f32 = 34.0;

/// Greys from the sidebar up to the input field; each step is a little lighter so the layers stay close together.
struct Palette {
    sidebar: u32,
    content: u32,
    group: u32,
    input: u32,
    border: u32,
    // Text in the content and in the sidebar differ because the sidebar is the darker surface.
    muted: u32,
    sidebar_muted: u32,
}

fn palette(cx: &App) -> Palette {
    if cx.theme().is_dark() {
        Palette {
            sidebar: 0x1c1c1c99,
            content: 0x232323b3,
            group: 0xffffff0d,
            input: 0xffffff14,
            border: 0xffffff1f,
            muted: 0xb0b0b0ff,
            sidebar_muted: 0x9a9a9aff,
        }
    } else {
        Palette {
            sidebar: 0xeeeeeeb3,
            content: 0xf6f6f6cc,
            group: 0xffffff99,
            input: 0xffffffe6,
            border: 0x0000001a,
            muted: 0x666666ff,
            sidebar_muted: 0x777777ff,
        }
    }
}

/// The field's own background, lighter than the group it sits in.
pub fn input_background(cx: &App) -> gpui::Hsla {
    rgba(palette(cx).input).into()
}

/// Muted text in the sidebar, a different grey from the content's because the surface is darker.
pub fn sidebar_muted(cx: &App) -> gpui::Hsla {
    rgba(palette(cx).sidebar_muted).into()
}

/// Sidebar and content get their own greys, and the title bar is clear so both run up to the top edge.
fn apply_palette(cx: &mut App) {
    let colors = palette(cx);
    let theme = Theme::global_mut(cx);
    theme.font_family = FONT_FAMILY.into();
    theme.sidebar = rgba(colors.sidebar).into();
    theme.sidebar_border = transparent_black();
    theme.background = rgba(colors.content).into();
    theme.group_box = rgba(colors.group).into();
    theme.accordion = rgba(colors.group).into();
    theme.accordion_hover = rgba(colors.input).into();
    theme.border = rgba(colors.border).into();
    theme.muted_foreground = rgba(colors.muted).into();
    theme.title_bar = transparent_black();
    theme.title_bar_border = transparent_black();
}

struct SettingsWindow {
    selected_page_index: usize,
    pages: Vec<Page>,
    logo: Arc<Image>,
}

impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let menu = SidebarMenu::new().children(self.pages.iter().enumerate().map(|(page_index, page)| {
            let view = view.clone();
            SidebarMenuItem::new(page.title).active(page_index == self.selected_page_index).on_click(
                move |_event, _window, cx| {
                    view.update(cx, |this, cx| {
                        this.selected_page_index = page_index;
                        cx.notify();
                    });
                },
            )
        }));
        let brand = h_flex().gap_2().items_center().child(img(self.logo.clone()).size(px(LOGO_SIZE))).child(
            h_flex()
                .gap_1()
                .items_baseline()
                .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child("Notch"))
                .child(div().text_xs().text_color(sidebar_muted(cx)).child(concat!("v", env!("CARGO_PKG_VERSION")))),
        );
        let repository = div().w_full().child(
            SidebarMenu::new().child(
                SidebarMenuItem::new(REPOSITORY_LABEL)
                    .icon(IconName::GitHub)
                    .suffix(div().text_color(sidebar_muted(cx)).child("\u{2197}"))
                    .on_click(|_event, _window, cx| cx.open_url(REPOSITORY_URL)),
            ),
        );
        let sidebar =
            Sidebar::left().w(px(SIDEBAR_WIDTH)).collapsed(false).header(brand).child(menu).footer(repository);
        let content = div()
            .id("content")
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .px_6()
            .pb_6()
            .pt(px(HEADER_HEIGHT + 8.0))
            .child(self.pages[self.selected_page_index].render(window, cx));

        v_flex()
            .relative()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(h_flex().size_full().child(sidebar).child(content))
            .child(div().absolute().top_0().left_0().right_0().child(TitleBar::new()))
    }
}

fn main() {
    let Some(_instance_guard) = single_instance::acquire() else { return };

    Application::new().with_assets(gpui_component_assets::Assets).run(|cx: &mut App| {
        gpui_component::init(cx);
        if let Err(error) =
            cx.text_system().add_fonts(GEIST_FONTS.iter().map(|bytes| std::borrow::Cow::Borrowed(*bytes)).collect())
        {
            eprintln!("could not load Geist, using the system font: {error}");
        }
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(DEFAULT_WINDOW_SIZE.0), px(DEFAULT_WINDOW_SIZE.1)), cx);
        let mut titlebar = TitleBar::title_bar_options();
        titlebar.title = Some(SharedString::from("Notch Settings"));
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(titlebar),
            // Acrylic: the palette's alpha lets the desktop show through.
            window_background: WindowBackgroundAppearance::Blurred,
            window_min_size: Some(size(px(MINIMUM_WINDOW_SIZE.0), px(MINIMUM_WINDOW_SIZE.1))),
            ..Default::default()
        };
        let opened = cx.open_window(options, |window, cx| {
            Theme::sync_system_appearance(Some(window), cx);
            apply_palette(cx);
            window
                .observe_window_appearance(|window, cx| {
                    Theme::sync_system_appearance(Some(window), cx);
                    apply_palette(cx);
                })
                .detach();
            let logo = Arc::new(Image::from_bytes(
                ImageFormat::Png,
                include_bytes!("../../assets/branding/logo.png").to_vec(),
            ));
            let view = cx.new(|_| SettingsWindow { selected_page_index: 0, pages: sections::all(), logo });
            cx.new(|cx| Root::new(view, window, cx))
        });
        if let Err(error) = opened {
            eprintln!("could not open the settings window: {error}");
            cx.quit();
            return;
        }
        cx.activate(true);
    });
}
