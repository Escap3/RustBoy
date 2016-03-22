use gpu::GPU;
use cpu::IFlags::{ VBLANK, LCDCSTATUS, TIMEROVERFLOW, SERIALTC, KEYPAD };

use std::io;
use sdl2::render::Renderer;
use sdl2::pixels::Color;
use sdl2::rect::Point;

// https://realboyemulator.files.wordpress.com/2013/01/gbcpuman.pdf Page 8
pub struct Memory {
    cart: [u8; 0x8000], // Cartridge   
    sram: [u8; 0x2000], // Switchable RAM bank
    iram: [u8; 0x2000], // Internal RAM
    eram: [u8; 0x2000], // Echo of Internal RAM
    io:   [u8; 0x100], // IO
    hram: [u8; 0x80], // Internal RAM 
    pub master: bool,
    pub enable: u8,
    pub flags: u8,
    pub gpu: GPU,
}

impl Memory {
    pub fn new(rend: Renderer<'static>) -> Memory {
         Memory {
            cart:   [0; 0x8000], 
            sram:   [0; 0x2000],
            iram:   [0; 0x2000],
            eram:   [0; 0x2000],
            io:     [0; 0x100],   // https://realboyemulator.files.wordpress.com/2013/01/gbcpuman.pdf Page 35 Special IO Registers
            hram:   [0; 0x80],           
            master: false,
            enable: 0,
            flags: 0,
            gpu: GPU::new(rend),           
        }      
    }

    pub fn gpu_cycle(&mut self, cputicks: u32) {
        if self.gpu.gpu_cycle(cputicks, self.flags, self.enable) {
            self.flags |= VBLANK as u8;
        }
    }

    // https://realboyemulator.files.wordpress.com/2013/01/gbcpuman.pdf Page 18
    pub fn put_initial(&mut self) { 
        self.write_byte(0xff05, 0);
        self.write_byte(0xff06, 0);
        self.write_byte(0xff07, 0);
        self.write_byte(0xff10, 0x80);
        self.write_byte(0xff11, 0xbf);
        self.write_byte(0xff12, 0xf3);
        self.write_byte(0xff14, 0xbf);
        self.write_byte(0xff16, 0x3f);
        self.write_byte(0xff17, 0);
        self.write_byte(0xff19, 0xbf);
        self.write_byte(0xff1a, 0x7f);
        self.write_byte(0xff1b, 0xff);
        self.write_byte(0xff1c, 0x9f);
        self.write_byte(0xff1e, 0xbf);
        self.write_byte(0xff20, 0xff);
        self.write_byte(0xff21, 0);
        self.write_byte(0xff22, 0);
        self.write_byte(0xff23, 0xbf);
        self.write_byte(0xff24, 0x77);
        self.write_byte(0xff25, 0xf3);
        self.write_byte(0xff26, 0xf1);
        self.write_byte(0xff40, 0x91);
        self.write_byte(0xff42, 0);
        self.write_byte(0xff43, 0);
        self.write_byte(0xff45, 0);
        self.write_byte(0xff47, 0xfc);
        self.write_byte(0xff48, 0xff);
        self.write_byte(0xff49, 0xff);
        self.write_byte(0xff4a, 0);
        self.write_byte(0xff4b, 0);
        self.write_byte(0xffff, 0);      
    }
    
    pub fn read_byte(&mut self, address: u16) -> u8 {
        match address {
            0x0000 ... 0x7fff => { self.cart[address as usize] }
            0x8000 ... 0x9fff => { self.gpu.vram[address as usize - 0x8000] }
            0xa000 ... 0xbfff => { self.sram[address as usize - 0xa000] }
            0xc000 ... 0xdfff => { self.iram[address as usize - 0xc000] }
            0xe000 ... 0xfdff => { self.eram[address as usize - 0xe000] }
            0xfe00 ... 0xfeff => { self.gpu.oam[address as usize - 0xfe00] }
            0xff00 => { 0 }
            0xff04 => { 1 }
            //0xff40 => { self.gpu.lcd_control }
            0xff40 => { (if self.gpu.switchbg { 0x01 } else { 0x0 }) |
                        (if self.gpu.bg_map   { 0x08 } else { 0x0 }) |
                        (if self.gpu.bg_tile  { 0x10 } else { 0x0 }) |
                        (if self.gpu.lcd_on   { 0x80 } else { 0x0 })
                      }
            0xff42 => { self.gpu.scroll_y }
            0xff43 => { self.gpu.scroll_x }
            0xff44 => { self.gpu.scanline }
            0xff4a => { self.gpu.win_y }
            0xff4b => { self.gpu.win_x }
            0xff0f => { self.flags }
            0xff00 ... 0xff7f => { self.io[address as usize - 0xff00] }
            0xff80 ... 0xfffe => { self.hram[address as usize - 0xff80] }   
            0xffff => { self.enable }   
            _ => 1
        }
    }

    pub fn write_byte(&mut self, address: u16, value: u8) {
        match address {
            0x0000 ... 0x7fff => { self.cart[address as usize] = value; }
            0x8000 ... 0x9fff => { self.gpu.vram[address as usize - 0x8000] = value;
                                   if address < 0x97ff { self.gpu.update_tile(address, value); }
                                 }
            0xa000 ... 0xbfff => { self.sram[address as usize - 0xa000] = value; }
            0xc000 ... 0xdfff => { self.iram[address as usize - 0xc000] = value; }
            0xe000 ... 0xfdff => { self.eram[address as usize - 0xe000] = value; }
            0xfe00 ... 0xfeff => { self.gpu.oam[address as usize - 0xfe00] = value; }
            //0xff40 => { self.gpu.lcd_control = value; }
            0xff40 => { self.gpu.switchbg = (if (value & 0x01) != 0 { true } else { false });
                        self.gpu.bg_map   = (if (value & 0x08) != 0 { true } else { false });
                        self.gpu.bg_tile  = (if (value & 0x10) != 0 { true } else { false });
                        self.gpu.lcd_on   = (if (value & 0x80) != 0 { true } else { false });
                      }
            0xff42 => { self.gpu.scroll_y = value; }
            0xff43 => { self.gpu.scroll_x = value; }
            0xff46 => { self.oam_to_ram(value); }
            0xff47 => { self.gpu.u_palette_b(value); }
            0xff48 => { self.gpu.u_s_palette0(value); }
            0xff49 => { self.gpu.u_s_palette1(value); }
            0xff4a => { self.gpu.win_y = value; }
            0xff4b => { self.gpu.win_x = value; }
            0xff0f => { self.flags = value; }
            0xff00 ... 0xff7f => { self.io[address as usize - 0xff00] = value }
            0xff80 ... 0xfffe => { self.hram[address as usize - 0xff80] = value }
            //0xffff => { self.enable = value; }
            _ => {  }
        }
    }

    fn oam_to_ram(&mut self, value: u8) {
        let v = (value as u16) << 8;
        for i in 0 .. 0xa0 {
            let b = self.read_byte(v + i);
            self.write_byte(0xfe00 + i, b);
        }
    }

    pub fn read_short(&mut self, address: u16) -> u16 {
        (self.read_byte(address) as u16 | ((self.read_byte(address + 1) as u16) << 8))
    }

    pub fn write_short(&mut self, address: u16, value: u16) {
        self.write_byte(address, (value & 0xff) as u8);   
        self.write_byte(address + 1, (value >> 8) as u8);
    }

    pub fn debug_memory(&mut self) {
        println!("{:?}", self.master);
        println!("{:X} IE", self.enable);
        println!("{:X} IF", self.flags);
    }
}