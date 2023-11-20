use std::{cell::RefCell, collections::BTreeMap, path::Path, rc::Rc, thread, time::Duration};

use async_channel::Sender;
use cairo::{Context, Format, ImageSurface};
use gtk::{
    gdk::Paintable, gio, glib, prelude::*, Application, ApplicationWindow, Box, Button,
    DrawingArea, FileChooserAction, FileChooserDialog, HeaderBar, Label, Orientation, Picture,
    ResponseType,
};
use poppler::{Document, Page};

use crate::cache::{self, CacheCommand, PageCache};
use glib::clone;
use gtk::prelude::*;

pub struct Ui {
    window: ApplicationWindow,
    bottom_bar: gtk::Box,
    header_bar: gtk::HeaderBar,
    page_indicator: gtk::Label,
    drawing_area: gtk::DrawingArea,
    picture: Picture,
    drawing_context: cairo::Context,
    document_canvas: Option<DocumentCanvas>,
}

pub struct DocumentCanvas {
    current_page_number: usize,
    num_pages: Option<usize>,
    page_cache_sender: Sender<CacheCommand>,
}

impl DocumentCanvas {
    pub fn new(page_cache_sender: Sender<CacheCommand>) -> Self {
        DocumentCanvas {
            current_page_number: 0,
            num_pages: None,
            page_cache_sender,
        }
    }

    pub fn increase_page_number(&mut self) {
        if self.current_page_number >= self.num_pages.unwrap_or(0) - 2 {
            return;
        }

        self.current_page_number += 1;
    }

    pub fn decrease_page_number(&mut self) {
        if self.current_page_number <= 0 {
            return;
        }

        self.current_page_number -= 1;
    }

    pub fn cache_surrounding_pages(&self) {
        self.page_cache_sender
            .send_blocking(CacheCommand::CachePages {
                pages: vec![
                    self.current_page_number.saturating_sub(2),
                    self.current_page_number.saturating_sub(1),
                    self.current_page_number,
                    self.current_page_number + 1,
                    self.current_page_number + 2,
                    self.current_page_number + 3,
                ],
            });
    }

