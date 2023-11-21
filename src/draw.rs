use std::{rc::Rc, time::Instant};

use cairo::Context;
use poppler::Page;

use crate::ui::DocumentCanvas;

pub fn draw(
    document_canvas: &Option<DocumentCanvas>,
    context: &Context,
    area_width: i32,
    area_height: i32,
) {
    println!("Draw");
    if let Some(document_canvas) = document_canvas {
        let begin_of_drawing = Instant::now();
        if document_canvas.num_pages.unwrap_or(0) > 1 {
            let mut pages = Vec::new();
            if let Some(page_left) = &document_canvas.left_page {
                pages.push(Rc::clone(page_left));
            }
            if let Some(page_right) = &document_canvas.right_page {
                pages.push(Rc::clone(page_right));
            }
            draw_pages(&pages, context, area_width, area_height);
        }

        println!(
            "Finished drawing in {}ms",
            begin_of_drawing.elapsed().as_millis()
        );
        document_canvas.cache_surrounding_pages();
    }
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
    let height_to_scale_to = f64::min(area_width / total_width_normalized, area_height);
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
