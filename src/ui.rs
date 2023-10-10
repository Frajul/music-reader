use std::{cell::RefCell, path::Path, rc::Rc};

use cairo::{Context, Format, ImageSurface};
use gtk4::{
    prelude::*, Application, ApplicationWindow, Box, Button, DrawingArea, FileChooserAction,
    FileChooserDialog, HeaderBar, Label, Orientation, ResponseType,
};

pub struct Ui {
    bottom_bar: gtk4::Box,
    header_bar: gtk4::HeaderBar,
    page_indicator: gtk4::Label,
    drawing_area: gtk4::DrawingArea,
    drawing_context: cairo::Context,
    document_canvas: Option<DocumentCanvas>,
}

pub struct DocumentCanvas {
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

pub fn toggle_fullscreen(ui: &Ui) {
    match ui.header_bar.is_visible() {
        true => {
            ui.header_bar.hide();
            ui.bottom_bar.hide();
        }
        false => {
            ui.header_bar.show();
            ui.bottom_bar.show();
        }
    }
}

fn update_page_status(ui: &Ui) {
    let page_status = match &ui.document_canvas {
        Some(doc) => format!("{} / {}", doc.current_page_number, doc.num_pages),
        None => "No document loaded!".to_string(),
    };
    ui.page_indicator.set_label(page_status.as_str());
    ui.drawing_area.queue_draw();
}

fn process_touch(ui: &mut Ui, x: f64, y: f64) {
    if ui.document_canvas.is_none() {
        return;
    }

    let doc = ui.document_canvas.as_mut().unwrap();
    let center = ui.drawing_area.width() / 2;
    if y < (ui.drawing_area.height() / 5) as f64 {
        toggle_fullscreen(ui);
    } else if x > center as f64 && doc.current_page_number < doc.num_pages {
        doc.current_page_number += 1;
    } else if x < center as f64 && doc.current_page_number > 1 {
        doc.current_page_number -= 1;
    }
    update_page_status(ui);
}

fn create_drawing_context() -> Context {
    let surface = ImageSurface::create(Format::Rgb24, 0, 0).unwrap();
    Context::new(&surface).unwrap()
}

impl Ui {
    pub fn build(app: &Application) -> Rc<RefCell<Ui>> {
        println!("building ui");
        let open_file_button = Button::from_icon_name("document-open");

        let app_wrapper = Box::builder().orientation(Orientation::Vertical).build();

        let ui = Ui {
            bottom_bar: Box::builder().hexpand_set(true).build(),
            header_bar: HeaderBar::builder().build(),
            page_indicator: Label::builder().build(),
            drawing_area: DrawingArea::builder().hexpand(true).vexpand(true).build(),
            drawing_context: create_drawing_context(),
            document_canvas: None,
        };
        let ui = Rc::new(RefCell::new(ui));

        ui.borrow().header_bar.pack_start(&open_file_button);
        app_wrapper.prepend(&ui.borrow().drawing_area);
        app_wrapper.append(&ui.borrow().bottom_bar);
        ui.borrow().bottom_bar.append(&ui.borrow().page_indicator);

        let click = gtk4::GestureClick::new();
        click.set_button(0);
        click.connect_pressed(glib::clone!(@weak ui => @default-panic, move |_, _, x, y| {
        process_touch(&mut ui.borrow_mut(), x, y);
             }));

        ui.borrow().drawing_area.add_controller(click);

        ui.borrow().drawing_area.set_draw_func(
            glib::clone!(@weak ui => move |area, context, _, _| {
                draw(&ui.borrow(), area, context);
            }),
        );

        let window = ApplicationWindow::builder()
            .application(app)
            .title("Music Reader")
            .child(&app_wrapper)
            .maximized(true)
            .build();
        let window = Rc::new(RefCell::new(window));

        window.borrow().set_titlebar(Some(&ui.borrow().header_bar));

        open_file_button.connect_clicked(
            glib::clone!(@strong ui, @strong window => @default-panic, move |_button| {
                choose_file(Rc::clone(&ui), &window.borrow());
            }),
        );

        window.borrow().present();
        ui
    }
}

fn draw(ui: &Ui, area: &DrawingArea, context: &Context) {
    if ui.document_canvas.is_none() {
        return;
    }
    let document_canvas = ui.document_canvas.as_ref().unwrap();

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

fn choose_file(ui: Rc<RefCell<Ui>>, window: &ApplicationWindow) {
    let filechooser = FileChooserDialog::builder()
        .title("Choose a PDF...")
        .action(FileChooserAction::Open)
        .modal(true)
        .build();
    filechooser.add_button("_Cancel", ResponseType::Cancel);
    filechooser.add_button("_Open", ResponseType::Accept);
    filechooser.set_transient_for(Some(window));
    filechooser.connect_response(move |d, response| {
        if response == ResponseType::Accept {
            let path = d.file().unwrap().path().unwrap();
            load_document(path, Rc::clone(&ui));
        }
        d.destroy();
    });
    filechooser.show()
}

pub fn load_document(file: impl AsRef<Path>, ui: Rc<RefCell<Ui>>) {
    println!("Loading file...");
    // TODO: catch errors, maybe show error dialog
    let uri = format!("file://{}", file.as_ref().to_str().unwrap());
    let document_canvas = DocumentCanvas::new(poppler::Document::from_file(&uri, None).unwrap());
    ui.borrow_mut().document_canvas = Some(document_canvas);

    update_page_status(&ui.borrow());
}
