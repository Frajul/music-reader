use std::rc::Rc;

use cairo::{Context, ImageSurface};
use glib::Bytes;
use gtk::gdk::Texture;
use poppler::Page;

pub fn draw_pages_to_texture(pages: &[Rc<Page>], area_height: i32) -> Texture {
    let area_height = i32::max(400, area_height);
    let total_width_normalized: f64 = pages
        .iter()
        .map(|page| page.size())
        .map(|(w, h)| w / h)
        .sum();
    let area_width = (total_width_normalized * area_height as f64 + 0.5) as i32;

    let surface = ImageSurface::create(cairo::Format::Rgb24, area_width, area_height).unwrap();
    let context = Context::new(&surface).unwrap();
    draw_pages(pages, &context, area_width, area_height);

    let mut stream: Vec<u8> = Vec::new();
    surface.write_to_png(&mut stream).unwrap();
    Texture::from_bytes(&Bytes::from(&stream)).unwrap()
}

fn draw_pages(pages: &[Rc<Page>], context: &Context, area_width: i32, area_height: i32) {
    if pages.is_empty() {
        return;
    }
    let area_width = area_width as f64;
    let area_height = area_height as f64;

    // Total width if height of every page was 1
    let total_width_normalized: f64 = pages
        .iter()
        .map(|page| page.size())
        .map(|(w, h)| w / h)
        .sum();
    // let height_to_scale_to = f64::min(area_width / total_width_normalized, area_height);
    let height_to_scale_to = area_height;
    let total_width = total_width_normalized * height_to_scale_to;

    context.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    context.translate(
        (area_width - total_width) / 2.0,
        (area_height - height_to_scale_to) / 2.0,
    );
    context.save().unwrap();

    for page in pages {
        let (page_width, page_height) = page.size();
        let scale = height_to_scale_to / page_height;
        let scaled_width = page_width * scale;

        println!(
            "drawing with size: {}, {}",
            scaled_width, height_to_scale_to
        );

        // context.translate(total_width_of_rendered_pages, 0.0);
        // Poppler sometimes crops white border, draw it manually
        context.rectangle(0.0, 0.0, scaled_width, height_to_scale_to);
        context.fill().unwrap();

        context.scale(scale, scale);
        page.render(context);

        context.restore().unwrap();
        context.translate(scaled_width, 0.0);
        context.save().unwrap();
    }
}
