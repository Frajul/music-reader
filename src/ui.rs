use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
};

use gtk::{
    glib, Application, ApplicationWindow, Box, Button, FileChooserAction, FileChooserDialog,
    HeaderBar, Label, Orientation, Picture, ResponseType,
};
use log::debug;

use crate::cache::{self, PageNumber, SyncCacheCommandSender};
use glib::clone;
use gtk::prelude::*;

pub struct Ui {
    window: ApplicationWindow,
    bottom_bar: gtk::Box,
    header_bar: gtk::HeaderBar,
    page_indicator: gtk::Label,
    pub app_wrapper: Box,
    pub image_container: Box,
    pub image_left: Picture,
    pub image_right: Picture,
    pub document_canvas: Option<DocumentCanvas>,
}

pub struct DocumentCanvas {
    pub current_page_number: usize,
    pub num_pages: Option<usize>,
    page_cache_sender: SyncCacheCommandSender,
}

impl DocumentCanvas {
    pub fn new(page_cache_sender: SyncCacheCommandSender) -> Self {
        DocumentCanvas {
            current_page_number: 0,
            num_pages: None,
            page_cache_sender,
        }
    }

    pub fn increase_page_number(&mut self) {
        if self.current_page_number >= self.num_pages.unwrap_or(0).saturating_sub(2) {
            return;
        }

        self.current_page_number += 1;
    }

    pub fn decrease_page_number(&mut self) {
        self.current_page_number = self.current_page_number.saturating_sub(1);
    }

    pub fn cache_initial_pages(&self, area_height: i32) {
        self.page_cache_sender.send_cache_commands(
            &vec![self.current_page_number, self.current_page_number + 1],
            area_height,
        );
    }

    pub fn cache_surrounding_pages(&self, area_height: i32) {
        self.page_cache_sender.send_cache_commands(
            &vec![
                self.current_page_number.saturating_sub(2),
                self.current_page_number.saturating_sub(1),
                self.current_page_number,
                self.current_page_number + 1,
                self.current_page_number + 2,
                self.current_page_number + 3,
            ],
            area_height,
        );
    }

    pub fn request_to_draw_pages(&self) {
        if self.num_pages == Some(1) {
            self.page_cache_sender.send_retrieve_command(
                cache::RetrievePagesCommand::GetCurrentPage {
                    page_number: self.current_page_number,
                },
            )
        } else {
            self.page_cache_sender.send_retrieve_command(
                cache::RetrievePagesCommand::GetCurrentTwoPages {
                    page_left_number: self.current_page_number,
                },
            )
        }
    }

    pub fn is_left_page(&self, page_number: PageNumber) -> bool {
        page_number == self.current_page_number
    }
    pub fn is_right_page(&self, page_number: PageNumber) -> bool {
        page_number == self.current_page_number + 1
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

fn process_right_click(ui: &mut Ui, _x: f64, _y: f64) {
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

    let center = ui.app_wrapper.width() / 2;
    if y < (ui.app_wrapper.height() / 5) as f64 {
        toggle_fullscreen(ui);
    } else if x > center as f64 {
        if x < ui.app_wrapper.width() as f64 * 0.75 {
            ui.document_canvas.as_mut().unwrap().increase_page_number();
        } else {
            ui.document_canvas.as_mut().unwrap().increase_page_number();
            ui.document_canvas.as_mut().unwrap().increase_page_number();
        }
    } else if x < center as f64 {
        if x > ui.app_wrapper.width() as f64 * 0.25 {
            ui.document_canvas.as_mut().unwrap().decrease_page_number();
        } else {
            ui.document_canvas.as_mut().unwrap().decrease_page_number();
            ui.document_canvas.as_mut().unwrap().decrease_page_number();
        }
    }
    update_page_status(ui);
}

impl Ui {
    pub fn build(app: &Application) -> Rc<RefCell<Ui>> {
        debug!("building ui");
        let open_file_button = Button::from_icon_name("document-open");

        let app_wrapper = Box::builder().orientation(Orientation::Vertical).build();
        let window = ApplicationWindow::builder()
            .application(app)
            .title("Music Reader")
            .child(&app_wrapper)
            .maximized(true)
            .width_request(600)
            .height_request(400)
            .build();

        let image_container = Box::builder()
            .spacing(0)
            // .width_request(600)
            // .height_request(300)
            .vexpand(true)
            .hexpand(true)
            .halign(gtk::Align::Center)
            .build();
        let image_left = Picture::builder()
            // .width_request(300)
            // .height_request(300)
            .vexpand(true)
            // .hexpand(true)
            .build();
        let image_right = Picture::builder()
            // .width_request(300)
            // .height_request(300)
            .vexpand(true)
            // .hexpand(true)
            .build();
        image_container.append(&image_left);
        image_container.append(&image_right);

        let ui = Ui {
            window,
            app_wrapper,
            bottom_bar: Box::builder().hexpand_set(true).build(),
            header_bar: HeaderBar::builder().build(),
            page_indicator: Label::builder().build(),
            image_container,
            image_left,
            image_right,
            document_canvas: None,
        };
        let ui = Rc::new(RefCell::new(ui));

        ui.borrow().header_bar.pack_start(&open_file_button);
        ui.borrow()
            .app_wrapper
            .prepend(&ui.borrow().image_container);
        ui.borrow().app_wrapper.append(&ui.borrow().bottom_bar);
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

        ui.borrow().app_wrapper.add_controller(click_left);
        ui.borrow().app_wrapper.add_controller(click_right);

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
    debug!("Loading file...");
    // TODO: catch errors, maybe show error dialog
    let path: PathBuf = file.as_ref().to_path_buf();
    let uri = format!("file://{}", path.to_str().unwrap());
    let document = poppler::Document::from_file(&uri, None).unwrap();
    let num_pages = document.n_pages() as usize;

    let sender = cache::spawn_sync_cache(
        document,
        clone!(@weak ui => move |cache_response| match cache_response {
                cache::CacheResponse::SinglePageRetrieved { page } => {
                    ui.borrow_mut().image_left.set_paintable(Some(page.as_ref()));
                    ui.borrow_mut().image_right.set_visible(false);
                    let area_height = ui.borrow().image_container.height();
                    ui.borrow().document_canvas.as_ref().unwrap().cache_surrounding_pages(area_height);
                }
                cache::CacheResponse::TwoPagesRetrieved {
                    page_left,
                    page_right,
                } => {
                    ui.borrow_mut().image_left.set_paintable(Some(page_left.as_ref()));
                    ui.borrow_mut().image_right.set_paintable(Some(page_right.as_ref()));
                    ui.borrow_mut().image_right.set_visible(true);
                    let area_height = ui.borrow().image_container.height();
                    ui.borrow().document_canvas.as_ref().unwrap().cache_surrounding_pages(area_height);
                },
            cache::CacheResponse::PageResolutionUpgraded { page_number, page } => {
                if ui.borrow().document_canvas.as_ref().unwrap().is_left_page(page_number){
                    ui.borrow_mut().image_left.set_paintable(Some(page.as_ref()));
                } else if ui.borrow().document_canvas.as_ref().unwrap().is_right_page(page_number){
                    ui.borrow_mut().image_right.set_paintable(Some(page.as_ref()));
                }
            }
        }),
    );

    let mut document_canvas = DocumentCanvas::new(sender);
    document_canvas.num_pages = Some(num_pages);
    document_canvas.cache_initial_pages(ui.borrow().image_container.height());

    ui.borrow_mut().document_canvas = Some(document_canvas);

    update_page_status(&ui.borrow());
    debug!("finished loading document");
}
