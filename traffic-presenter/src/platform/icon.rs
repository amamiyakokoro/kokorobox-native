use tray_icon::Icon;

pub fn status_icon() -> Result<Icon, tray_icon::BadIcon> {
    const SIZE: u32 = 18;
    let mut rgba = vec![0_u8; (SIZE * SIZE * 4) as usize];

    for y in 1..SIZE - 1 {
        for x in 1..SIZE - 1 {
            set_pixel(&mut rgba, SIZE, x, y, [30, 122, 238, 255]);
        }
    }

    for offset in 0..5 {
        set_pixel(&mut rgba, SIZE, 5 - offset, 7 + offset, [255; 4]);
        set_pixel(&mut rgba, SIZE, 5 + offset, 7 + offset, [255; 4]);
        set_pixel(&mut rgba, SIZE, 5, 7 + offset, [255; 4]);

        set_pixel(&mut rgba, SIZE, 12 - offset, 10 - offset, [255; 4]);
        set_pixel(&mut rgba, SIZE, 12 + offset, 10 - offset, [255; 4]);
        set_pixel(&mut rgba, SIZE, 12, 6 + offset, [255; 4]);
    }

    Icon::from_rgba(rgba, SIZE, SIZE)
}

fn set_pixel(rgba: &mut [u8], size: u32, x: u32, y: u32, color: [u8; 4]) {
    if x >= size || y >= size {
        return;
    }
    let index = ((y * size + x) * 4) as usize;
    rgba[index..index + 4].copy_from_slice(&color);
}
