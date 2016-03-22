use sdl2::render::Renderer;
use sdl2::pixels::Color;
use sdl2::rect::Point;

pub struct GPU {
    pub vram: [u8; 0x2000], // Video RAM
    pub oam: [u8; 0x100], // Sprite Attrib Memory
    //pub lcd_control: u8,
    pub switchbg: bool,
    pub bg_map: bool,
    pub bg_tile: bool,
    pub lcd_on: bool,
    pub scanline: u8,
    pub scroll_x: u8,
    pub scroll_y: u8,
    pub win_x: u8,
    pub win_y: u8,
    gpu_mode: u8,
    gpu_ticks: u32,
    prev_ticks: u32,
    palette_b: [u8; 4],
    s_palette0: [u8; 4],
    s_palette1: [u8; 4],
    pixel_buffer: [u8; 160 * 144],
    tiles: [[[u8; 8]; 8]; 384],
    renderer: Renderer<'static>,
}

impl GPU {
    pub fn new(render: Renderer<'static>) -> GPU {
        GPU {
            vram: [0; 0x2000],
            oam: [0; 0x100],
            //lcd_control: 0,
            switchbg: false,
            bg_map: false,
            bg_tile: false,
            lcd_on: false,
            scanline: 0,
            scroll_x: 0,
            scroll_y: 0,
            win_x: 0,
            win_y: 0,
            gpu_mode: 0,
            gpu_ticks: 0,
            prev_ticks: 0,
            palette_b: [0; 4],
            s_palette0: [0; 4],
            s_palette1: [0; 4],
            pixel_buffer: [0; 160 * 144],
            tiles: [[[0u8; 8]; 8]; 384],
            renderer: render,
        }
    }

    pub fn render_scanline(&mut self) {
        let mut map_offset = (if self.bg_map { 0x1c00 } else { 0x1800 });
        map_offset += (((self.scanline + self.scroll_y) & 255) >> 3);

        let mut line_offset = (self.scroll_x >> 3);

        let mut x = self.scroll_x & 7;
        let y = (self.scanline + self.scroll_y) & 7;
 
        let mut pixel_offset = self.scanline as u32 * 160;

        let mut tile: u32 = self.vram[(map_offset + line_offset) as usize] as u32;
        tile += (if self.bg_tile && tile < 128 { 256 } else { 0 });

        for i in 0..160 {
            let color = self.tiles[tile as usize][x as usize][y as usize];
            self.pixel_buffer[pixel_offset as usize] = self.palette_b[color as usize];
            pixel_offset += 1;

            x += 1;
            if x == 8 {
                x = 0;
                line_offset = (line_offset + 1) & 31;
                tile = self.vram[(map_offset + line_offset) as usize] as u32;
                tile += (if self.bg_tile && tile < 128 { 256 } else { 0 });
            }
        }

        
        // for i in 0..(144 / 8) * (160 / 8) {
        //     for y in 0..8 {
        //         for x in 0..8 {
        //             let color = self.tiles[i as usize][x as usize][y as usize];
        //             self.pixel_buffer[((i * 8 % 160) + x + (y + i * 8 / 160 * 8) * 160) as usize] = self.palette_b[color as usize];
        //         }
        //     }
        // } 
        self.draw_framebuffer();
    }

    pub fn draw_framebuffer(&mut self) {
        self.renderer.set_draw_color(Color::RGB(0, 0, 0));
        self.renderer.clear();
        for y in 0..144 {
            for x in 0..160 {
                let color = self.pixel_buffer[(x + (y * 160)) as usize];
                self.renderer.set_draw_color(Color::RGB(color, color, color));
                self.renderer.draw_point(Point::new(x, y));
            }
        }
        self.renderer.present();
    }

    // http://imrannazar.com/GameBoy-Emulation-in-JavaScript:-Graphics
    pub fn update_tile(&mut self, address: u16, value: u8) {
        let addr = (address & 0x1ffe);

        let tile = (addr >> 4) & 511;
        let y = (addr >> 1) & 7;

        let mut bit = 0 as u8;

        for x in 0..8 {
            bit = (1 << (7 - x as u8));
            self.tiles[tile as usize][x as usize][y as usize] = ((if (self.vram[addr as usize] & bit) != 0 { 1 } else { 0 }) + 
                                        (if (self.vram[(addr + 1) as usize] & bit) != 0 { 2 } else { 0 }));
        }
    }

    pub fn u_palette_b(&mut self, value: u8) {
        for i in 0..4 {
            self.palette_b[i] = self.get_color(value, i);
        }
    }

    pub fn u_s_palette0(&mut self, value: u8) {
        for i in 0..4 {
            self.s_palette0[i] = self.get_color(value, i);
        }
    }

    pub fn u_s_palette1(&mut self, value: u8) {
        for i in 0..4 {
            self.s_palette1[i] = self.get_color(value, i);
        }
    }

    fn get_color(&mut self, value: u8, i: usize) -> u8 {
        match (value >> (i * 2)) & 0x03 {
            0 => 255,
            1 => 192,
            2 => 96,
            _ => 0
        }
    }

    // http://imrannazar.com/GameBoy-Emulation-in-JavaScript:-GPU-Timings
    // http://www.codeslinger.co.uk/pages/projects/gameboy/lcd.html
    pub fn gpu_cycle(&mut self, cputicks: u32, flags: u8, enable: u8) -> bool {
        self.gpu_ticks += cputicks - self.prev_ticks;
        
        self.prev_ticks = cputicks;

        let mut flagupdate = false;

        match self.gpu_mode {
            0 => { 
                if self.gpu_ticks >= 204 {
                    self.scanline += 1;
                    if self.scanline == 143 {
                        if (enable & flags) != 0 { flagupdate = true; }
                        self.gpu_mode = 1;
                    }
                    else {
                        self.gpu_mode = 2;
                    }
                    self.gpu_ticks -= 204;
                }
            }
            1 => {
                if self.gpu_ticks >= 456 {
                    self.scanline += 1;
                    if self.scanline > 153 {
                        self.scanline = 0;
                        self.gpu_mode = 2;
                    }
                    self.gpu_ticks -= 456;
                }
            }
            2 => {
                if self.gpu_ticks >= 80 {
                    self.gpu_mode = 3;
                    self.gpu_ticks -= 80;
                }
            }
            3 => {
                if self.gpu_ticks >= 172 {
                    self.gpu_mode = 0;
                    //self.render_scanline();
                    self.gpu_ticks -= 172;
                }
            }
            _ => { panic!("Unknown gpu mode!") }
        }
        flagupdate
    }
}