    pub fn request_to_draw_pages(&self) {
        self.page_cache_sender
            .send_blocking(CacheCommand::GetCurrentTwoPages {
                page_left_number: self.current_page_number,
            });
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
            if doc.num_pages.unwrap_or(0) == 1 {
                format!(
                    "{} / {}",
                    doc.current_page_number,
                    doc.num_pages.unwrap_or(0)
                )
            } else {
                format!(
                    "{}-{} / {}",
                    doc.current_page_number + 1,
                    doc.current_page_number + 2,
                    doc.num_pages.unwrap_or(0)
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
        if x < ui.drawing_area.width() as f64 * 0.75 {
            ui.document_canvas.as_mut().unwrap().increase_page_number();
        } else {
            ui.document_canvas.as_mut().unwrap().increase_page_number();
            ui.document_canvas.as_mut().unwrap().increase_page_number();
        }
    } else if x < center as f64 {
        if x > ui.drawing_area.width() as f64 * 0.25 {
            ui.document_canvas.as_mut().unwrap().decrease_page_number();
        } else {
            ui.document_canvas.as_mut().unwrap().decrease_page_number();
            ui.document_canvas.as_mut().unwrap().decrease_page_number();
        }
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
            picture: Picture::builder()
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
        // app_wrapper.prepend(&ui.borrow().drawing_area);
        app_wrapper.prepend(&ui.borrow().picture);
        app_wrapper.append(&ui.borrow().bottom_bar);
        ui.borrow().bottom_bar.append(&ui.borrow().page_indicator);

        let click_left = gtk::GestureClick::new();
        click_left.set_button(1);
        click_left.connect_pressed(glib::clone!(@weak ui => @default-panic, move |_, _, x, y| {
        process_left_click(&mut ui.borrow_mut(), x, y);
             }));

        let click_right = gtk::GestureClick::new();
        click_right.set_button(3);
        click_right.connect_pressed(glib::clone!(@weak ui => @default-panic, move |_, _, x, y| {
        process_right_click(&mut ui.borrow_mut(), x, y);
             }));

        // ui.borrow().drawing_area.add_controller(click_left);
        // ui.borrow().drawing_area.add_controller(click_right);
        ui.borrow().picture.add_controller(click_left);
        ui.borrow().picture.add_controller(click_right);

        ui.borrow().drawing_area.set_draw_func(
            glib::clone!(@weak ui => move |area, context, _, _| {
                // draw(&mut ui.borrow_mut(), area, context, sender.clone());
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

// fn draw(ui: &mut Ui, area: &DrawingArea, context: &Context, sender: Sender<String>) {
//     if ui.document_canvas.is_none() {
//         return;
//     }
//     let document_canvas = ui.document_canvas.as_ref().unwrap();
//     if document_canvas.num_pages.unwrap_or(0) > 1 {
//         draw_two_pages(ui, area, context);
//     } else {
//         draw_single_page(ui, area, context);
//     }

//     // gio::spawn_blocking(move || {
//     //     ui.document_canvas
//     //         .as_mut()
//     //         .unwrap()
//     //         .cache_surrounding_pages();
//     // });
// }

// fn draw_two_pages(ui: &Ui, area: &DrawingArea, context: &Context) {
//     if ui.document_canvas.is_none() {
//         return;
//     }
//     let document_canvas = ui.document_canvas.as_ref().unwrap();

//     let page_left = document_canvas.get_left_page();
//     let page_right = document_canvas.get_right_page();

//     if page_left.is_none() || page_right.is_none() {
//         // TODO: show error message
//         return;
//     }

//     let page_left = page_left.unwrap();
//     let page_right = page_right.unwrap();

//     // Add white background
//     // context.set_source_rgba(1.0, 1.0, 1.0, 1.0);
//     // context.fill().unwrap();
//     // context.paint().unwrap();

//     let (w_left, h_left) = page_left.size();
//     let (w_right, h_right) = page_right.size();

//     let h_max = f64::max(h_left, h_right);
//     // Make sure both pages are rendered with the same height
//     let w_max = match h_left < h_right {
//         true => w_left * h_right / h_left + w_right,
//         false => w_left + w_right * h_left / h_right,
//     };

//     let h_scale = area.height() as f64 / h_max;
//     let w_scale = area.width() as f64 / w_max;
//     let scale = f64::min(h_scale, w_scale);
//     let h_page = h_max * scale;

//     let scale_left = h_page / h_left;
//     let scale_right = h_page / h_right;

//     context.set_source_rgba(1.0, 1.0, 1.0, 1.0);
//     context.save().unwrap();
//     context.translate(
//         area.width() as f64 / 2.0 - w_left * scale_left,
//         area.height() as f64 / 2.0 - h_page / 2.0,
//     );
//     // Poppler sometimes crops white border, draw it manually
//     context.rectangle(0.0, 0.0, w_left * scale_left, h_page);
//     context.fill().unwrap();
//     context.scale(scale_left, scale_left);
//     page_left.render(context);

//     context.restore().unwrap();
//     context.translate(
//         area.width() as f64 / 2.0,
//         area.height() as f64 / 2.0 - h_page / 2.0,
//     );
//     // Poppler sometimes crops white border, draw it manually
//     context.rectangle(0.0, 0.0, w_right * scale_right, h_page);
//     context.fill().unwrap();
//     context.scale(scale_right, scale_right);
//     page_right.render(context);

//     let r = ui.drawing_context.paint();
//     match r {
//         Err(v) => println!("Error painting PDF: {v:?}"),
//         Ok(_v) => {}
//     }

//     ui.drawing_context.show_page().unwrap();
// }
// fn draw_single_page(ui: &Ui, area: &DrawingArea, context: &Context) {
//     if ui.document_canvas.is_none() {
//         return;
//     }
//     let document_canvas = ui.document_canvas.as_ref().unwrap();

//     if document_canvas.get_left_page().is_none() {
//         // TODO: show error message
//         return;
//     }

//     let page = document_canvas.get_left_page().unwrap();

//     // Draw background
//     // context.set_source_rgba(1.0, 1.0, 1.0, 1.0);
//     // context.paint().unwrap();
//     // context.fill().expect("uh oh");
//     // context.paint().unwrap();

//     let (w, h) = page.size();

//     let width_diff = area.width() as f64 / w;
//     let height_diff = area.height() as f64 / h;
//     if width_diff > height_diff {
//         context.translate(
//             (area.width() as f64 - w * height_diff) / 2.0,
//             (area.height() as f64 - h * height_diff) / 2.0,
//         );
//         context.scale(height_diff, height_diff);
//     } else {
//         context.translate(
//             (area.width() as f64 - w * width_diff) / 2.0,
//             (area.height() as f64 - h * width_diff) / 2.0,
//         );
//         context.scale(width_diff, width_diff);
//     }

//     // Poppler sometimes crops white border, draw it manually
//     context.set_source_rgba(1.0, 1.0, 1.0, 1.0);
//     context.rectangle(0.0, 0.0, w, h);
//     context.fill().unwrap();

//     page.render(context);

//     let r = ui.drawing_context.paint();
//     match r {
//         Err(v) => println!("Error painting PDF: {v:?}"),
//         Ok(_v) => {}
//     }

//     ui.drawing_context.show_page().unwrap();
// }

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
    // let uri = format!("file://{}", file.as_ref().to_str().unwrap());

    let (sender, receiver) = cache::spawn_async_cache(file);
    // gtk::spawn
    glib::spawn_future_local(clone!(@weak ui => async move {
        while let Ok(cache_response) = receiver.recv().await {
            match cache_response{
    cache::CacheResponse::DocumentLoaded { num_pages } => todo!(), //ui.borrow_mut().document_canvas.unwrap().num_pages = Some(num_pages),
                cache::CacheResponse::SinglePageLoaded { page } => { ui.borrow_mut().picture.set_paintable(Some(page.as_ref()));},
    cache::CacheResponse::TwoPagesLoaded { page_left, page_right } => todo!(),
            }
        }
    }));

    let document_canvas = DocumentCanvas::new(sender);
    document_canvas.cache_surrounding_pages();
    // document_canvas.cache_all_pages();

    // update_page_status(&ui.borrow());
    document_canvas.request_to_draw_pages();
    ui.borrow_mut().document_canvas = Some(document_canvas);
}
