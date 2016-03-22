pub mod cartridge;
pub mod memory;
pub mod cpu;
pub mod registers;
pub mod gpu;

extern crate sdl2;

use sdl2::pixels::Color;

const MAX_CYCLES: u16 = 4194304;

fn main() {
	let sdl_context = sdl2::init().unwrap();
    let video = sdl_context.video().unwrap();

    let window = video.window("rustyboy", 160, 144)
        .position_centered().opengl()
        .build().unwrap();

    let mut renderer = window.renderer()
        .accelerated()
        .build().unwrap();

    renderer.set_draw_color(Color::RGB(0, 0, 0));
    renderer.clear();

    let mut cpu = cpu::CPU::new(renderer);
    cpu.initialize("t.gb");
    let mut b = true;
    while b {
        cpu.cpu_cycle();
    }
    
}