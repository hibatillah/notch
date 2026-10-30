//! Throwaway GPUI spike: one window with a sidebar and one toggle, used to measure memory, size and licenses.
#![windows_subsystem = "windows"]

use gpui::{App, Application, Bounds, Context, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb, size};

const PAGE_NAMES: [&str; 4] = ["General", "Pages", "Activities", "About"];

struct SettingsWindow {
    selected_page_index: usize,
    notifications_enabled: bool,
}

impl Render for SettingsWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sidebar = div().flex().flex_col().w(px(180.0)).h_full().p_2().gap_1().bg(rgb(0x1e1e1e)).children(
            PAGE_NAMES.iter().enumerate().map(|(page_index, page_name)| {
                let selected = page_index == self.selected_page_index;
                div()
                    .id(page_index)
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .when(selected, |item| item.bg(rgb(0x3a3a3a)))
                    .hover(|item| item.bg(rgb(0x2c2c2c)))
                    .child(*page_name)
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        this.selected_page_index = page_index;
                        cx.notify();
                    }))
            }),
        );

        let enabled = self.notifications_enabled;
        let toggle = div()
            .id("notifications-toggle")
            .flex()
            .w(px(40.0))
            .h(px(22.0))
            .p(px(2.0))
            .rounded_full()
            .bg(if enabled { rgb(0x2f7bf5) } else { rgb(0x555555) })
            .when(enabled, |track| track.justify_end())
            .child(div().size(px(18.0)).rounded_full().bg(rgb(0xffffff)))
            .on_click(cx.listener(|this, _event, _window, cx| {
                this.notifications_enabled = !this.notifications_enabled;
                cx.notify();
            }));

        let content = div()
            .flex()
            .flex_col()
            .flex_1()
            .p_4()
            .gap_3()
            .child(div().text_xl().child(PAGE_NAMES[self.selected_page_index]))
            .child(div().flex().items_center().justify_between().child("Notifications").child(toggle));

        div().flex().size_full().bg(rgb(0x252525)).text_color(rgb(0xf0f0f0)).child(sidebar).child(content)
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(640.0), px(420.0)), cx);
        let options = WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() };
        let opened = cx.open_window(options, |_window, cx| {
            cx.new(|_| SettingsWindow { selected_page_index: 0, notifications_enabled: true })
        });
        if let Err(error) = opened {
            eprintln!("could not open the settings window: {error}");
            cx.quit();
            return;
        }
        cx.activate(true);
    });
}
