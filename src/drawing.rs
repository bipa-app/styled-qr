use image::GenericImage;
use imageproc::{
    drawing::{Canvas, draw_filled_circle_mut, draw_filled_rect_mut},
    rect::Rect,
};
use std::num::TryFromIntError;

// The eye artwork is a 308px square and the four measurements below are in its units, so each
// one is an exact integer fraction of the width the eye is drawn at.
const ARTWORK_WIDTH: u64 = 308;
const CORNER_RADIUS: u64 = 60;
const MIDDLE_RECT_WIDTH: u64 = 228;
const INNER_RECT_WIDTH: u64 = 138;

fn artwork_fraction(drawn_width: u32, artwork_part: u64) -> u64 {
    u64::from(drawn_width) * artwork_part / ARTWORK_WIDTH
}

pub fn draw_filled_rounded_rect<I>(
    img: &mut I,
    rect: Rect,
    corner_radius: i32,
    color: <I as Canvas>::Pixel,
) -> Result<(), TryFromIntError>
where
    I: GenericImage,
    <I as Canvas>::Pixel: 'static,
{
    let corner_size = u32::try_from(corner_radius)?;
    let double_corner_radius = corner_size * 2;

    draw_filled_circle_mut(
        img,
        (rect.left() + corner_radius, rect.top() + corner_radius),
        corner_radius,
        color,
    );

    draw_filled_circle_mut(
        img,
        (
            rect.left() + corner_radius,
            rect.bottom() - corner_radius - 1,
        ),
        corner_radius,
        color,
    );

    draw_filled_circle_mut(
        img,
        (
            rect.right() - corner_radius - 1,
            rect.bottom() - corner_radius - 1,
        ),
        corner_radius,
        color,
    );

    draw_filled_circle_mut(
        img,
        (rect.right() - corner_radius - 1, rect.top() + corner_radius),
        corner_radius,
        color,
    );

    if rect.width() > double_corner_radius {
        draw_filled_rect_mut(
            img,
            Rect::at(rect.left() + corner_radius, rect.top())
                .of_size(rect.width() - double_corner_radius, rect.height()),
            color,
        );

        draw_filled_rect_mut(
            img,
            Rect::at(rect.left(), rect.top() + corner_radius)
                .of_size(corner_size, rect.height() - double_corner_radius),
            color,
        );

        draw_filled_rect_mut(
            img,
            Rect::at(rect.right() - corner_radius, rect.top() + corner_radius)
                .of_size(corner_size, rect.height() - double_corner_radius),
            color,
        );
    }

    Ok(())
}

pub fn draw_qrcode_eye<I>(
    img: &mut I,
    (x, y): (i32, i32),
    (w, h): (u32, u32),
    dark_color: <I as Canvas>::Pixel,
    light_color: <I as Canvas>::Pixel,
) -> Result<(), TryFromIntError>
where
    I: GenericImage,
    <I as Canvas>::Pixel: 'static,
{
    let outer_rect_corner_radius = i32::try_from(artwork_fraction(w, CORNER_RADIUS))?;

    let middle_rect_width = u32::try_from(artwork_fraction(w, MIDDLE_RECT_WIDTH))?;
    let middle_rect_height = u32::try_from(artwork_fraction(h, MIDDLE_RECT_WIDTH))?;
    let middle_rect_corner_radius =
        i32::try_from(artwork_fraction(middle_rect_width, CORNER_RADIUS))?;

    let inner_rect_width = u32::try_from(artwork_fraction(w, INNER_RECT_WIDTH))?;
    let inner_rect_height = u32::try_from(artwork_fraction(h, INNER_RECT_WIDTH))?;
    let inner_rect_corner_radius =
        i32::try_from(artwork_fraction(inner_rect_width, CORNER_RADIUS))?;

    draw_filled_rounded_rect(
        img,
        Rect::at(x, y).of_size(w, h),
        outer_rect_corner_radius,
        dark_color,
    )?;

    draw_filled_rounded_rect(
        img,
        Rect::at(
            x + i32::try_from(w - middle_rect_width)? / 2,
            y + i32::try_from(h - middle_rect_height)? / 2,
        )
        .of_size(middle_rect_width, middle_rect_height),
        middle_rect_corner_radius,
        light_color,
    )?;

    draw_filled_rounded_rect(
        img,
        Rect::at(
            x + i32::try_from(w - inner_rect_width)? / 2,
            y + i32::try_from(h - inner_rect_height)? / 2,
        )
        .of_size(inner_rect_width, inner_rect_height),
        inner_rect_corner_radius,
        dark_color,
    )?;

    Ok(())
}
