mod cache;
mod draw;
mod ui;

use clap::Parser;
use env_logger::Env;
use gtk::prelude::*;
use gtk::Application;
use log::debug;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use ui::Ui;

const APP_ID: &str = "de.frajul.music-reader";

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    file: Option<PathBuf>,
}

fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("debug")).init();
    let cli = Cli::parse();
    debug!("Parse args");
    let app = Application::builder().application_id(APP_ID).build();

    app.connect_activate(move |app| {
        let ui = build_ui(app);
        if let Some(file) = cli.file.as_ref() {
            ui::load_document(file, Rc::clone(&ui));
        }
    });

    app.run_with_args(&[] as &[&str]);
}

fn build_ui(app: &Application) -> Rc<RefCell<Ui>> {
    Ui::build(app)
}
