// mod window;

use cairo::Context;
use clap::Parser;
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, DrawingArea};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

const APP_ID: &str = "de.frajul.music-reader";

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    file: Option<PathBuf>,
}

fn main() {
    let cli = Cli::parse();
    println!("Parse args");
    let app = Application::builder()
        .application_id(APP_ID)
        // .flags(gio::ApplicationFlags::FLAGS_NONE)
        .build();

    app.connect_activate(move |app| {
        build_ui(&app, &cli);
    });

    app.run_with_args(&[] as &[&str]);
}

struct Ui {
    bottom_bar: gtk4::Box,
    header_bar: gtk4::HeaderBar,
    page_indicator: gtk4::Label,
    drawing_area: gtk4::DrawingArea,
    drawing_context: Context,
}

struct DocumentCanvas {
    document: poppler::Document,
    current_page_number: i32,
    num_pages: i32,
}

impl DocumentCanvas {
    pub fn new(document: poppler::Document) -> Self {
        let num_pages = document.n_pages();
        DocumentCanvas {
            document,
            num_pages,
            current_page_number: 1,
        }
    }
}

fn toggle_fullscreen(ui: &Ui) {
    if ui.header_bar.is_visible() {
        ui.header_bar.hide();
        ui.bottom_bar.hide();
    } else {
        ui.header_bar.show();
        ui.bottom_bar.show();
    }
}

fn update_page_status(ui: &Ui, doc: &DocumentCanvas) {
    let page_status: String = format!("{} / {}", doc.current_page_number, doc.num_pages);
    let page_status_s: &str = &page_status[..];
    ui.page_indicator.set_label(page_status_s);
    ui.drawing_area.queue_draw();
}

fn process_touch(ui: &Ui, doc: &mut DocumentCanvas, x: f64, y: f64) {
    let center = ui.drawing_area.width() / 2;
    if y < (ui.drawing_area.height() / 5) as f64 {
        toggle_fullscreen(ui);
    } else if x > center as f64 && doc.current_page_number < doc.num_pages {
        doc.current_page_number += 1;
    } else if x < center as f64 && doc.current_page_number > 1 {
        doc.current_page_number -= 1;
    }
    update_page_status(ui, doc);
}

fn draw(ui: &Ui, document_canvas: &DocumentCanvas, area: &DrawingArea, context: &Context) {
    context.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    context.paint().unwrap();
    context.fill().expect("uh oh");
    context.paint().unwrap();

    let page = document_canvas
        .document
        .page(document_canvas.current_page_number - 1)
        .unwrap();
    let (w, h) = page.size();

    let width_diff = area.width() as f64 / w;
    let height_diff = area.height() as f64 / h;
    context.save().unwrap();
    if width_diff > height_diff {
        context.translate(
            (area.width() as f64 - w * height_diff) / 2.0,
            (area.height() as f64 - h * height_diff) / 2.0,
        );
        context.scale(height_diff, height_diff);
    } else {
        context.translate(
            (area.width() as f64 - w * width_diff) / 2.0,
            (area.height() as f64 - h * width_diff) / 2.0,
        );
        context.scale(width_diff, width_diff);
    }

    page.render(context);

    let r = ui.drawing_context.paint();
    match r {
        Err(v) => println!("Error painting PDF: {v:?}"),
        Ok(_v) => {}
    }

    ui.drawing_context.show_page().unwrap();
}

fn load_document(file: &PathBuf, ui: Rc<RefCell<Ui>>) {
    println!("Loading file...");
    // let first_child = app_wrapper.first_child().unwrap();
    // let last_child = app_wrapper.last_child().unwrap();
    // if &first_child != &last_child {
    //     app_wrapper.remove(&first_child);
    // }

    // app_wrapper.prepend(&ui.borrow().drawing_area);

    // let old_indicator = ui.borrow().bottom_bar.last_child();
    // if old_indicator.is_some() {
    //     ui.borrow().bottom_bar.remove(&old_indicator.unwrap());
    // }
    // ui.borrow().bottom_bar.append(&ui.borrow().page_indicator);

    // TODO: catch errors
    let uri = format!("file://{}", file.to_str().unwrap());
    let document_canvas = DocumentCanvas::new(poppler::Document::from_file(&uri, None).unwrap());
    let document_canvas = Rc::new(RefCell::new(document_canvas));

    update_page_status(&ui.borrow(), &document_canvas.borrow());

    let click = gtk4::GestureClick::new();
    click.set_button(0);
    click.connect_pressed(
        glib::clone!(@weak ui, @strong document_canvas => @default-panic, move |_, _, x, y| {
        process_touch(&ui.borrow(), &mut document_canvas.borrow_mut(), x, y);
             }),
    );

    // TODO: opening new file keeps the old controller
    ui.borrow().drawing_area.add_controller(click);

    ui.borrow().drawing_area.set_draw_func(
        glib::clone!(@weak ui, @weak document_canvas => move |area, context, _, _| {
            draw(&ui.borrow(), &document_canvas.borrow(), area, context);
        }),
    );
}

fn build_ui(app: &Application, cli: &Cli) {
    println!("building ui");
    let open_file_button = gtk4::Button::from_icon_name("document-open");

    let app_wrapper = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .build();

    let surface = cairo::ImageSurface::create(cairo::Format::Rgb24, 0, 0).unwrap();
    let ctx = Context::new(&surface).unwrap();

    let ui = Ui {
        bottom_bar: gtk4::Box::builder().hexpand_set(true).build(),
        header_bar: gtk4::HeaderBar::builder().build(),
        page_indicator: gtk4::Label::builder().label("Counting...").build(),
        drawing_area: DrawingArea::builder().hexpand(true).vexpand(true).build(),
        drawing_context: ctx,
    };
    let ui = Rc::new(RefCell::new(ui));

    ui.borrow().header_bar.pack_start(&open_file_button);
    app_wrapper.prepend(&ui.borrow().drawing_area);
    app_wrapper.append(&ui.borrow().bottom_bar);
    ui.borrow().bottom_bar.append(&ui.borrow().page_indicator);

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Music Reader")
        .child(&app_wrapper)
        .maximized(true)
        .build();
    window.set_titlebar(Some(&ui.borrow().header_bar));

    let load_doc = move |file: &PathBuf| {
        load_document(file, Rc::clone(&ui));
    };

    match cli.file.as_ref() {
        Some(file) => load_doc(file),
        None => {}
    }

    open_file_button.connect_clicked(
        glib::clone!(@weak window, @strong load_doc => @default-panic, move |_button| {
            let filechooser = gtk4::FileChooserDialog::builder()
                .title("Choose a PDF...")
                .action(gtk4::FileChooserAction::Open)
                .modal(true)
                .build();
            filechooser.add_button("_Cancel", gtk4::ResponseType::Cancel);
            filechooser.add_button("_Open", gtk4::ResponseType::Accept);
            filechooser.set_transient_for(Some(&window));
            filechooser.connect_response(glib::clone!(@strong load_doc => @default-panic, move |d, response| {
                if response == gtk4::ResponseType::Accept {
                    let path = d.file().unwrap().path().unwrap();
                    load_doc(&path);
                }
                d.destroy();
            }));
            filechooser.show()
        }),
    );
    window.present();
}
