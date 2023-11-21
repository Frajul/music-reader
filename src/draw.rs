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
    ui::Ui,
};
use glib::clone;
use gtk::prelude::*;

pub fn draw(ui: &mut Ui, area: &DrawingArea, context: &Context) {
    println!("Draw");
    if ui.document_canvas.is_none() {
        return;
    }
    let document_canvas = ui.document_canvas.as_ref().unwrap();

    // let left_page = document_canvas.left_page.as_ref().unwrap();
    // let left_page = left_page.as_ref();

    // let data: Vec<u8> = left_page.into_iter().map(|x| x.to_owned()).collect();

    // let data: Vec<u8> = page.iter().map(|x| x.clone()).collect();
    // let surface = ImageSurface::create_for_data(data, Format::Rgb24, 0, 0, 0).unwrap();

    // context.set_source_surface(surface, 0.0, 0.0);
    // context.paint();

    if document_canvas.num_pages.unwrap_or(0) > 1 {
        draw_two_pages(ui, area, context);
    } else {
        draw_single_page(ui, area, context);
    }

    // gio::spawn_blocking(move || {
    //     ui.document_canvas
    //         .as_mut()
    //         .unwrap()
    //         .cache_surrounding_pages();
    // });
    println!("Finished drawing");
    document_canvas.cache_surrounding_pages();
}

fn draw_two_pages(ui: &Ui, area: &DrawingArea, context: &Context) {
    if ui.document_canvas.is_none() {
        return;
    }
    let document_canvas = ui.document_canvas.as_ref().unwrap();

    let page_left = document_canvas.left_page.as_ref();
    let page_right = document_canvas.right_page.as_ref();

    if page_left.is_none() || page_right.is_none() {
        // TODO: show error message
        return;
    }

    let page_left = page_left.unwrap();
    let page_right = page_right.unwrap();

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

    if document_canvas.left_page.is_none() {
        // TODO: show error message
        return;
    }

    let page = document_canvas.left_page.as_ref().unwrap();
    // let page = ImageSurface::create_for_data(page.into(), Format::Rgb24, 0, 0, 0).unwrap();

    // context.set_source_surface(page, 0, 0);
    // Draw background
    // context.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    // context.paint().unwrap();
    // context.fill().expect("uh oh");
    // context.paint().unwrap();

    let (w, h) = page.size();
    // let w = page.width() as f64;
    // let h = page.height() as f64;

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
