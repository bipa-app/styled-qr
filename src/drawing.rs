use image::GenericImage;
use imageproc::{
    drawing::{draw_filled_circle_mut, draw_filled_rect_mut, Canvas},
    rect::Rect,
};

pub fn draw_filled_rounded_rect<I>(
    img: &mut I,
    rect: Rect,
    corner_radius: i32,
    color: <I as Canvas>::Pixel,
) where
    I: GenericImage,
    <I as Canvas>::Pixel: 'static,
{
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

    draw_filled_rect_mut(
        img,
        Rect::at(rect.left() + corner_radius, rect.top())
            .of_size(rect.width() - ((corner_radius * 2) as u32), rect.height()),
        color,
    );

    draw_filled_rect_mut(
        img,
        Rect::at(rect.left(), rect.top() + corner_radius).of_size(
            corner_radius as u32,
            rect.height() - ((corner_radius * 2) as u32),
        ),
        color,
    );

    draw_filled_rect_mut(
        img,
        Rect::at(rect.right() - corner_radius, rect.top() + corner_radius).of_size(
            corner_radius as u32,
            rect.height() - ((corner_radius * 2) as u32),
        ),
        color,
    );
}

pub fn draw_qrcode_eye<I>(
    img: &mut I,
    (x, y): (i32, i32),
    (w, h): (u32, u32),
    dark_color: <I as Canvas>::Pixel,
    light_color: <I as Canvas>::Pixel,
) where
    I: GenericImage,
    <I as Canvas>::Pixel: 'static,
{
    draw_filled_rounded_rect(img, Rect::at(x, y).of_size(w, h), 60, dark_color);

    draw_filled_rounded_rect(
        img,
        Rect::at(x + 40, y + 40).of_size(w - 80, h - 80),
        40,
        light_color,
    );

    draw_filled_rounded_rect(
        img,
        Rect::at(x + 85, y + 85).of_size(w - 170, h - 170),
        10,
        dark_color,
    );
}
