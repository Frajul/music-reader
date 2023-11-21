use cairo::Context;

use crate::ui::DocumentCanvas;

pub fn draw(
    document_canvas: &Option<DocumentCanvas>,
    context: &Context,
    area_width: f64,
    area_height: f64,
) {
    println!("Draw");
    if let Some(document_canvas) = document_canvas {
        if document_canvas.num_pages.unwrap_or(0) > 1 {
            draw_two_pages(document_canvas, context, area_width, area_height);
        } else {
            draw_single_page(document_canvas, context, area_width, area_height);
        }

        println!("Finished drawing");
        document_canvas.cache_surrounding_pages();
    }
}

fn draw_two_pages(
    document_canvas: &DocumentCanvas,
    context: &Context,
    area_width: f64,
    area_height: f64,
) {
    let page_left = document_canvas.left_page.as_ref();
    let page_right = document_canvas.right_page.as_ref();

    if page_left.is_none() || page_right.is_none() {
        // TODO: show error message
        return;
    }

    let page_left = page_left.unwrap();
    let page_right = page_right.unwrap();

    let (w_left, h_left) = page_left.size();
    let (w_right, h_right) = page_right.size();

    let h_max = f64::max(h_left, h_right);
    // Make sure both pages are rendered with the same height
    let w_max = match h_left < h_right {
        true => w_left * h_right / h_left + w_right,
        false => w_left + w_right * h_left / h_right,
    };

    let h_scale = area_height / h_max;
    let w_scale = area_width / w_max;
    let scale = f64::min(h_scale, w_scale);
    let h_page = h_max * scale;

    let scale_left = h_page / h_left;
    let scale_right = h_page / h_right;

    context.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    context.save().unwrap();
    context.translate(
        area_width / 2.0 - w_left * scale_left,
        area_height / 2.0 - h_page / 2.0,
    );
    // Poppler sometimes crops white border, draw it manually
    context.rectangle(0.0, 0.0, w_left * scale_left, h_page);
    context.fill().unwrap();
    context.scale(scale_left, scale_left);
    page_left.render(context);

    context.restore().unwrap();
    context.translate(area_width / 2.0, area_height / 2.0 - h_page / 2.0);

    // Poppler sometimes crops white border, draw it manually
    context.rectangle(0.0, 0.0, w_right * scale_right, h_page);
    context.fill().unwrap();
    context.scale(scale_right, scale_right);
    page_right.render(context);
}

fn draw_single_page(
    document_canvas: &DocumentCanvas,
    context: &Context,
    area_width: f64,
    area_height: f64,
) {
    if document_canvas.left_page.is_none() {
        // TODO: show error message
        return;
    }

    let page = document_canvas.left_page.as_ref().unwrap();

    let (w, h) = page.size();

    let width_diff = area_width / w;
    let height_diff = area_height / h;
    if width_diff > height_diff {
        context.translate(
            (area_width - w * height_diff) / 2.0,
            (area_height - h * height_diff) / 2.0,
        );
        context.scale(height_diff, height_diff);
    } else {
        context.translate(
            (area_width - w * width_diff) / 2.0,
            (area_height - h * width_diff) / 2.0,
        );
        context.scale(width_diff, width_diff);
    }

    // Poppler sometimes crops white border, draw it manually
    context.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    context.rectangle(0.0, 0.0, w, h);
    context.fill().unwrap();

    page.render(context);
}
