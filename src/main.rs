mod ui;

use clap::Parser;
use gtk4::prelude::*;
use gtk4::Application;
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
    let cli = Cli::parse();
    println!("Parse args");
    let app = Application::builder().application_id(APP_ID).build();

    app.connect_activate(move |app| {
        let myui = build_ui(&app);
        match cli.file.as_ref() {
            Some(file) => ui::load_document(file, Rc::clone(&myui)),
            None => {}
        }
    });

    app.run_with_args(&[] as &[&str]);
}

fn build_ui(app: &Application) -> Rc<RefCell<Ui>> {
    let ui = Ui::build(app);
    ui
}
