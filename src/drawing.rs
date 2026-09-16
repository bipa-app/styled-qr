use image::GenericImage;
use imageproc::{
    drawing::{Canvas, draw_filled_circle_mut, draw_filled_rect_mut},
    rect::Rect,
};
use std::num::TryFromIntError;

pub fn draw_filled_rounded_rect<I>(
    img: &mut I,
    rect: Rect,
    corner_radius: i32,
    color: <I as Canvas>::Pixel,
) where
    I: GenericImage,
    <I as Canvas>::Pixel: 'static,
{
    let double_corner_radius = (corner_radius * 2) as u32;

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
                .of_size(corner_radius as u32, rect.height() - double_corner_radius),
            color,
        );

        draw_filled_rect_mut(
            img,
            Rect::at(rect.right() - corner_radius, rect.top() + corner_radius)
                .of_size(corner_radius as u32, rect.height() - double_corner_radius),
            color,
        );
    }
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
    let corner_radius_r = 60f64 / 308f64;

    let outer_rect_corner_radius = (corner_radius_r * f64::from(w)) as i32;

    let middle_rect_r: f64 = 228f64 / 308f64;
    let middle_rect_width = (f64::from(w) * middle_rect_r) as u32;
    let middle_rect_height = (f64::from(h) * middle_rect_r) as u32;
    let middle_rect_corner_radius = (corner_radius_r * f64::from(middle_rect_width)) as i32;

    let inner_rect_r: f64 = 138f64 / 308f64;
    let inner_rect_width = (f64::from(w) * inner_rect_r) as u32;
    let inner_rect_height = (f64::from(h) * inner_rect_r) as u32;
    let inner_rect_corner_radius = (corner_radius_r * f64::from(inner_rect_width)) as i32;

    draw_filled_rounded_rect(
        img,
        Rect::at(x, y).of_size(w, h),
        outer_rect_corner_radius,
        dark_color,
    );

    draw_filled_rounded_rect(
        img,
        Rect::at(
            x + i32::try_from(w - middle_rect_width)? / 2,
            y + i32::try_from(h - middle_rect_height)? / 2,
        )
        .of_size(middle_rect_width, middle_rect_height),
        middle_rect_corner_radius,
        light_color,
    );

    draw_filled_rounded_rect(
        img,
        Rect::at(
            x + i32::try_from(w - inner_rect_width)? / 2,
            y + i32::try_from(h - inner_rect_height)? / 2,
        )
        .of_size(inner_rect_width, inner_rect_height),
        inner_rect_corner_radius,
        dark_color,
    );

    Ok(())
}
