use std::{cell::RefCell, path::Path, rc::Rc};

use cairo::{Context, Format, ImageSurface};
use gtk4::{
    prelude::*, Application, ApplicationWindow, Box, Button, DrawingArea, FileChooserAction,
    FileChooserDialog, HeaderBar, Label, Orientation, ResponseType,
};
use poppler::Page;

pub struct Ui {
    window: ApplicationWindow,
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
    page_left: Option<Page>,
    page_right: Option<Page>,
}

impl DocumentCanvas {
    pub fn new(document: poppler::Document) -> Self {
        let num_pages = document.n_pages();
        let page_left = document.page(0);
        let page_right = document.page(1);
        DocumentCanvas {
            document,
            num_pages,
            current_page_number: 1,
            page_left,
            page_right,
        }
    }

    pub fn increase_page_number(&mut self) {
        if self.current_page_number >= self.num_pages - 1 {
            return;
        }

        self.current_page_number += 1;
        self.page_left = self.page_right.take();
        self.page_right = self.document.page(self.current_page_number);
    }

    pub fn decrease_page_number(&mut self) {
        if self.current_page_number <= 1 {
            return;
        }

        self.current_page_number -= 1;
        self.page_right = self.page_left.take();
        self.page_left = self.document.page(self.current_page_number - 1);
    }
}

pub fn toggle_fullscreen(ui: &Ui) {
    match !ui.window.is_fullscreen() {
        true => {
            ui.header_bar.hide();
            ui.bottom_bar.hide();
            ui.window.fullscreen();
        }
        false => {
            ui.header_bar.show();
            ui.bottom_bar.show();
            ui.window.unfullscreen();
        }
    }
}

fn update_page_status(ui: &Ui) {
    let page_status = match &ui.document_canvas {
        Some(doc) => {
            if doc.num_pages == 1 {
                format!("{} / {}", doc.current_page_number, doc.num_pages)
            } else {
                format!(
                    "{}-{} / {}",
                    doc.current_page_number,
                    doc.current_page_number + 1,
                    doc.num_pages
                )
            }
        }
        None => "No document loaded!".to_string(),
    };
    ui.page_indicator.set_label(page_status.as_str());
    ui.drawing_area.queue_draw();
}

fn process_right_click(ui: &mut Ui, x: f64, y: f64) {
    if ui.document_canvas.is_none() {
        return;
    }

    ui.document_canvas.as_mut().unwrap().decrease_page_number();
    update_page_status(ui);
}

fn process_left_click(ui: &mut Ui, x: f64, y: f64) {
    if ui.document_canvas.is_none() {
        return;
    }

    let center = ui.drawing_area.width() / 2;
    if y < (ui.drawing_area.height() / 5) as f64 {
        toggle_fullscreen(ui);
    } else if x > center as f64 {
        ui.document_canvas.as_mut().unwrap().increase_page_number();
    } else if x < center as f64 {
        ui.document_canvas.as_mut().unwrap().decrease_page_number();
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
        let window = ApplicationWindow::builder()
            .application(app)
            .title("Music Reader")
            .child(&app_wrapper)
            .maximized(true)
            .build();

        let ui = Ui {
            window,
            bottom_bar: Box::builder().hexpand_set(true).build(),
            header_bar: HeaderBar::builder().build(),
            page_indicator: Label::builder().build(),
            drawing_area: DrawingArea::builder()
                .width_request(400)
                .height_request(300)
                .hexpand(true)
                .vexpand(true)
                .build(),
            drawing_context: create_drawing_context(),
            document_canvas: None,
        };
        let ui = Rc::new(RefCell::new(ui));

        ui.borrow().header_bar.pack_start(&open_file_button);
        app_wrapper.prepend(&ui.borrow().drawing_area);
        app_wrapper.append(&ui.borrow().bottom_bar);
        ui.borrow().bottom_bar.append(&ui.borrow().page_indicator);

        let click_left = gtk4::GestureClick::new();
        click_left.set_button(1);
        click_left.connect_pressed(glib::clone!(@weak ui => @default-panic, move |_, _, x, y| {
        process_left_click(&mut ui.borrow_mut(), x, y);
             }));

        let click_right = gtk4::GestureClick::new();
        click_right.set_button(3);
        click_right.connect_pressed(glib::clone!(@weak ui => @default-panic, move |_, _, x, y| {
        process_right_click(&mut ui.borrow_mut(), x, y);
             }));

        ui.borrow().drawing_area.add_controller(click_left);
        ui.borrow().drawing_area.add_controller(click_right);

        ui.borrow().drawing_area.set_draw_func(
            glib::clone!(@weak ui => move |area, context, _, _| {
                draw(&ui.borrow(), area, context);
            }),
        );

        ui.borrow()
            .window
            .set_titlebar(Some(&ui.borrow().header_bar));

        open_file_button.connect_clicked(
            glib::clone!(@strong ui => @default-panic, move |_button| {
                choose_file(Rc::clone(&ui), &ui.borrow().window);
            }),
        );

        ui.borrow().window.present();
        ui
    }
}

