#![windows_subsystem = "windows"]

mod notify;
mod sections;
mod single_instance;

use gpui::{
    App, Application, Bounds, Context, Rgba, SharedString, TitlebarOptions, Window, WindowAppearance, WindowBounds,
    WindowOptions, div, prelude::*, px, rgb, size,
};
use notch_store::Settings;

struct Palette {
    sidebar: Rgba,
    content: Rgba,
    selected: Rgba,
    hover: Rgba,
    text: Rgba,
}

impl Palette {
    fn for_appearance(appearance: WindowAppearance) -> Self {
        match appearance {
            WindowAppearance::Dark | WindowAppearance::VibrantDark => Self {
                sidebar: rgb(0x1e1e1e),
                content: rgb(0x252525),
                selected: rgb(0x3a3a3a),
                hover: rgb(0x2c2c2c),
                text: rgb(0xf0f0f0),
            },
            WindowAppearance::Light | WindowAppearance::VibrantLight => Self {
                sidebar: rgb(0xf0f0f0),
                content: rgb(0xfafafa),
                selected: rgb(0xd8d8d8),
                hover: rgb(0xe4e4e4),
                text: rgb(0x1a1a1a),
            },
        }
    }
}

struct SettingsWindow {
    selected_section_index: usize,
    // Section bodies will read and write settings through this.
    #[allow(dead_code)]
    settings: Settings,
}

impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = Palette::for_appearance(window.appearance());
        let sidebar = div().flex().flex_col().w(px(180.0)).h_full().p_2().gap_1().bg(palette.sidebar).children(
            sections::SECTION_NAMES.iter().enumerate().map(|(section_index, section_name)| {
                let selected = section_index == self.selected_section_index;
                let (selected_color, hover_color) = (palette.selected, palette.hover);
                div()
                    .id(section_index)
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .when(selected, |item| item.bg(selected_color))
                    .hover(|item| item.bg(hover_color))
                    .child(*section_name)
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        this.selected_section_index = section_index;
                        cx.notify();
                    }))
            }),
        );

        let content = div()
            .flex()
            .flex_col()
            .flex_1()
            .p_4()
            .gap_3()
            .child(div().text_xl().child(sections::SECTION_NAMES[self.selected_section_index]));

        div().flex().size_full().bg(palette.content).text_color(palette.text).child(sidebar).child(content)
    }
}

fn main() {
    let Some(_instance_guard) = single_instance::acquire() else { return };

    Application::new().run(|cx: &mut App| {
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(720.0), px(480.0)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions { title: Some(SharedString::from("Notch Settings")), ..Default::default() }),
            ..Default::default()
        };
        let opened = cx.open_window(options, |window, cx| {
            let settings_window = cx.new(|_| SettingsWindow { selected_section_index: 0, settings: Settings::load() });
            let observed_window = settings_window.clone();
            window
                .observe_window_appearance(move |_window, cx| observed_window.update(cx, |_, cx| cx.notify()))
                .detach();
            settings_window
        });
        if let Err(error) = opened {
            eprintln!("could not open the settings window: {error}");
            cx.quit();
            return;
        }
        cx.activate(true);
    });
}
