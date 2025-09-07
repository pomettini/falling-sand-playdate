use pd::sys::ffi::{LCD_COLUMNS, LCD_ROWS};

pub const ROWS: usize = LCD_ROWS as usize;
pub const COLUMNS: usize = 2 + (LCD_COLUMNS / 8) as usize;
pub const PIXEL_WIDTH: usize = LCD_COLUMNS as usize;
pub const BUFFER_SIZE: usize = COLUMNS * ROWS;

pub const SAND_BRUSH_SIZE: usize = 5;
