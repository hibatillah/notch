#![windows_subsystem = "windows"]

mod activities;
mod activity;
mod gfx;
mod host;
mod log;
mod page;
mod pages;
mod ui;

fn main() {
    if let Err(error) = host::run(pages::all(), activities::all()) {
        log::debug(&format!("fatal: {error}"));
    }
}
