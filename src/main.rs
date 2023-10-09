// mod window;

use cairo::Context;
use clap::Parser;
use gio::ApplicationFlags;
use glib::clone;
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, DrawingArea, Label};
use poppler::PopplerDocument;
use std::cell::RefCell;
use std::env;
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

fn build_ui(app: &Application, cli: &Cli) {
    println!("building ui");
    let open_file_button = gtk4::Button::from_icon_name("document-open");

    let app_wrapper = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .build();

    let ui = Ui {
        bottom_bar: gtk4::Box::builder().hexpand_set(true).build(),
        header_bar: gtk4::HeaderBar::builder().build(),
    };
    let ui = Rc::new(RefCell::new(ui));

    ui.borrow().header_bar.pack_start(&open_file_button);
    app_wrapper.append(&ui.borrow().bottom_bar);

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Music Reader")
        .child(&app_wrapper)
        .maximized(true)
        .build();
    window.set_titlebar(Some(&ui.borrow().header_bar));

    // let toggle_fullscreen = clone!(@weak header_bar, @weak bottom_bar => move || {
    //     if header_bar.is_visible() {
    //         header_bar.hide();
    //         bottom_bar.hide();
    //     } else {
    //         header_bar.show();
    //         bottom_bar.show();
    //     }
    // });

    let load_doc = move |file: &PathBuf| {
        println!("Loading file...");
        let drawing_area = DrawingArea::builder()
            // .width_request(100)
            // .height_request(100)
            .hexpand(true)
            .vexpand(true)
            .build();
        let first_child = app_wrapper.first_child().unwrap();
        let last_child = app_wrapper.last_child().unwrap();
        if &first_child != &last_child {
            app_wrapper.remove(&first_child);
        }

        app_wrapper.prepend(&drawing_area);

        let page_indicator = Label::builder().label("Counting").build();
        let old_indicator = ui.borrow().bottom_bar.last_child();
        if old_indicator.is_some() {
            ui.borrow().bottom_bar.remove(&old_indicator.unwrap());
        }
        ui.borrow().bottom_bar.append(&page_indicator);

        let doc = PopplerDocument::new_from_file(file, "").unwrap();

        let num_pages = doc.get_n_pages();
        let current_page_number = Rc::new(RefCell::new(1));

        let surface = cairo::ImageSurface::create(cairo::Format::Rgb24, 0, 0).unwrap();
        let ctx = Context::new(&surface).unwrap();

        let update_page_status = glib::clone!(@strong num_pages, @strong current_page_number, @strong page_indicator, @weak drawing_area => @default-panic, move || {
            let page_status: String = format!("{} of {}", current_page_number.borrow(), num_pages);
            let page_status_s: &str = &page_status[..];
            page_indicator.set_label(page_status_s);
            drawing_area.queue_draw();
        });

        update_page_status();

        let click = gtk4::GestureClick::new();
        click.set_button(0);
        click.connect_pressed(
                 glib::clone!(@weak ui, @weak drawing_area, @strong current_page_number, @strong num_pages, @strong update_page_status => @default-panic, move |_count, _, x, y| {
                     let center = drawing_area.width() / 2;
                     if y < (drawing_area.height() / 5) as f64 {
                     toggle_fullscreen(&ui.borrow());
                     } else if x > center as f64 &&  *current_page_number.borrow() < num_pages{
                        *current_page_number.borrow_mut() += 1;
                     } else if x < center as f64 && *current_page_number.borrow() > 1 {
                        *current_page_number.borrow_mut()  -= 1;
                     }
                     update_page_status();

                 }),
             );

        drawing_area.add_controller(&click);

        drawing_area.set_draw_func(
            glib::clone!(@strong current_page_number => @default-panic, move |area, context, _a, _b| {
                println!("Draw!");
                context.set_source_rgba(1.0, 1.0, 1.0, 1.0);
                context.paint().unwrap();
                context.fill().expect("uh oh");
                context.paint().unwrap();

                let page = doc.get_page(*current_page_number.borrow_mut()- 1).unwrap();
                let (w, h) = page.get_size();

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

                page.render(&context);

                let r = ctx.paint();
                match r {
                    Err(v) => println!("Error painting PDF: {v:?}"),
                    Ok(_v) => {}
                }

                ctx.show_page().unwrap();
            }),
        );
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