fn draw(ui: &Ui, area: &DrawingArea, context: &Context) {
    if ui.document_canvas.is_none() {
        return;
    }
    let document_canvas = ui.document_canvas.as_ref().unwrap();
    if document_canvas.num_pages > 1 {
        draw_two_pages(ui, area, context);
    } else {
        draw_single_page(ui, area, context);
    }
}

fn draw_two_pages(ui: &Ui, area: &DrawingArea, context: &Context) {
    if ui.document_canvas.is_none() {
        return;
    }
    let document_canvas = ui.document_canvas.as_ref().unwrap();

    if document_canvas.page_left.is_none() || document_canvas.page_right.is_none() {
        return;
    }

    let page_left = document_canvas.page_left.as_ref().unwrap();
    let page_right = document_canvas.page_right.as_ref().unwrap();

    // Add white background
    // context.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    // context.fill().unwrap();
    // context.paint().unwrap();

    let (w_left, h_left) = page_left.size();
    let (w_right, h_right) = page_right.size();

    let h_max = f64::max(h_left, h_right);
    // Make sure both pages are rendered with the same height
    let w_max = match h_left < h_right {
        true => w_left * h_right / h_left + w_right,
        false => w_left + w_right * h_left / h_right,
    };

    let h_scale = area.height() as f64 / h_max;
    let w_scale = area.width() as f64 / w_max;
    let scale = f64::min(h_scale, w_scale);
    let h_page = h_max * scale;

    let scale_left = h_page / h_left;
    let scale_right = h_page / h_right;

    context.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    context.save().unwrap();
    context.translate(
        area.width() as f64 / 2.0 - w_left * scale_left,
        area.height() as f64 / 2.0 - h_page / 2.0,
    );
    // Poppler sometimes crops white border, draw it manually
    context.rectangle(0.0, 0.0, w_left * scale_left, h_page);
    context.fill().unwrap();
    context.scale(scale_left, scale_left);
    page_left.render(context);

    context.restore().unwrap();
    context.translate(
        area.width() as f64 / 2.0,
        area.height() as f64 / 2.0 - h_page / 2.0,
    );
    // Poppler sometimes crops white border, draw it manually
    context.rectangle(0.0, 0.0, w_right * scale_right, h_page);
    context.fill().unwrap();
    context.scale(scale_right, scale_right);
    page_right.render(context);

    let r = ui.drawing_context.paint();
    match r {
        Err(v) => println!("Error painting PDF: {v:?}"),
        Ok(_v) => {}
    }

    ui.drawing_context.show_page().unwrap();
}
fn draw_single_page(ui: &Ui, area: &DrawingArea, context: &Context) {
    if ui.document_canvas.is_none() {
        return;
    }
    let document_canvas = ui.document_canvas.as_ref().unwrap();

    if document_canvas.page_left.is_none() {
        return;
    }

    let page = document_canvas.page_left.as_ref().unwrap();

    // Draw background
    // context.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    // context.paint().unwrap();
    // context.fill().expect("uh oh");
    // context.paint().unwrap();

    let (w, h) = page.size();

    let width_diff = area.width() as f64 / w;
    let height_diff = area.height() as f64 / h;
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

    // Poppler sometimes crops white border, draw it manually
    context.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    context.rectangle(0.0, 0.0, w, h);
    context.fill().unwrap();

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
