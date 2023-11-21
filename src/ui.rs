use std::{
    cell::RefCell, collections::BTreeMap, path::Path, rc::Rc, sync::Arc, thread, time::Duration,
};

use async_channel::Sender;
use cairo::{Context, Format, ImageSurface, ImageSurfaceData, ImageSurfaceDataOwned};
use gtk::{
    gdk::Paintable, gio, glib, prelude::*, subclass::drawing_area, Application, ApplicationWindow,
    Box, Button, DrawingArea, FileChooserAction, FileChooserDialog, HeaderBar, Label, Orientation,
    Picture, ResponseType,
};
use poppler::{Document, Page};

use crate::{
    cache::{self, CacheCommand, MyPageType, PageCache},
    draw,
};
use glib::clone;
use gtk::prelude::*;

pub struct Ui {
    window: ApplicationWindow,
    bottom_bar: gtk::Box,
    header_bar: gtk::HeaderBar,
    page_indicator: gtk::Label,
    drawing_area: gtk::DrawingArea,
    picture: Picture,
    pub drawing_context: cairo::Context,
    pub document_canvas: Option<DocumentCanvas>,
}

pub struct DocumentCanvas {
    current_page_number: usize,
    pub num_pages: Option<usize>,
    page_cache_sender: Sender<CacheCommand>,
    pub left_page: Option<Arc<MyPageType>>,
    pub right_page: Option<Arc<MyPageType>>,
}

impl DocumentCanvas {
    pub fn new(page_cache_sender: Sender<CacheCommand>) -> Self {
        DocumentCanvas {
            current_page_number: 0,
            num_pages: None,
            page_cache_sender,
            left_page: None,
            right_page: None,
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

    pub fn cache_initial_pages(&self) {
        self.page_cache_sender
            .send_blocking(CacheCommand::CachePages {
                pages: vec![self.current_page_number, self.current_page_number + 1],
            })
            .unwrap();
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
            })
            .unwrap();
    }

    pub fn request_to_draw_pages(&self) {
        self.page_cache_sender
            .send_blocking(CacheCommand::GetCurrentTwoPages {
                page_left_number: self.current_page_number,
            })
            .unwrap();
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
            doc.request_to_draw_pages();

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
        app_wrapper.prepend(&ui.borrow().drawing_area);
        // app_wrapper.prepend(&ui.borrow().picture);
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

        ui.borrow().drawing_area.add_controller(click_left);
        ui.borrow().drawing_area.add_controller(click_right);
        // ui.borrow().picture.add_controller(click_left);
        // ui.borrow().picture.add_controller(click_right);

        ui.borrow().drawing_area.set_draw_func(
            glib::clone!(@weak ui => move |area, context, _, _| {
                draw::draw(&mut ui.borrow_mut(), area, context);
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

    let sender = cache::spawn_async_cache(
        file,
        clone!(@weak ui => move |cache_response| match cache_response {
            cache::CacheResponse::DocumentLoaded { num_pages } => {
                ui.borrow_mut().document_canvas.as_mut().unwrap().num_pages = Some(num_pages);
                update_page_status(&ui.borrow())
            }
            cache::CacheResponse::SinglePageLoaded { page } => {
                ui.borrow_mut().document_canvas.as_mut().unwrap().left_page = Some(page);
                ui.borrow_mut().document_canvas.as_mut().unwrap().right_page = None;
                ui.borrow().drawing_area.queue_draw();
            }
            cache::CacheResponse::TwoPagesLoaded {
                page_left,
                page_right,
            } => {
                ui.borrow_mut().document_canvas.as_mut().unwrap().left_page = Some(page_left);
                ui.borrow_mut().document_canvas.as_mut().unwrap().right_page = Some(page_right);
                ui.borrow().drawing_area.queue_draw();
            }
        }),
    );

    println!("Spawned async cache");
    // // gtk::spawn
    // glib::spawn_future_local(clone!(@weak ui => async move {
    //     println!("Waiting for cache response:...");
    //     while let Ok(cache_response) = receiver.recv().await {
    //         match cache_response{
    //             cache::CacheResponse::DocumentLoaded { num_pages } => {ui.borrow_mut().document_canvas.as_mut().unwrap().num_pages = Some(num_pages); update_page_status(&ui.borrow())},
    //             cache::CacheResponse::SinglePageLoaded { page } => { ui.borrow_mut().document_canvas.as_mut().unwrap().left_page = Some(page);
    //             ui.borrow_mut().document_canvas.as_mut().unwrap().right_page = None;
    // ui.borrow().drawing_area.queue_draw();
    //             },
    // cache::CacheResponse::TwoPagesLoaded { page_left, page_right } => { ui.borrow_mut().document_canvas.as_mut().unwrap().left_page = Some(page_left);
    //             ui.borrow_mut().document_canvas.as_mut().unwrap().right_page = Some(page_right);
    // ui.borrow().drawing_area.queue_draw();
    // }
    //         }
    //     }
    // }));

    let document_canvas = DocumentCanvas::new(sender);
    document_canvas.cache_initial_pages();
    ui.borrow_mut().document_canvas = Some(document_canvas);

    update_page_status(&ui.borrow());
    println!("finished loading document");
}
