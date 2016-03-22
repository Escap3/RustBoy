use registers::Registers;
use registers::Flags::{Z, N, H, C};
use memory::Memory;
use cartridge;

use sdl2::render::Renderer;

use std::io;
use std::io::prelude::*;
use std::fs::File;
use std::io::BufWriter;
use std::fs::OpenOptions;
use std::path;

// https://realboyemulator.files.wordpress.com/2013/01/gbcpuman.pdf Page 34
pub enum IFlags {
    VBLANK          = 0b00000001,
    LCDCSTATUS      = 0b00000010,
    TIMEROVERFLOW   = 0b00000100,
    SERIALTC        = 0b00001000,
    KEYPAD          = 0b00010000,
}

pub struct CPU {
    register: Registers,
    memory: Memory,
    ticks: u32,
    stopped: bool,
    halted: bool,
    debugging: bool,
}

#[allow(dead_code)]
impl CPU {
    pub fn new(rend: Renderer<'static>) -> CPU {
        CPU {
            register: Registers::new(),
            memory: Memory::new(rend),            
            ticks: 0,
            stopped: false,
            halted: false,
            debugging: false,
        }
    }

    pub fn initialize(&mut self, filename: &str) {
        match cartridge::load_rom(filename, &mut self.memory) {
            Ok(n) => println!("Rom loaded successfully!"),
            Err(err) => println!("Error: {:?}", err),
        }
        self.memory.put_initial();
    }

    pub fn cpu_cycle(&mut self) {
        if self.stopped { return; }
        self.ticks += self.execute() as u32;
        self.memory.gpu_cycle(self.ticks);
        self.interrupt_cycle();
    }

    pub fn interrupt_cycle(&mut self) {
        if self.memory.master && self.memory.enable != 0 && self.memory.flags != 0 {
            let trigger = self.memory.enable & self.memory.flags;

            if (trigger & IFlags::VBLANK as u8) != 0 {
                self.memory.flags &= !(IFlags::VBLANK as u8);
                self.vblank();
                self.memory.master = false;
            }  
            
            if (trigger & IFlags::LCDCSTATUS as u8) != 0 {
                self.memory.flags &= !(IFlags::LCDCSTATUS as u8);
                self.lcd_status();
                self.memory.master = false;
            }    
            
            if (trigger & IFlags::TIMEROVERFLOW as u8) != 0 {
                self.memory.flags &= !(IFlags::TIMEROVERFLOW as u8);
                self.timer_overflow();
                self.memory.master = false;
            }
            
            if (trigger & IFlags::SERIALTC as u8) != 0 {
                self.memory.flags &= !(IFlags::SERIALTC as u8);
                self.serial_transf_complete();
                self.memory.master = false;
            }     

            if (trigger & IFlags::KEYPAD as u8) != 0 {
                self.memory.flags &= !(IFlags::KEYPAD as u8);
                self.keypad();
                self.memory.master = false;
            }
        }
    }

    fn vblank(&mut self){
        self.memory.master = false;
        self.memory.gpu.draw_framebuffer();
        let pc = self.register.PC;
        self.push_stack(pc);
        self.register.PC = 0x40;
        self.ticks += 36;
    }

    fn lcd_status(&mut self) {
        self.memory.master = false;
        let pc = self.register.PC;
        self.push_stack(pc);
        self.register.PC = 0x48;
        self.ticks += 36;
    }

    fn timer_overflow(&mut self) {
        self.memory.master = false;
        let pc = self.register.PC;
        self.push_stack(pc);
        self.register.PC = 0x50;
        self.ticks += 36;
    }

    fn serial_transf_complete(&mut self) {
        self.memory.master = false;
        let pc = self.register.PC;
        self.push_stack(pc);
        self.register.PC = 0x58;
        self.ticks += 36;
    }

    fn keypad(&mut self) {
        self.memory.master = false;
        let pc = self.register.PC;
        self.push_stack(pc);
        self.register.PC = 0x60;
        self.ticks += 36;
    }

    fn getbyte(&mut self) -> u8 {
        let op = self.memory.read_byte(self.register.PC);
        self.register.PC += 1;
        op
    }

    fn getshort(&mut self) -> u16 {
        let op = self.memory.read_short(self.register.PC);
        self.register.PC += 2;
        op
    }

    fn push_stack(&mut self, value: u16) {
        self.register.SP -= 2; // Stack grows downwards
        self.memory.write_short(self.register.SP, value);
    }

    fn pop_stack(&mut self) -> u16 {
        let v = self.memory.read_short(self.register.SP);
        if self.debugging {
            println!("Read {:x} from stack", v);
        }
        self.register.SP += 2;
        v
    }

    fn execute(&mut self) -> u16 {
        

        if self.register.SP == 0xcff7 {
            self.register.debug_register();
            self.memory.debug_memory();
            let mut input = String::new();
            match io::stdin().read_line(&mut input) {
                Ok(n) => {  }
                Err(error) => println!("error: {}", error),
            }
            //self.debugging = true;
            self.memory.gpu.render_scanline();
        }

        let op = self.getbyte();
        //println!("{:X}", op); 
        if self.debugging {
            let mut input = String::new();
            match io::stdin().read_line(&mut input) {
                Ok(n) => {  }
                Err(error) => println!("error: {}", error),
            }
            println!("{:X}", op);      
            self.register.debug_register();
        }

        match op {
            0x00 => {                                   self.nop();         4 }
            0x01 => { let v = self.getshort();          self.ld_bc_nn(v);   12 }
            0x02 => {                                   self.ld_bc_a();     8 }
            0x03 => {                                   self.inc_bc();      8 }
            0x04 => {                                   self.inc_b();       4 }
            0x05 => {                                   self.dec_b();       4 }
            0x06 => { let v = self.getbyte();           self.ld_b_n(v);     8 }
            0x07 => {                                   self.rlca();        4 }
            0x08 => { let v = self.getshort();          self.ld_nn_sp(v);   20 }
            0x09 => { let v = self.register.get_bc();   self.add_hl_bc(v);  8 }
            0x0a => {                                   self.ld_a_bc();     8 }
            0x0b => {                                   self.dec_bc();      8 }
            0x0c => {                                   self.inc_c();       4 }
            0x0d => {                                   self.dec_c();       4 }
            0x0e => { let v = self.getbyte();           self.ld_c_n(v);     8 }
            0x0f => {                                   self.rrca();        4 }
            0x10 => {                                   self.stop();        4 }
            0x11 => { let v = self.getshort();          self.ld_de_nn(v);   12 }
            0x12 => {                                   self.ld_de_a();     8 }
            0x13 => {                                   self.inc_de();      8 }
            0x14 => {                                   self.inc_d();       4 }
            0x15 => {                                   self.dec_d();       4 }
            0x16 => { let v = self.getbyte();           self.ld_d_n(v);     8 }
            0x17 => {                                   self.rla();         4 }
            0x18 => { let v = self.getbyte() as i8;     self.jr_n(v);       8 }
            0x19 => { let v = self.register.get_de();   self.add_hl_de(v);  8 }
            0x1a => {                                   self.ld_a_de();     8 }
            0x1b => {                                   self.dec_de();      8 }
            0x1c => {                                   self.inc_e();       4 }
            0x1d => {                                   self.dec_e();       4 }
            0x1e => { let v = self.getbyte();           self.ld_e_n(v);     8 }
            0x1f => {                                   self.rra();         4 }
            0x20 => { let v = self.getbyte() as i8;     self.jr_nz_n(v);    8 }
            0x21 => { let v = self.getshort();          self.ld_hl_nn(v);   12 }
            0x22 => {                                   self.ldi_hl_a();    8 }
            0x23 => {                                   self.inc_hl();      8 }
            0x24 => {                                   self.inc_h();       4 }
            0x25 => {                                   self.dec_h();       4 }
            0x26 => { let v = self.getbyte();           self.ld_h_n(v);     8 }
            0x27 => {                                   self.daa();         4 }
            0x28 => { let v = self.getbyte() as i8;     self.jr_z_n(v);     8 }
            0x29 => { let v = self.register.get_hl();   self.add_hl_hl(v);  8 }
            0x2a => {                                   self.ldi_a_hl();    8 }
            0x2b => {                                   self.dec_hl();      8 }
            0x2c => {                                   self.inc_l();       4 }
            0x2d => {                                   self.dec_l();       4 }
            0x2e => { let v = self.getbyte();           self.ld_l_n(v);     8 }
            0x2f => {                                   self.cpl();         4 }
            0x30 => { let v = self.getbyte() as i8;     self.jr_nc_n(v);    8 }
            0x31 => { let v = self.getshort();          self.ld_sp_nn(v);   12 }
            0x32 => {                                   self.ldd_hl_a();    8 }
            0x33 => {                                   self.inc_sp();      8 }
            0x34 => {                                   self.inc_hl_ptr();  12 }
            0x35 => {                                   self.dec_hl_ptr();  12 }
            0x36 => { let v = self.getbyte();           self.ld_hl_n(v);    12 }
            0x37 => {                                   self.scf();         4 }
            0x38 => { let v = self.getbyte() as i8;     self.jr_c_n(v);     8 }
            0x39 => { let v = self.register.SP;         self.add_hl_sp(v);  8 }
            0x3a => {                                   self.ldd_a_hl();    8 }
            0x3b => {                                   self.dec_sp();      8 }
            0x3c => {                                   self.inc_a();       4 }
            0x3d => {                                   self.dec_a();       4 }
            0x3e => { let v = self.getbyte();           self.ld_a_n(v);     8 }
            0x3f => {                                   self.ccf();         4 }
            0x41 => {                                   self.ld_b_c();      4 }
            0x42 => {                                   self.ld_b_d();      4 }
            0x43 => {                                   self.ld_b_e();      4 }
            0x44 => {                                   self.ld_b_h();      4 }
            0x45 => {                                   self.ld_b_l();      4 }
            0x46 => {                                   self.ld_b_hl();     8 }
            0x47 => {                                   self.ld_b_a();      4 }
            0x48 => {                                   self.ld_c_b();      4 }
            0x4a => {                                   self.ld_c_d();      4 }
            0x4b => {                                   self.ld_c_e();      4 }
            0x4c => {                                   self.ld_c_h();      4 }
            0x4d => {                                   self.ld_c_l();      4 }
            0x4e => {                                   self.ld_c_hl();     8 }
            0x4f => {                                   self.ld_c_a();      4 }
            0x50 => {                                   self.ld_d_b();      4 }
            0x51 => {                                   self.ld_d_c();      4 }
            0x53 => {                                   self.ld_d_e();      4 }
            0x54 => {                                   self.ld_d_h();      4 }
            0x55 => {                                   self.ld_d_l();      4 }
            0x56 => {                                   self.ld_d_hl();     8 }
            0x57 => {                                   self.ld_d_a();      4 }
            0x58 => {                                   self.ld_e_b();      4 }
            0x59 => {                                   self.ld_e_c();      4 }
            0x5a => {                                   self.ld_e_d();      4 }
            0x5c => {                                   self.ld_e_h();      4 }
            0x5d => {                                   self.ld_e_l();      4 }
            0x5e => {                                   self.ld_e_hl();     8 }
            0x5f => {                                   self.ld_e_a();      4 }
            0x60 => {                                   self.ld_h_b();      4 }
            0x61 => {                                   self.ld_h_c();      4 }
            0x62 => {                                   self.ld_h_d();      4 }
            0x63 => {                                   self.ld_h_e();      4 }
            0x65 => {                                   self.ld_h_l();      4 }
            0x66 => {                                   self.ld_h_hl();     8 }
            0x67 => {                                   self.ld_h_a();      4 }
            0x68 => {                                   self.ld_l_b();      4 }
            0x69 => {                                   self.ld_l_c();      4 }
            0x6a => {                                   self.ld_l_d();      4 }
            0x6b => {                                   self.ld_l_e();      4 }
            0x6c => {                                   self.ld_l_h();      4 }
            0x6e => {                                   self.ld_l_hl();     8 }
            0x6f => {                                   self.ld_l_a();      4 }
            0x70 => {                                   self.ld_hl_b();     8 }
            0x71 => {                                   self.ld_hl_c();     8 }
            0x72 => {                                   self.ld_hl_d();     8 }
            0x73 => {                                   self.ld_hl_e();     8 }
            0x74 => {                                   self.ld_hl_h();     8 }
            0x75 => {                                   self.ld_hl_l();     8 }
            0x76 => {                                   self.halt();        4 }
            0x77 => {                                   self.ld_hl_a();     8 }
            0x78 => {                                   self.ld_a_b();      4 }
            0x79 => {                                   self.ld_a_c();      4 }
            0x7a => {                                   self.ld_a_d();      4 }
            0x7b => {                                   self.ld_a_e();      4 }
            0x7c => {                                   self.ld_a_h();      4 }
            0x7d => {                                   self.ld_a_l();      4 }
            0x7e => {                                   self.ld_a_hl();     8 }
            0x7f => {                                   self.ld_a_a();      4 }
            0x80 => {                                   self.add_a_b();     4 }
            0x81 => {                                   self.add_a_c();     4 }
            0x82 => {                                   self.add_a_d();     4 }
            0x83 => {                                   self.add_a_e();     4 }
            0x84 => {                                   self.add_a_h();     4 }
            0x85 => {                                   self.add_a_l();     4 }
            0x86 => {                                   self.add_a_hl();    8 }
            0x87 => {                                   self.add_a_a();     4 }
            0x88 => {                                   self.adc_a_b();     4 }
            0x89 => {                                   self.adc_a_c();     4 }
            0x8a => {                                   self.adc_a_d();     4 }
            0x8b => {                                   self.adc_a_e();     4 }
            0x8c => {                                   self.adc_a_h();     4 }
            0x8d => {                                   self.adc_a_l();     4 }
            0x8e => {                                   self.adc_a_hl();    8 }
            0x8f => {                                   self.adc_a_a();     4 }
            0x90 => {                                   self.sub_a_b();     4 }
            0x91 => {                                   self.sub_a_c();     4 }
            0x92 => {                                   self.sub_a_d();     4 }
            0x93 => {                                   self.sub_a_e();     4 }
            0x94 => {                                   self.sub_a_h();     4 }
            0x95 => {                                   self.sub_a_l();     4 }
            0x96 => {                                   self.sub_a_hl();    8 }
            0x97 => {                                   self.sub_a_a();     4 }
            0x98 => {                                   self.sbc_a_b();     4 }
            0x99 => {                                   self.sbc_a_c();     4 }
            0x9a => {                                   self.sbc_a_d();     4 }
            0x9b => {                                   self.sbc_a_e();     4 }
            0x9c => {                                   self.sbc_a_h();     4 }
            0x9d => {                                   self.sbc_a_l();     4 }
            0x9e => {                                   self.sbc_a_hl();    8 }
            0x9f => {                                   self.sbc_a_a();     4 }
            0xa0 => {                                   self.and_b();       4 }
            0xa1 => {                                   self.and_c();       4 }
            0xa2 => {                                   self.and_d();       4 }
            0xa3 => {                                   self.and_e();       4 }
            0xa4 => {                                   self.and_h();       4 }
            0xa5 => {                                   self.and_l();       4 }
            0xa6 => {                                   self.and_hl();      8 }
            0xa7 => {                                   self.and_a();       4 }
            0xa8 => {                                   self.xor_b();       4 }
            0xa9 => {                                   self.xor_c();       4 }
            0xaa => {                                   self.xor_d();       4 }
            0xab => {                                   self.xor_e();       4 }
            0xac => {                                   self.xor_h();       4 }
            0xad => {                                   self.xor_l();       4 }
            0xae => {                                   self.xor_hl();      8 }
            0xaf => {                                   self.xor_a();       4 }
            0xb0 => {                                   self.or_b();        4 }
            0xb1 => {                                   self.or_c();        4 }
            0xb2 => {                                   self.or_d();        4 }
            0xb3 => {                                   self.or_e();        4 }
            0xb4 => {                                   self.or_h();        4 }
            0xb5 => {                                   self.or_l();        4 }
            0xb6 => {                                   self.or_hl();       8 }
            0xb7 => {                                   self.or_a();        4 }
            0xb8 => {                                   self.cp_b();        4 }
            0xb9 => {                                   self.cp_c();        4 }
            0xba => {                                   self.cp_d();        4 }
            0xbb => {                                   self.cp_e();        4 }
            0xbc => {                                   self.cp_h();        4 }
            0xbd => {                                   self.cp_l();        4 }
            0xbe => {                                   self.cp_hl();       8 }
            0xbf => {                                   self.cp_a();        4 }
            0xc0 => {                                   self.ret_nz();      8 }
            0xc1 => {                                   self.pop_bc();      12 }
            0xc2 => { let v = self.getshort();          self.jp_nz_nn(v);   12 }
            0xc3 => { let v = self.getshort();          self.jp_nn(v);      12 }
            0xc4 => { let v = self.getshort();          self.call_nz_nn(v); 12 }
            0xc5 => {                                   self.push_bc();     16 }
            0xc6 => { let v = self.getbyte();           self.add_a_n(v);    8 }
            0xc7 => {                                   self.rst_0();       32 }
            0xc8 => {                                   self.ret_z();       8 }
            0xc9 => {                                   self.ret();         8 }
            0xca => { let v = self.getshort();          self.jp_z_nn(v);    12 }
            0xcb => {                           let r = self.execute_cb();  r }
            0xcc => { let v = self.getshort();          self.call_z_nn(v);  12 }
            0xcd => { let v = self.getshort();          self.call_nn(v);    12 }
            0xce => { let v = self.getbyte();           self.adc_a_n(v);    8 }
            0xcf => {                                   self.rst_8();       32 }
            0xd0 => {                                   self.ret_nc();      8 }
            0xd1 => {                                   self.pop_de();      12 }
            0xd2 => { let v = self.getshort();          self.jp_nc_nn(v);   12 }
            0xd4 => { let v = self.getshort();          self.call_nc_nn(v); 12 }
            0xd5 => {                                   self.push_de();     16 }
            0xd6 => { let v = self.getbyte();           self.sub_a_n(v);    8 }
            0xd7 => {                                   self.rst_10();      32 }
            0xd8 => {                                   self.ret_c();       8 }
            0xd9 => {                                   self.reti();        8 }
            0xda => { let v = self.getshort();          self.jp_c_nn(v);    12 }
            0xdc => { let v = self.getshort();          self.call_c_nn(v);  12 }
            0xde => { let v = self.getbyte();           self.sbc_a_n(v);    8 }
            0xdf => {                                   self.rst_18();      32 }
            0xe0 => { let v = self.getbyte();           self.ldh_n_a(v);    12 }
            0xe1 => {                                   self.pop_hl();      12 }
            0xe2 => {                                   self.ldh_c_a();     12 }
            0xe5 => {                                   self.push_hl();     16 }
            0xe6 => { let v = self.getbyte();           self.and_n(v);      8 }
            0xe7 => {                                   self.rst_20();      32 }
            0xe8 => { let v = self.getbyte();           self.add_sp_n(v);   16 }
            0xe9 => {                                   self.jp_hl();       4 }
            0xea => { let v = self.getshort();          self.ld_nn_a(v);    16 }
            0xee => { let v = self.getbyte();           self.xor_n(v);      8 }
            0xef => {                                   self.rst_28();      32 }
            0xf0 => { let v = self.getbyte();           self.ldh_a_n(v);    12 }
            0xf1 => {                                   self.pop_af();      12 }
            0xf3 => {                                   self.di();          4 }
            0xf5 => {                                   self.push_af();     16 }
            0xf6 => { let v = self.getbyte();           self.or_n(v);       8 }
            0xf7 => {                                   self.rst_30();      32 }
            0xf8 => { let v = self.getbyte();           self.ldhl_sp_d(v);  12 }
            0xf9 => {                                   self.ld_sp_hl();    8 }
            0xfa => { let v = self.getshort();          self.ld_a_nn(v);    16 }
            0xfb => {                                   self.ei();          4 }
            0xfe => { let v = self.getbyte();           self.cp_n(v);       8 }
            0xff => {                                   self.rst_38();      32 }
            _ => panic!("Unknown instruction, {:X}", op)
        }
    }

    //0xcb
    fn execute_cb(&mut self) -> u16 {
        let op = self.getbyte();
        //println!("{:X}", op);
        match op {
            0x00 => { self.rlc_b();     8 }
            0x01 => { self.rlc_c();     8 }
            0x02 => { self.rlc_d();     8 }
            0x03 => { self.rlc_e();     8 }
            0x04 => { self.rlc_h();     8 }
            0x05 => { self.rlc_l();     8 }
            0x06 => { self.rlc_hl();    16 }
            0x07 => { self.rlc_a();     8 }
            0x08 => { self.rrc_b();     8 }
            0x09 => { self.rrc_c();     8 }
            0x0a => { self.rrc_d();     8 }
            0x0b => { self.rrc_e();     8 }
            0x0c => { self.rrc_h();     8 }
            0x0d => { self.rrc_l();     8 }
            0x0e => { self.rrc_hl();    16 }
            0x0f => { self.rrc_a();     8 }
            0x10 => { self.rl_b();      8 }
            0x11 => { self.rl_c();      8 }
            0x12 => { self.rl_d();      8 }
            0x13 => { self.rl_e();      8 }
            0x14 => { self.rl_h();      8 }
            0x15 => { self.rl_l();      8 }
            0x16 => { self.rl_hl();     16 }
            0x17 => { self.rl_a();      8 }
            0x18 => { self.rr_b();      8 }
            0x19 => { self.rr_c();      8 }
            0x1a => { self.rr_d();      8 }
            0x1b => { self.rr_e();      8 }
            0x1c => { self.rr_h();      8 }
            0x1d => { self.rr_l();      8 }
            0x1e => { self.rr_hl();     16 }
            0x1f => { self.rr_a();      8 }
            0x20 => { self.sla_b();     8 }
            0x21 => { self.sla_c();     8 }
            0x22 => { self.sla_d();     8 }
            0x23 => { self.sla_e();     8 }
            0x24 => { self.sla_h();     8 }
            0x25 => { self.sla_l();     8 }
            0x26 => { self.sla_hl();    16 }
            0x27 => { self.sla_a();     8 }
            0x28 => { self.sra_b();     8 }
            0x29 => { self.sra_c();     8 }
            0x2a => { self.sra_d();     8 }
            0x2b => { self.sra_e();     8 }
            0x2c => { self.sra_h();     8 }
            0x2d => { self.sra_l();     8 }
            0x2e => { self.sra_hl();    16 }
            0x2f => { self.sra_a();     8 }
            0x30 => { self.swap_b();    8 }
            0x31 => { self.swap_c();    8 }
            0x32 => { self.swap_d();    8 }
            0x33 => { self.swap_e();    8 }
            0x34 => { self.swap_h();    8 }
            0x35 => { self.swap_l();    8 }
            0x36 => { self.swap_hl();   16 }
            0x37 => { self.swap_a();    8 }
            0x38 => { self.srl_b();     8 }
            0x39 => { self.srl_c();     8 }
            0x3a => { self.srl_d();     8 }
            0x3b => { self.srl_e();     8 }
            0x3c => { self.srl_h();     8 }
            0x3d => { self.srl_l();     8 }
            0x3e => { self.srl_hl();    16 }
            0x3f => { self.srl_a();     8 }
            0x40 => { let v = self.register.B; self.bit(1 << 0, v); 8 }
            0x41 => { let v = self.register.C; self.bit(1 << 0, v); 8 }
            0x42 => { let v = self.register.D; self.bit(1 << 0, v); 8 }
            0x43 => { let v = self.register.E; self.bit(1 << 0, v); 8 }
            0x44 => { let v = self.register.H; self.bit(1 << 0, v); 8 }
            0x45 => { let v = self.register.L; self.bit(1 << 0, v); 8 }
            0x46 => { let v = self.memory.read_byte(self.register.get_hl()); self.bit(1 << 0, v); 16 }
            0x47 => { let v = self.register.A; self.bit(1 << 0, v); 8 }
            0x48 => { let v = self.register.B; self.bit(1 << 1, v); 8 }
            0x49 => { let v = self.register.C; self.bit(1 << 1, v); 8 }
            0x4a => { let v = self.register.D; self.bit(1 << 1, v); 8 }
            0x4b => { let v = self.register.E; self.bit(1 << 1, v); 8 }
            0x4c => { let v = self.register.H; self.bit(1 << 1, v); 8 }
            0x4d => { let v = self.register.L; self.bit(1 << 1, v); 8 }
            0x4e => { let v = self.memory.read_byte(self.register.get_hl()); self.bit(1 << 1, v); 16 }
            0x4f => { let v = self.register.A; self.bit(1 << 1, v); 8 }
            0x50 => { let v = self.register.B; self.bit(1 << 2, v); 8 }
            0x51 => { let v = self.register.C; self.bit(1 << 2, v); 8 }
            0x52 => { let v = self.register.D; self.bit(1 << 2, v); 8 }
            0x53 => { let v = self.register.E; self.bit(1 << 2, v); 8 }
            0x54 => { let v = self.register.H; self.bit(1 << 2, v); 8 }
            0x55 => { let v = self.register.L; self.bit(1 << 2, v); 8 }
            0x56 => { let v = self.memory.read_byte(self.register.get_hl()); self.bit(1 << 2, v); 16 }
            0x57 => { let v = self.register.A; self.bit(1 << 2, v); 8 }
            0x58 => { let v = self.register.B; self.bit(1 << 3, v); 8 }
            0x59 => { let v = self.register.C; self.bit(1 << 3, v); 8 }
            0x5a => { let v = self.register.D; self.bit(1 << 3, v); 8 }
            0x5b => { let v = self.register.E; self.bit(1 << 3, v); 8 }
            0x5c => { let v = self.register.H; self.bit(1 << 3, v); 8 }
            0x5d => { let v = self.register.L; self.bit(1 << 3, v); 8 }
            0x5e => { let v = self.memory.read_byte(self.register.get_hl()); self.bit(1 << 3, v); 16 }
            0x5f => { let v = self.register.A; self.bit(1 << 3, v); 8 }
            0x60 => { let v = self.register.B; self.bit(1 << 4, v); 8 }
            0x61 => { let v = self.register.C; self.bit(1 << 4, v); 8 }
            0x62 => { let v = self.register.D; self.bit(1 << 4, v); 8 }
            0x63 => { let v = self.register.E; self.bit(1 << 4, v); 8 }
            0x64 => { let v = self.register.H; self.bit(1 << 4, v); 8 }
            0x65 => { let v = self.register.L; self.bit(1 << 4, v); 8 }
            0x66 => { let v = self.memory.read_byte(self.register.get_hl()); self.bit(1 << 4, v); 16 }
            0x67 => { let v = self.register.A; self.bit(1 << 4, v); 8 }
            0x68 => { let v = self.register.B; self.bit(1 << 5, v); 8 }
            0x69 => { let v = self.register.C; self.bit(1 << 5, v); 8 }
            0x6a => { let v = self.register.D; self.bit(1 << 5, v); 8 }
            0x6b => { let v = self.register.E; self.bit(1 << 5, v); 8 }
            0x6c => { let v = self.register.H; self.bit(1 << 5, v); 8 }
            0x6d => { let v = self.register.L; self.bit(1 << 5, v); 8 }
            0x6e => { let v = self.memory.read_byte(self.register.get_hl()); self.bit(1 << 5, v); 16 }
            0x6f => { let v = self.register.A; self.bit(1 << 5, v); 8 }
            0x70 => { let v = self.register.B; self.bit(1 << 6, v); 8 }
            0x71 => { let v = self.register.C; self.bit(1 << 6, v); 8 }
            0x72 => { let v = self.register.D; self.bit(1 << 6, v); 8 }
            0x73 => { let v = self.register.E; self.bit(1 << 6, v); 8 }
            0x74 => { let v = self.register.H; self.bit(1 << 6, v); 8 }
            0x75 => { let v = self.register.L; self.bit(1 << 6, v); 8 }
            0x76 => { let v = self.memory.read_byte(self.register.get_hl()); self.bit(1 << 6, v); 16 }
            0x77 => { let v = self.register.A; self.bit(1 << 6, v); 8 }
            0x78 => { let v = self.register.B; self.bit(1 << 7, v); 8 }
            0x79 => { let v = self.register.C; self.bit(1 << 7, v); 8 }
            0x7a => { let v = self.register.D; self.bit(1 << 7, v); 8 }
            0x7b => { let v = self.register.E; self.bit(1 << 7, v); 8 }
            0x7c => { let v = self.register.H; self.bit(1 << 7, v); 8 }
            0x7d => { let v = self.register.L; self.bit(1 << 7, v); 8 }
            0x7e => { let v = self.memory.read_byte(self.register.get_hl()); self.bit(1 << 7, v); 16 }
            0x7f => { let v = self.register.A; self.bit(1 << 7, v); 8 }
            0x80 => { self.register.B = self.register.B & !(1 << 0); 8 }
            0x81 => { self.register.C = self.register.C & !(1 << 0); 8 }
            0x82 => { self.register.D = self.register.D & !(1 << 0); 8 }
            0x83 => { self.register.E = self.register.E & !(1 << 0); 8 }
            0x84 => { self.register.H = self.register.H & !(1 << 0); 8 }
            0x85 => { self.register.L = self.register.L & !(1 << 0); 8 }
            0x86 => { let v = self.memory.read_byte(self.register.get_hl()) & !(1 << 0);
                      self.memory.write_byte(self.register.get_hl(), v); 16 }
            0x87 => { self.register.A = self.register.A & !(1 << 0); 8 }
            0x88 => { self.register.B = self.register.B & !(1 << 1); 8 }
            0x89 => { self.register.C = self.register.C & !(1 << 1); 8 }
            0x8a => { self.register.D = self.register.D & !(1 << 1); 8 }
            0x8b => { self.register.E = self.register.E & !(1 << 1); 8 }
            0x8c => { self.register.H = self.register.H & !(1 << 1); 8 }
            0x8d => { self.register.L = self.register.L & !(1 << 1); 8 }
            0x8e => { let v = self.memory.read_byte(self.register.get_hl()) & !(1 << 1);
                      self.memory.write_byte(self.register.get_hl(), v); 16 }
            0x8f => { self.register.A = self.register.A & !(1 << 1); 8 }
            0x90 => { self.register.B = self.register.B & !(1 << 2); 8 }
            0x91 => { self.register.C = self.register.C & !(1 << 2); 8 }
            0x92 => { self.register.D = self.register.D & !(1 << 2); 8 }
            0x93 => { self.register.E = self.register.E & !(1 << 2); 8 }
            0x94 => { self.register.H = self.register.H & !(1 << 2); 8 }
            0x95 => { self.register.L = self.register.L & !(1 << 2); 8 }
            0x96 => { let v = self.memory.read_byte(self.register.get_hl()) & !(1 << 2);
                      self.memory.write_byte(self.register.get_hl(), v); 16 }
            0x97 => { self.register.A = self.register.A & !(1 << 2); 8 }
            0x98 => { self.register.B = self.register.B & !(1 << 3); 8 }
            0x99 => { self.register.C = self.register.C & !(1 << 3); 8 }
            0x9a => { self.register.D = self.register.D & !(1 << 3); 8 }
            0x9b => { self.register.E = self.register.E & !(1 << 3); 8 }
            0x9c => { self.register.H = self.register.H & !(1 << 3); 8 }
            0x9d => { self.register.L = self.register.L & !(1 << 3); 8 }
            0x9e => { let v = self.memory.read_byte(self.register.get_hl()) & !(1 << 3);
                      self.memory.write_byte(self.register.get_hl(), v); 16 }
            0x9f => { self.register.A = self.register.A & !(1 << 3); 8 }
            0xa0 => { self.register.B = self.register.B & !(1 << 4); 8 }
            0xa1 => { self.register.C = self.register.C & !(1 << 4); 8 }
            0xa2 => { self.register.D = self.register.D & !(1 << 4); 8 }
            0xa3 => { self.register.E = self.register.E & !(1 << 4); 8 }
            0xa4 => { self.register.H = self.register.H & !(1 << 4); 8 }
            0xa5 => { self.register.L = self.register.L & !(1 << 4); 8 }
            0xa6 => { let v = self.memory.read_byte(self.register.get_hl()) & !(1 << 4);
                      self.memory.write_byte(self.register.get_hl(), v); 16 }
            0xa7 => { self.register.A = self.register.A & !(1 << 4); 8 }
            0xa8 => { self.register.B = self.register.B & !(1 << 5); 8 }
            0xa9 => { self.register.C = self.register.C & !(1 << 5); 8 }
            0xaa => { self.register.D = self.register.D & !(1 << 5); 8 }
            0xab => { self.register.E = self.register.E & !(1 << 5); 8 }
            0xac => { self.register.H = self.register.H & !(1 << 5); 8 }
            0xad => { self.register.L = self.register.L & !(1 << 5); 8 }
            0xae => { let v = self.memory.read_byte(self.register.get_hl()) & !(1 << 5);
                      self.memory.write_byte(self.register.get_hl(), v); 16 }
            0xaf => { self.register.A = self.register.A & !(1 << 5); 8 }
            0xb0 => { self.register.B = self.register.B & !(1 << 6); 8 }
            0xb1 => { self.register.C = self.register.C & !(1 << 6); 8 }
            0xb2 => { self.register.D = self.register.D & !(1 << 6); 8 }
            0xb3 => { self.register.E = self.register.E & !(1 << 6); 8 }
            0xb4 => { self.register.H = self.register.H & !(1 << 6); 8 }
            0xb5 => { self.register.L = self.register.L & !(1 << 6); 8 }
            0xb6 => { let v = self.memory.read_byte(self.register.get_hl()) & !(1 << 6);
                      self.memory.write_byte(self.register.get_hl(), v); 16 }
            0xb7 => { self.register.A = self.register.A & !(1 << 6); 8 }
            0xb8 => { self.register.B = self.register.B & !(1 << 7); 8 }
            0xb9 => { self.register.C = self.register.C & !(1 << 7); 8 }
            0xba => { self.register.D = self.register.D & !(1 << 7); 8 }
            0xbb => { self.register.E = self.register.E & !(1 << 7); 8 }
            0xbc => { self.register.H = self.register.H & !(1 << 7); 8 }
            0xbd => { self.register.L = self.register.L & !(1 << 7); 8 }
            0xbe => { let v = self.memory.read_byte(self.register.get_hl()) & !(1 << 7);
                      self.memory.write_byte(self.register.get_hl(), v); 16 }
            0xbf => { self.register.A = self.register.A & !(1 << 7); 8 }
            0xc0 => { self.register.B = self.register.B | (1 << 0); 8 }
            0xc1 => { self.register.C = self.register.C | (1 << 0); 8 }
            0xc2 => { self.register.D = self.register.D | (1 << 0); 8 }
            0xc3 => { self.register.E = self.register.E | (1 << 0); 8 }
            0xc4 => { self.register.H = self.register.H | (1 << 0); 8 }
            0xc5 => { self.register.L = self.register.L | (1 << 0); 8 }
            0xc6 => { let v = self.memory.read_byte(self.register.get_hl()) | (1 << 0);
                      self.memory.write_byte(self.register.get_hl(), v); 16 }
            0xc7 => { self.register.A = self.register.A | (1 << 0); 8 }
            0xc8 => { self.register.B = self.register.B | (1 << 1); 8 }
            0xc9 => { self.register.C = self.register.C | (1 << 1); 8 }
            0xca => { self.register.D = self.register.D | (1 << 1); 8 }
            0xcb => { self.register.E = self.register.E | (1 << 1); 8 }
            0xcc => { self.register.H = self.register.H | (1 << 1); 8 }
            0xcd => { self.register.L = self.register.L | (1 << 1); 8 }
            0xce => { let v = self.memory.read_byte(self.register.get_hl()) | (1 << 1);
                      self.memory.write_byte(self.register.get_hl(), v); 16 }
            0xcf => { self.register.A = self.register.A | (1 << 1); 8 }
            0xd0 => { self.register.B = self.register.B | (1 << 2); 8 }
            0xd1 => { self.register.C = self.register.C | (1 << 2); 8 }
            0xd2 => { self.register.D = self.register.D | (1 << 2); 8 }
            0xd3 => { self.register.E = self.register.E | (1 << 2); 8 }
            0xd4 => { self.register.H = self.register.H | (1 << 2); 8 }
            0xd5 => { self.register.L = self.register.L | (1 << 2); 8 }
            0xd6 => { let v = self.memory.read_byte(self.register.get_hl()) | (1 << 2);
                      self.memory.write_byte(self.register.get_hl(), v); 16 }
            0xd7 => { self.register.A = self.register.A | (1 << 2); 8 }
            0xd8 => { self.register.B = self.register.B | (1 << 3); 8 }
            0xd9 => { self.register.C = self.register.C | (1 << 3); 8 }
            0xda => { self.register.D = self.register.D | (1 << 3); 8 }
            0xdb => { self.register.E = self.register.E | (1 << 3); 8 }
            0xdc => { self.register.H = self.register.H | (1 << 3); 8 }
            0xdd => { self.register.L = self.register.L | (1 << 3); 8 }
            0xde => { let v = self.memory.read_byte(self.register.get_hl()) | (1 << 3);
                      self.memory.write_byte(self.register.get_hl(), v); 16 }
            0xdf => { self.register.A = self.register.A | (1 << 3); 8 }
            0xe0 => { self.register.B = self.register.B | (1 << 4); 8 }
            0xe1 => { self.register.C = self.register.C | (1 << 4); 8 }
            0xe2 => { self.register.D = self.register.D | (1 << 4); 8 }
            0xe3 => { self.register.E = self.register.E | (1 << 4); 8 }
            0xe4 => { self.register.H = self.register.H | (1 << 4); 8 }
            0xe5 => { self.register.L = self.register.L | (1 << 4); 8 }
            0xe6 => { let v = self.memory.read_byte(self.register.get_hl()) | (1 << 4);
                      self.memory.write_byte(self.register.get_hl(), v); 16 }
            0xe7 => { self.register.A = self.register.A | (1 << 4); 8 }
            0xe8 => { self.register.B = self.register.B | (1 << 5); 8 }
            0xe9 => { self.register.C = self.register.C | (1 << 5); 8 }
            0xea => { self.register.D = self.register.D | (1 << 5); 8 }
            0xeb => { self.register.E = self.register.E | (1 << 5); 8 }
            0xec => { self.register.H = self.register.H | (1 << 5); 8 }
            0xed => { self.register.L = self.register.L | (1 << 5); 8 }
            0xee => { let v = self.memory.read_byte(self.register.get_hl()) | (1 << 5);
                      self.memory.write_byte(self.register.get_hl(), v); 16 }
            0xef => { self.register.A = self.register.A | (1 << 5); 8 }
            0xf0 => { self.register.B = self.register.B | (1 << 6); 8 }
            0xf1 => { self.register.C = self.register.C | (1 << 6); 8 }
            0xf2 => { self.register.D = self.register.D | (1 << 6); 8 }
            0xf3 => { self.register.E = self.register.E | (1 << 6); 8 }
            0xf4 => { self.register.H = self.register.H | (1 << 6); 8 }
            0xf5 => { self.register.L = self.register.L | (1 << 6); 8 }
            0xf6 => { let v = self.memory.read_byte(self.register.get_hl()) | (1 << 6);
                      self.memory.write_byte(self.register.get_hl(), v); 16 }
            0xf7 => { self.register.A = self.register.A | (1 << 6); 8 }
            0xf8 => { self.register.B = self.register.B | (1 << 7); 8 }
            0xf9 => { self.register.C = self.register.C | (1 << 7); 8 }
            0xfa => { self.register.D = self.register.D | (1 << 7); 8 }
            0xfb => { self.register.E = self.register.E | (1 << 7); 8 }
            0xfc => { self.register.H = self.register.H | (1 << 7); 8 }
            0xfd => { self.register.L = self.register.L | (1 << 7); 8 }
            0xfe => { let v = self.memory.read_byte(self.register.get_hl()) | (1 << 7);
                      self.memory.write_byte(self.register.get_hl(), v); 16 }
            0xff => { self.register.A = self.register.A | (1 << 7); 8 }
            _ => panic!("Unknown instruction in cb")
        }
    }

    //////////////////////////////////////////////////////
    // CB
    //////////////////////////////////////////////////////

    fn rlc(&mut self, value: u8) -> u8 {
        let carry = value & 0x80 == 0x80;
        let v = (value << 1) + (if carry { 1 } else { 0 });
        if carry { self.register.flag_set(C) } else { self.register.flag_reset(C) }
        if v == 0 { self.register.flag_set(Z) } else { self.register.flag_reset(Z) }
        self.register.flag_reset(N);
        self.register.flag_reset(H);
        v
    }

    fn rrc(&mut self, value: u8) -> u8 {
        let carry = value & 0x01 == 0x01;
        let v = (value >> 1) | (if carry { 0x80 } else { 0 });
        if carry { self.register.flag_set(C) } else { self.register.flag_reset(C) }
        if v == 0 { self.register.flag_set(Z) } else { self.register.flag_reset(Z) }
        self.register.flag_reset(N);
        self.register.flag_reset(H);
        v
    }

    fn rl(&mut self, value: u8) -> u8 {
        // let carry = value & 0x80 == 0x80;
        // let v = (value << 1) | (if self.register.flag_get(C) { 1 } else { 0 });
        // if carry { self.register.flag_set(C) } else { self.register.flag_reset(C) }
        let carry = (if self.register.flag_get(C) { 1 } else { 0 });
        let v = (value << 1) + carry;
        if value & 0x80 != 0 { self.register.flag_set(C) } else { self.register.flag_reset(C) }
        if v == 0 { self.register.flag_set(Z) } else { self.register.flag_reset(Z) }
        self.register.flag_reset(N);
        self.register.flag_reset(H);
        v
    }

    fn rr(&mut self, value: u8) -> u8 {
        // let carry = value & 0x01 == 0x01;
        // let v = (value >> 1) | (if self.register.flag_get(C) { 0x80 } else { 0 });
        // if carry { self.register.flag_set(C) } else { self.register.flag_reset(C) }
        let v = (value >> 1) | (if self.register.flag_get(C) { 0x80 } else { 0 });
        if (value & 0x01) != 0 { self.register.flag_set(C) } else { self.register.flag_reset(C) }
        if v == 0 { self.register.flag_set(Z) } else { self.register.flag_reset(Z) }
        self.register.flag_reset(N);
        self.register.flag_reset(H);
        v
    }

    fn sla(&mut self, value: u8) -> u8 {
        let v = value << 1;
        if (value & 0x80) != 0 { self.register.flag_set(C) } else { self.register.flag_reset(C) }
        if v == 0 { self.register.flag_set(Z) } else { self.register.flag_reset(Z) }
        self.register.flag_reset(N);
        self.register.flag_reset(H);
        v
    }

    fn sra(&mut self, value: u8) -> u8 {
        let v = (value >> 1) | (value & 0x80);
        if (value & 0x01) != 0 { self.register.flag_set(C) } else { self.register.flag_reset(C) }
        if v == 0 { self.register.flag_set(Z) } else { self.register.flag_reset(Z) }
        self.register.flag_reset(N);
        self.register.flag_reset(H);
        v
    }

    fn srl(&mut self, value: u8) -> u8 {
        let v = value >> 1;
        if (value & 0x01) != 0 { self.register.flag_set(C) } else { self.register.flag_reset(C) }
        if v == 0 { self.register.flag_set(Z) } else { self.register.flag_reset(Z) }
        self.register.flag_reset(N);
        self.register.flag_reset(H);
        v
    }

    fn swap(&mut self, value: u8) -> u8 {
        let v = ((value & 0xf) << 4) | ((value & 0xf0) >> 4);
        if v == 0 { self.register.flag_set(Z) } else { self.register.flag_reset(Z) }
        self.register.flag_reset(C);
        self.register.flag_reset(N);
        self.register.flag_reset(H);
        v
    }

    fn bit(&mut self, bit: u8, value: u8) {
        let v = value & bit;
        if v == 0 { self.register.flag_set(Z) } else { self.register.flag_reset(Z) }
        self.register.flag_reset(N);
        self.register.flag_set(H);
    }

    //0x00
    fn rlc_b(&mut self) {
        let v = self.register.B;
        self.register.B = self.rlc(v);
    }

    //0x01
    fn rlc_c(&mut self) {
        let v = self.register.C;
        self.register.C = self.rlc(v);
    }

    //0x02
    fn rlc_d(&mut self) {
        let v = self.register.D;
        self.register.D = self.rlc(v);
    }

    //0x03
    fn rlc_e(&mut self) {
        let v = self.register.E;
        self.register.E = self.rlc(v);
    }

    //0x04
    fn rlc_h(&mut self) {
        let v = self.register.H;
        self.register.H = self.rlc(v);
    }

    //0x05
    fn rlc_l(&mut self) {
        let v = self.register.L;
        self.register.L = self.rlc(v);
    }

    //0x06
    fn rlc_hl(&mut self) {
        let v = self.memory.read_byte(self.register.get_hl());
        let v2 = self.rlc(v);
        self.memory.write_byte(self.register.get_hl(), v2);
    }

    //0x07
    fn rlc_a(&mut self) {
        let v = self.register.A;
        self.register.A = self.rlc(v);
    }

    //0x08
    fn rrc_b(&mut self) {
        let v = self.register.B;
        self.register.B = self.rrc(v);
    }

    //0x09
    fn rrc_c(&mut self) {
        let v = self.register.C;
        self.register.C = self.rrc(v);
    }

    //0x0a
    fn rrc_d(&mut self) {
        let v = self.register.D;
        self.register.D = self.rrc(v);
    }

    //0x0b
    fn rrc_e(&mut self) {
        let v = self.register.E;
        self.register.E = self.rrc(v);
    }

    //0x0c
    fn rrc_h(&mut self) {
        let v = self.register.H;
        self.register.H = self.rrc(v);
    }

    //0x0d
    fn rrc_l(&mut self) {
        let v = self.register.L;
        self.register.L = self.rrc(v);
    }

    //0x0e
    fn rrc_hl(&mut self) {
        let v = self.memory.read_byte(self.register.get_hl());
        let v2 = self.rrc(v);
        self.memory.write_byte(self.register.get_hl(), v2);
    }

    //0x0f
    fn rrc_a(&mut self) {
        let v = self.register.A;
        self.register.A = self.rrc(v);
    }

    //0x10
    fn rl_b(&mut self) {
        let v = self.register.B;
        self.register.B = self.rl(v);
    }

    //0x11
    fn rl_c(&mut self) {
        let v = self.register.C;
        self.register.C = self.rl(v);
    }

    //0x12
    fn rl_d(&mut self) {
        let v = self.register.D;
        self.register.D = self.rl(v);
    }

    //0x13
    fn rl_e(&mut self) {
        let v = self.register.E;
        self.register.E = self.rl(v);
    }

    //0x14
    fn rl_h(&mut self) {
        let v = self.register.H;
        self.register.H = self.rl(v);
    }

    //0x15
    fn rl_l(&mut self) {
        let v = self.register.L;
        self.register.L = self.rl(v);
    }

    //0x16
    fn rl_hl(&mut self) {
        let v = self.memory.read_byte(self.register.get_hl());
        let v2 = self.rl(v);
        self.memory.write_byte(self.register.get_hl(), v2);
    }

    //0x17
    fn rl_a(&mut self) {
        let v = self.register.A;
        self.register.A = self.rl(v);
    }

    //0x18
    fn rr_b(&mut self) {
        let v = self.register.B;
        self.register.B = self.rr(v);
    }

    //0x19
    fn rr_c(&mut self) {
        let v = self.register.C;
        self.register.C = self.rr(v);
    }

    //0x1a
    fn rr_d(&mut self) {
        let v = self.register.D;
        self.register.D = self.rr(v);
    }

    //0x1b
    fn rr_e(&mut self) {
        let v = self.register.E;
        self.register.E = self.rr(v);
    }

    //0x1c
    fn rr_h(&mut self) {
        let v = self.register.H;
        self.register.H = self.rr(v);
    }

    //0x1d
    fn rr_l(&mut self) {
        let v = self.register.L;
        self.register.L = self.rr(v);
    }

    //0x1e
    fn rr_hl(&mut self) {
        let v = self.memory.read_byte(self.register.get_hl());
        let v2 = self.rr(v);
        self.memory.write_byte(self.register.get_hl(), v2);
    }

    //0x1f
    fn rr_a(&mut self) {
        let v = self.register.A;
        self.register.A = self.rr(v);
    }

    //0x20
    fn sla_b(&mut self) {
        let v = self.register.B;
        self.register.B = self.sla(v);
    }

    //0x21
    fn sla_c(&mut self) {
        let v = self.register.C;
        self.register.C = self.sla(v);
    }

    //0x22
    fn sla_d(&mut self) {
        let v = self.register.D;
        self.register.D = self.sla(v);
    }

    //0x23
    fn sla_e(&mut self) {
        let v = self.register.E;
        self.register.E = self.sla(v);
    }

    //0x24
    fn sla_h(&mut self) {
        let v = self.register.H;
        self.register.H = self.sla(v);
    }

    //0x25
    fn sla_l(&mut self) {
        let v = self.register.L;
        self.register.L = self.sla(v);
    }

    //0x26
    fn sla_hl(&mut self) {
        let v = self.memory.read_byte(self.register.get_hl());
        let v2 = self.sla(v);
        self.memory.write_byte(self.register.get_hl(), v2);
    }

    //0x27
    fn sla_a(&mut self) {
        let v = self.register.A;
        self.register.A = self.sla(v);
    }

    //0x28
    fn sra_b(&mut self) {
        let v = self.register.B;
        self.register.B = self.sra(v);
    }

    //0x29
    fn sra_c(&mut self) {
        let v = self.register.C;
        self.register.C = self.sra(v);
    }

    //0x2a
    fn sra_d(&mut self) {
        let v = self.register.D;
        self.register.D = self.sra(v);
    }

    //0x2b
    fn sra_e(&mut self) {
        let v = self.register.E;
        self.register.E = self.sra(v);
    }

    //0x2c
    fn sra_h(&mut self) {
        let v = self.register.H;
        self.register.H = self.sra(v);
    }

    //0x2d
    fn sra_l(&mut self) {
        let v = self.register.L;
        self.register.L = self.sra(v);
    }

    //0x2e
    fn sra_hl(&mut self) {
        let v = self.memory.read_byte(self.register.get_hl());
        let v2 = self.sra(v);
        self.memory.write_byte(self.register.get_hl(), v2);
    }

    //0x2f
    fn sra_a(&mut self) {
        let v = self.register.A;
        self.register.A = self.sra(v);
    }

    //0x30
    fn swap_b(&mut self) {
        let v = self.register.B;
        self.register.B = self.swap(v);
    }

    //0x31
    fn swap_c(&mut self) {
        let v = self.register.C;
        self.register.C = self.swap(v);
    }

    //0x32
    fn swap_d(&mut self) {
        let v = self.register.D;
        self.register.D = self.swap(v);
    }

    //0x33
    fn swap_e(&mut self) {
        let v = self.register.E;
        self.register.E = self.swap(v);
    }

    //0x34
    fn swap_h(&mut self) {
        let v = self.register.H;
        self.register.H = self.swap(v);
    }

    //0x35
    fn swap_l(&mut self) {
        let v = self.register.L;
        self.register.L = self.swap(v);
    }

    //0x36
    fn swap_hl(&mut self) {
        let v = self.memory.read_byte(self.register.get_hl());
        let v2 = self.swap(v);
        self.memory.write_byte(self.register.get_hl(), v2);
    }

    //0x37
    fn swap_a(&mut self) {
        let v = self.register.A;
        self.register.A = self.swap(v);
    }

    //0x38
    fn srl_b(&mut self) {
        let v = self.register.B;
        self.register.B = self.srl(v);
    }

    //0x39
    fn srl_c(&mut self) {
        let v = self.register.C;
        self.register.C = self.srl(v);
    }

    //0x3a
    fn srl_d(&mut self) {
        let v = self.register.D;
        self.register.D = self.srl(v);
    }

    //0x3b
    fn srl_e(&mut self) {
        let v = self.register.E;
        self.register.E = self.srl(v);
    }

    //0x3c
    fn srl_h(&mut self) {
        let v = self.register.H;
        self.register.H = self.srl(v);
    }

    //0x3d
    fn srl_l(&mut self) {
        let v = self.register.L;
        self.register.L = self.srl(v);
    }

    //0x3e
    fn srl_hl(&mut self) {
        let v = self.memory.read_byte(self.register.get_hl());
        let v2 = self.srl(v);
        self.memory.write_byte(self.register.get_hl(), v2);
    }

    //0x3f
    fn srl_a(&mut self) {
        let v = self.register.A;
        self.register.A = self.srl(v);
    }

    //////////////////////////////////////////////////////
    // REGULAR
    //////////////////////////////////////////////////////

    fn inc(&mut self, value: u8) -> u8 {
        let v = value.wrapping_add(1);
        if (value & 0x0f) == 0x0f { self.register.flag_set(H) } else { self.register.flag_reset(H) }
        if v == 0 { self.register.flag_set(Z) } else { self.register.flag_reset(Z) }
        self.register.flag_reset(N);
        v
    }

    fn dec(&mut self, value: u8) -> u8 {
        let v = value.wrapping_sub(1);
        if (value & 0x0f) != 0 { self.register.flag_reset(H) } else { self.register.flag_set(H) }
        if v == 0 { self.register.flag_set(Z) } else { self.register.flag_reset(Z) }
        self.register.flag_set(N);
        v
    }

    fn and(&mut self, value: u8) {
        let v = value & self.register.A;
        if v == 0 { self.register.flag_set(Z) } else { self.register.flag_reset(Z) }
        self.register.flag_reset(N);
        self.register.flag_reset(C);
        self.register.flag_set(H);
        self.register.A = v;
    }

    fn xor(&mut self, value: u8) {
        let v = self.register.A ^ value;
        if v == 0 { self.register.flag_set(Z) } else { self.register.flag_reset(Z) }
        self.register.flag_reset(N);
        self.register.flag_reset(H);
        self.register.flag_reset(C);
        self.register.A = v;
    }

    fn or(&mut self, value: u8) {
        let v = self.register.A | value;
        if v == 0 { self.register.flag_set(Z) } else { self.register.flag_reset(Z) }
        self.register.flag_reset(N);
        self.register.flag_reset(H);
        self.register.flag_reset(C);
        self.register.A = v;
    }

    fn cp(&mut self, value: u8) {
        if self.register.A == value { self.register.flag_set(Z) } else { self.register.flag_reset(Z) }
        if (self.register.A & 0x0f) < (value & 0x0f) { self.register.flag_set(H) } else { self.register.flag_reset(H) }
        if self.register.A < value { self.register.flag_set(C) } else { self.register.flag_reset(C) }
        self.register.flag_set(N);
    }

    fn add_a(&mut self, value: u8) {
        let a = self.register.A;
        let v = a.wrapping_add(value);
        if v == 0 { self.register.flag_set(Z) } else { self.register.flag_reset(Z) }
        if (self.register.A & 0x0f) + (value & 0x0f) > 0x0f { self.register.flag_set(H) } else { self.register.flag_reset(H) }
        //if (self.register.A as u16 + value as u16) > 0xff { self.register.flag_set(C) } else { self.register.flag_reset(C) }
        if (v & 0xff00) != 0 { self.register.flag_set(C) } else { self.register.flag_reset(C) }
        self.register.flag_reset(N);
        self.register.A = v;
    }

    fn adc_a(&mut self, value: u8) {
        let a = self.register.A;
        let carry = if self.register.flag_get(C) { 1 } else { 0 };
        let v = a.wrapping_add(value).wrapping_add(carry);
        //if v == 0 { self.register.flag_set(Z) } else { self.register.flag_reset(Z) }
        if a == value { self.register.flag_set(Z) } else { self.register.flag_reset(Z) }
        if (self.register.A & 0x0f) + (value & 0x0f) > 0x0f { self.register.flag_set(H) } else { self.register.flag_reset(H) }
        //if (self.register.A as u16 + value as u16 + carry as u16) > 0xff { self.register.flag_set(C) } else { self.register.flag_reset(C) }
        if (v & 0xff00) != 0 { self.register.flag_set(C) } else { self.register.flag_reset(C) }
        self.register.flag_reset(N); // CHECK
        //self.register.A = v;
        self.register.A = v & 0xff;
    }

    fn sub_a(&mut self, value: u8) {
        let a = self.register.A;
        let v = a.wrapping_sub(value);
        if v == 0 { self.register.flag_set(Z) } else { self.register.flag_reset(Z) }
        if (self.register.A & 0x0f) < (value & 0x0f) { self.register.flag_set(H) } else { self.register.flag_reset(H) }
        if value > self.register.A { self.register.flag_set(C) } else { self.register.flag_reset(C) }
        self.register.flag_set(N);
        self.register.A = v;
    }

    fn sbc_a(&mut self, value: u8) {
        let a = self.register.A;
        let carry = if self.register.flag_get(C) { 1 } else { 0 };
        let v = a.wrapping_sub(value).wrapping_sub(carry);
        //if v == 0 { self.register.flag_set(Z) } else { self.register.flag_reset(Z) }
        if v == a { self.register.flag_set(Z) } else { self.register.flag_reset(Z) }
        if (self.register.A & 0x0f) < ((value + carry) & 0x0f) { self.register.flag_set(H) } else { self.register.flag_reset(H) }
        if value > self.register.A { self.register.flag_set(C) } else { self.register.flag_reset(C) }
        self.register.flag_set(N);
        self.register.A = v;
    }

    fn add_hl(&mut self, value: u16) {
        let hl = self.register.get_hl();
        let res = hl.wrapping_add(value);
        self.register.flag_reset(N);
        //if hl > 0xFFFF - value { self.register.flag_set(C) } else { self.register.flag_reset(C) }
        if (res & 0xffff0000) != 0 { self.register.flag_set(C) } else { self.register.flag_reset(C) }
        //if (hl & 0x07FF) + (value & 0x07FF) > 0x07FF { self.register.flag_set(H) } else { self.register.flag_reset(H) }
        if (hl & 0x0f) + (value & 0x0f) > 0x0f { self.register.flag_set(H) } else { self.register.flag_reset(H) }
        self.register.set_hl(res);
    }

    // http://imrannazar.com/Gameboy-Z80-Opcode-Map
    // https://realboyemulator.files.wordpress.com/2013/01/gbcpuman.pdf Page 65+

    //0x00
    fn nop(&self) {
        
    }

    // 0x01
    fn ld_bc_nn(&mut self, operand: u16) {
        self.register.set_bc(operand);
    }

    //0x02
    fn ld_bc_a(&mut self) {
        self.memory.write_byte(self.register.get_bc(), self.register.A);
    }

    //0x03
    fn inc_bc(&mut self) {
        let v = self.register.get_bc().wrapping_add(1);
        self.register.set_bc(v);
    }

    //0x04
    fn inc_b(&mut self) {
        let b = self.register.B;
        let v = self.inc(b);
        self.register.B = v;
    }

    //0x05
    fn dec_b(&mut self) {
        let b = self.register.B;
        let v = self.dec(b);
        self.register.B = v;
    }

    //0x06
    fn ld_b_n(&mut self, operand: u8) {
        self.register.B = operand;
    }

    //0x07
    fn rlca(&mut self) {
        let carry = (self.register.A & 0x80) == 0x80; 
        if carry { self.register.flag_set(C) } else { self.register.flag_reset(C) }
        self.register.A <<= 1;
        self.register.A |= if carry { 1 } else { 0 };
        if self.register.A == 0 { self.register.flag_set(Z) } else { self.register.flag_reset(Z) }
        self.register.flag_reset(H);
        self.register.flag_reset(N);
    }

    //0x08
    fn ld_nn_sp(&mut self, operand: u16) {
        self.memory.write_short(operand, self.register.SP);
    }

    //0x09
    fn add_hl_bc(&mut self, operand: u16) {
        self.add_hl(operand);
    }

    //0x0a
    fn ld_a_bc(&mut self) {
        let v = self.memory.read_byte(self.register.get_bc());
        self.register.A = v;
    }

    //0x0b
    fn dec_bc(&mut self) {
        let v = self.register.get_bc().wrapping_sub(1);
        self.register.set_bc(v);
    }

    //0x0c
    fn inc_c(&mut self) {
        let c = self.register.C;
        let v = self.inc(c);
        self.register.C = v;
    }

    //0x0d
    fn dec_c(&mut self) {
        let c = self.register.C;
        let v = self.dec(c);
        self.register.C = v;
    }

    //0x0e
    fn ld_c_n(&mut self, operand: u8) {
        self.register.C = operand;
    }

    //0x0f
    fn rrca(&mut self) {
        let carry = (self.register.A & 0x01) == 0x01;
        if carry { self.register.flag_set(C) } else { self.register.flag_reset(C) }
        self.register.A >>= 1;
	    if carry { self.register.A |= 0x80 }
        if self.register.A == 0 { self.register.flag_set(Z) } else { self.register.flag_reset(Z) }
        self.register.flag_reset(H);
        self.register.flag_reset(N);
    }

    //0x10
    fn stop(&mut self) {
        self.stopped = true;
    }

    //0x11
    fn ld_de_nn(&mut self, operand: u16) {
        self.register.set_de(operand);
    }

    //0x12
    fn ld_de_a(&mut self) {
        self.memory.write_byte(self.register.get_de(), self.register.A);
    }

    //0x13
    fn inc_de(&mut self) {
        let v = self.register.get_de().wrapping_add(1);
        self.register.set_de(v);
    }

    //0x14
    fn inc_d(&mut self) {
        let d = self.register.D;
        let v = self.inc(d);
        self.register.D = v;
    }

    //0x15
    fn dec_d(&mut self) {
        let d = self.register.D;
        let v = self.dec(d);
        self.register.D = v;
    }

    //0x16
    fn ld_d_n(&mut self, operand: u8) {
        self.register.D = operand;
    }

    //0x17
    fn rla(&mut self) {
        let carry = (self.register.A & 0x80) == 0x80; 
        if carry { self.register.flag_set(C) } else { self.register.flag_reset(C) }
        self.register.A <<= 1;
        self.register.A |= if carry { 1 } else { 0 };
        if self.register.A == 0 { self.register.flag_set(Z) } else { self.register.flag_reset(Z) }
        self.register.flag_reset(H);
        self.register.flag_reset(N);
    }

    //0x18
    fn jr_n(&mut self, operand: i8) {
        self.register.PC = ((self.register.PC as u32 as i32) + operand as i32) as u16;
    }

    //0x19
    fn add_hl_de(&mut self, operand: u16) {
        self.add_hl(operand);
    }

    //0x1a
    fn ld_a_de(&mut self) {
        let v = self.memory.read_byte(self.register.get_de());
        self.register.A = v;
    }

    //0x1b
    fn dec_de(&mut self) {
        let v = self.register.get_de().wrapping_sub(1);
        self.register.set_de(v);
    }

    //0x1c
    fn inc_e(&mut self) {
        let e = self.register.E;
        let v = self.inc(e);
        self.register.E = v;
    }

    //0x1d
    fn dec_e(&mut self) {
        let e = self.register.E;
        let v = self.dec(e);
        self.register.E = v;
    }

    //0x1e
    fn ld_e_n(&mut self, operand: u8) {
        self.register.E = operand;
    }

    //0x1f
    fn rra(&mut self) {
        let carry = self.register.A & 0x01 == 0x01;
        if carry { self.register.flag_set(C) } else { self.register.flag_reset(C) }
        self.register.A >>= 1;
        self.register.A |= if self.register.flag_get(C) { 0x80 } else { 0 };
        if self.register.A == 0 { self.register.flag_set(Z) } else { self.register.flag_reset(Z) }
        self.register.flag_reset(H);
        self.register.flag_reset(N);
    }

    //0x20
    fn jr_nz_n(&mut self, operand: i8) {
        if !self.register.flag_get(Z) {
            self.register.PC = ((self.register.PC as u32 as i32) + operand as i32) as u16;
        }
    }

    //0x21
    fn ld_hl_nn(&mut self, operand: u16) {
        self.register.set_hl(operand);
    }

    //0x22
    fn ldi_hl_a(&mut self) {
        self.memory.write_byte(self.register.get_hl(), self.register.A);
        let v = self.register.get_hl().wrapping_add(1);
        self.register.set_hl(v);
    }

    //0x23
    fn inc_hl(&mut self) {
        let v = self.register.get_hl().wrapping_add(1);
        self.register.set_hl(v);
    }

    //0x24
    fn inc_h(&mut self) {
        let h = self.register.H;
        let v = self.inc(h);
        self.register.H = v;
    }

    //0x25
    fn dec_h(&mut self) {
        let h = self.register.H;
        let v = self.dec(h);
        self.register.H = v;
    }

    //0x26
    fn ld_h_n(&mut self, operand: u8) {
        self.register.H = operand;
    }

    //0x27
    fn daa(&mut self) {
        let mut a = self.register.A;
        let mut adjust = if self.register.flag_get(C) { 0x60 } else { 0x00 };
        if self.register.flag_get(H) { adjust |= 0x06; };
        if !self.register.flag_get(N) {
            if a & 0x0F > 0x09 { adjust |= 0x06; };
            if a > 0x99 { adjust |= 0x60; };
            a = a.wrapping_add(adjust);
        } else {
            a = a.wrapping_sub(adjust);
        }
        if adjust >= 0x60 { self.register.flag_set(C) } else { self.register.flag_reset(C) }
        if self.register.A == 0 { self.register.flag_set(Z) } else { self.register.flag_reset(Z) }
        self.register.flag_reset(H);
        self.register.A = a;
    }

    //0x28
    fn jr_z_n(&mut self, operand: i8) {
        if self.register.flag_get(Z) {
            self.register.PC = ((self.register.PC as u32 as i32) + operand as i32) as u16;
        }
    }

    //0x29
    fn add_hl_hl(&mut self, operand: u16) {
        self.add_hl(operand);
    }

    //0x2a
    fn ldi_a_hl(&mut self) {
        let a = self.memory.read_byte(self.register.get_hl());
        let v = self.register.get_hl().wrapping_add(1);
        self.register.set_hl(v);
        self.register.A = a;
    }

    //0x2b
    fn dec_hl(&mut self) {
        let v = self.register.get_hl().wrapping_sub(1);
        self.register.set_hl(v);
    }

    //0x2c
    fn inc_l(&mut self) {
        let l = self.register.L;
        let v = self.inc(l);
        self.register.L = v;
    }

    //0x2d
    fn dec_l(&mut self) {
        let l = self.register.L;
        let v = self.dec(l);
        self.register.L = v;
    }

    //0x2e
    fn ld_l_n(&mut self, operand: u8) {
        self.register.L = operand;
    }

    //0x2f
    fn cpl(&mut self) {
        self.register.A = !self.register.A;
        self.register.flag_set(N);
        self.register.flag_set(H);
    }

    //0x30 
    fn jr_nc_n(&mut self, operand: i8) {
        if !self.register.flag_get(C) {
            self.register.PC = ((self.register.PC as u32 as i32) + operand as i32) as u16;
        }
    }

    //0x31
    fn ld_sp_nn(&mut self, operand: u16) {
        self.register.SP = operand;
    }

    //0x32
    fn ldd_hl_a(&mut self) {
        self.memory.write_byte(self.register.get_hl(), self.register.A);
        let v = self.register.get_hl().wrapping_sub(1);
        self.register.set_hl(v);
    }

    //0x33
    fn inc_sp(&mut self) {
        self.register.SP += 1;
    }

    //0x34
    fn inc_hl_ptr(&mut self) {
        let v = self.memory.read_byte(self.register.get_hl());
        let new = self.inc(v);
        self.memory.write_byte(self.register.get_hl(), new);
    }

    //0x35
    fn dec_hl_ptr(&mut self) {
        let v = self.memory.read_byte(self.register.get_hl());
        let new = self.dec(v);
        self.memory.write_byte(self.register.get_hl(), new);
    }

    //0x36
    fn ld_hl_n(&mut self, operand: u8) {
        self.memory.write_byte(self.register.get_hl(), operand);
    }

    //0x37
    fn scf(&mut self) {
        self.register.flag_set(C);
        self.register.flag_reset(H);
        self.register.flag_reset(N);
    }

    //0x38
    fn jr_c_n(&mut self, operand: i8) {
        if self.register.flag_get(C) {
            self.register.PC = ((self.register.PC as u32 as i32) + operand as i32) as u16;
        }
    }

    //0x39
    fn add_hl_sp(&mut self, operand: u16) {
        self.add_hl(operand);
    }

    //0x3a
    fn ldd_a_hl(&mut self) {
        self.register.A = self.memory.read_byte(self.register.get_hl());
        let v = self.register.get_hl().wrapping_sub(1);
        self.register.set_hl(v);
    }

    //0x3b
    fn dec_sp(&mut self) {
        self.register.SP = self.register.SP.wrapping_sub(1);
    }

    //0x3c
    fn inc_a(&mut self) {
        let a = self.register.A;
        let v = self.inc(a);
        self.register.A = v;
    }

    //0x3d
    fn dec_a(&mut self) {
        let a = self.register.A;
        let v = self.dec(a);
        self.register.A = v;
    }

    //0x3e
    fn ld_a_n(&mut self, operand: u8) {
        self.register.A = operand;
    }

    //0x3f
    fn ccf(&mut self) {
        if self.register.flag_get(C) { self.register.flag_reset(C) } else { self.register.flag_set(C) }
        self.register.flag_reset(N);
        self.register.flag_reset(H);
    }

    //0x41
    fn ld_b_c(&mut self) {
        self.register.B = self.register.C;
    }

    //0x42
    fn ld_b_d(&mut self) {
        self.register.B = self.register.D;
    }

    //0x43 
    fn ld_b_e(&mut self) {
        self.register.B = self.register.E;
    }

    //0x44
    fn ld_b_h(&mut self) {
        self.register.B = self.register.H;
    }

    //0x45
    fn ld_b_l(&mut self) {
        self.register.B = self.register.L;
    }

    //0x46
    fn ld_b_hl(&mut self) {
        self.register.B = self.memory.read_byte(self.register.get_hl());
    }

    //0x47
    fn ld_b_a(&mut self) {
        self.register.B = self.register.A;
    }

    //0x48
    fn ld_c_b(&mut self) {
        self.register.C = self.register.B;
    }

    //0x4a
    fn ld_c_d(&mut self) {
        self.register.C = self.register.D;
    }

    //0x4b
    fn ld_c_e(&mut self) {
        self.register.C = self.register.E;
    }

    //0x4c
    fn ld_c_h(&mut self) {
        self.register.C = self.register.H;
    }

    //0x4d
    fn ld_c_l(&mut self) {
        self.register.C = self.register.L;
    }

    //0x4e
    fn ld_c_hl(&mut self) {
        self.register.C = self.memory.read_byte(self.register.get_hl());
    }

    //0x4f
    fn ld_c_a(&mut self) {
        self.register.C = self.register.A;
    }

    //0x50
    fn ld_d_b(&mut self) {
        self.register.D = self.register.B;
    }

    //0x51
    fn ld_d_c(&mut self) {
        self.register.D = self.register.C;
    }

    //0x53
    fn ld_d_e(&mut self) {
        self.register.D = self.register.E;
    }

    //0x54
    fn ld_d_h(&mut self) {
        self.register.D = self.register.H;
    }

    //0x55
    fn ld_d_l(&mut self) {
        self.register.D = self.register.L;
    }

    //0x56
    fn ld_d_hl(&mut self) {
        self.register.D = self.memory.read_byte(self.register.get_hl());
    }

    //0x57
    fn ld_d_a(&mut self) {
        self.register.D = self.register.A;
    }

    //0x58
    fn ld_e_b(&mut self) {
        self.register.E = self.register.B;
    }

    //0x59
    fn ld_e_c(&mut self) {
        self.register.E = self.register.C;
    }

    //0x5a
    fn ld_e_d(&mut self) {
        self.register.E = self.register.D;
    }

    //0x5c
    fn ld_e_h(&mut self) {
        self.register.E = self.register.H;
    }

    //0x5d
    fn ld_e_l(&mut self) {
        self.register.E = self.register.L;
    }

    //0x5e
    fn ld_e_hl(&mut self) {
        self.register.E = self.memory.read_byte(self.register.get_hl());
    }

    //0x5f
    fn ld_e_a(&mut self) {
        self.register.E = self.register.A;
    }

    //0x60
    fn ld_h_b(&mut self) {
        self.register.H = self.register.B;
    }

    //0x61
    fn ld_h_c(&mut self) {
        self.register.H = self.register.C;
    }

    //0x62
    fn ld_h_d(&mut self) {
        self.register.H = self.register.D;
    }

    //0x63
    fn ld_h_e(&mut self) {
        self.register.H = self.register.E;
    }

    //0x65
    fn ld_h_l(&mut self) {
        self.register.H = self.register.L;
    }

    //0x66
    fn ld_h_hl(&mut self) {
        self.register.H = self.memory.read_byte(self.register.get_hl());
    }

    //0x67
    fn ld_h_a(&mut self) {
        self.register.H = self.register.A;
    }

    //0x68
    fn ld_l_b(&mut self) {
        self.register.L = self.register.B;
    }

    //0x69
    fn ld_l_c(&mut self) {
        self.register.L = self.register.C;
    }

    //0x6a
    fn ld_l_d(&mut self) {
        self.register.L = self.register.D;
    }

    //0x6b
    fn ld_l_e(&mut self) {
        self.register.L = self.register.E;
    }

    //0x6c
    fn ld_l_h(&mut self) {
        self.register.L = self.register.H;
    }

    //0x6e
    fn ld_l_hl(&mut self) {
        self.register.L = self.memory.read_byte(self.register.get_hl());
    }

    //0x6f
    fn ld_l_a(&mut self) {
        self.register.L = self.register.A;
    }

    //0x70
    fn ld_hl_b(&mut self) {
        self.memory.write_byte(self.register.get_hl(), self.register.B);
    }

    //0x71
    fn ld_hl_c(&mut self) {
        self.memory.write_byte(self.register.get_hl(), self.register.C);
    }

    //0x72
    fn ld_hl_d(&mut self) {
        self.memory.write_byte(self.register.get_hl(), self.register.D);
    }

    //0x73
    fn ld_hl_e(&mut self) {
        self.memory.write_byte(self.register.get_hl(), self.register.E);
    }

    //0x74
    fn ld_hl_h(&mut self) {
        self.memory.write_byte(self.register.get_hl(), self.register.H);
    }

    //0x75
    fn ld_hl_l(&mut self) {
        self.memory.write_byte(self.register.get_hl(), self.register.L);
    }

    //0x76
    fn halt(&mut self) {
        self.halted = true;
    }

    //0x77
    fn ld_hl_a(&mut self) {
        self.memory.write_byte(self.register.get_hl(), self.register.A);
    }

    //0x78
    fn ld_a_b(&mut self) {
        self.register.A = self.register.B;
    }

    //0x79
    fn ld_a_c(&mut self) {
        self.register.A = self.register.C;
    }

    //0x7a
    fn ld_a_d(&mut self) {
        self.register.A = self.register.D;
    }

    //0x7b
    fn ld_a_e(&mut self) {
        self.register.A = self.register.E;
    }

    //0x7c
    fn ld_a_h(&mut self) {
        self.register.A = self.register.H;
    }

    //0x7d
    fn ld_a_l(&mut self) {
        self.register.A = self.register.L;
    }

    //0x7e
    fn ld_a_hl(&mut self) {
        self.register.A = self.memory.read_byte(self.register.get_hl());
    }

    fn ld_a_a(&mut self) {
        self.register.A = self.register.A;
    }

    //0x80
    fn add_a_b(&mut self) {
        let v = self.register.B;
        self.add_a(v);
    }

    //0x81
    fn add_a_c(&mut self) {
        let v = self.register.C;
        self.add_a(v);
    }

    //0x82
    fn add_a_d(&mut self) {
        let v = self.register.D;
        self.add_a(v);
    }

    //0x83
    fn add_a_e(&mut self) {
        let v = self.register.E;
        self.add_a(v);
    }

    //0x84
    fn add_a_h(&mut self) {
        let v = self.register.H;
        self.add_a(v);
    }

    //0x85
    fn add_a_l(&mut self) {
        let v = self.register.L;
        self.add_a(v);
    }

    //0x86
    fn add_a_hl(&mut self) {
        let v = self.memory.read_byte(self.register.get_hl());
        self.add_a(v);
    }

    //0x87
    fn add_a_a(&mut self) {
        let v = self.register.A;
        self.add_a(v);
    }

    //0x88
    fn adc_a_b(&mut self) {
        let v = self.register.B;
        self.adc_a(v);
    }

    //0x89
    fn adc_a_c(&mut self) {
        let v = self.register.C;
        self.adc_a(v);
    }

    //0x8a
    fn adc_a_d(&mut self) {
        let v = self.register.D;
        self.adc_a(v);
    }

    //0x8b
    fn adc_a_e(&mut self) {
        let v = self.register.E;
        self.adc_a(v);
    }

    //0x8c
    fn adc_a_h(&mut self) {
        let v = self.register.H;
        self.adc_a(v);
    }

    //0x8d
    fn adc_a_l(&mut self) {
        let v = self.register.L;
        self.adc_a(v);
    }

    //0x8e
    fn adc_a_hl(&mut self) {
        let v = self.memory.read_byte(self.register.get_hl());
        self.adc_a(v);
    }

    //0x8f
    fn adc_a_a(&mut self) {
        let v = self.register.A;
        self.adc_a(v);
    }

    //0x90
    fn sub_a_b(&mut self) {
        let v = self.register.B;
        self.sub_a(v);
    }

    //0x91
    fn sub_a_c(&mut self) {
        let v = self.register.C;
        self.sub_a(v);
    }

    //0x92
    fn sub_a_d(&mut self) {
        let v = self.register.D;
        self.sub_a(v);
    }

    //0x93
    fn sub_a_e(&mut self) {
        let v = self.register.E;
        self.sub_a(v);
    }

    //0x94
    fn sub_a_h(&mut self) {
        let v = self.register.H;
        self.sub_a(v);
    }

    //0x95
    fn sub_a_l(&mut self) {
        let v = self.register.L;
        self.sub_a(v);
    }

    //0x96
    fn sub_a_hl(&mut self) {
        let v = self.memory.read_byte(self.register.get_hl());
        self.sub_a(v);
    }

    //0x97
    fn sub_a_a(&mut self) {
        let v = self.register.A;
        self.sub_a(v);
    }

    //0x98
    fn sbc_a_b(&mut self) {
        let v = self.register.B;
        self.sbc_a(v);
    }

    //0x99
    fn sbc_a_c(&mut self) {
        let v = self.register.C;
        self.sbc_a(v);
    }

    //0x9a
    fn sbc_a_d(&mut self) {
        let v = self.register.D;
        self.sbc_a(v);
    }

    //0x9b
    fn sbc_a_e(&mut self) {
        let v = self.register.E;
        self.sbc_a(v);
    }

    //0x9c
    fn sbc_a_h(&mut self) {
        let v = self.register.H;
        self.sbc_a(v);
    }

    //0x9d
    fn sbc_a_l(&mut self) {
        let v = self.register.L;
        self.sbc_a(v);
    }

    //0x9e
    fn sbc_a_hl(&mut self) {
        let v = self.memory.read_byte(self.register.get_hl());
        self.sbc_a(v);
    }

    //0x9f
    fn sbc_a_a(&mut self) {
        let v = self.register.A;
        self.sbc_a(v);
    }

    //0xa0
    fn and_b(&mut self) {
        let v = self.register.B;
        self.and(v);
    }

    //0xa1
    fn and_c(&mut self) {
        let v = self.register.C;
        self.and(v);
    }

    //0xa2
    fn and_d(&mut self) {
        let v = self.register.D;
        self.and(v);
    }

    //0xa3
    fn and_e(&mut self) {
        let v = self.register.E;
        self.and(v);
    }

    //0xa4
    fn and_h(&mut self) {
        let v = self.register.H;
        self.and(v);
    }

    //0xa5
    fn and_l(&mut self) {
        let v = self.register.L;
        self.and(v);
    }

    //0xa6
    fn and_hl(&mut self) {
        let v = self.memory.read_byte(self.register.get_hl());
        self.and(v);
    }

    //0xa7
    fn and_a(&mut self) {
        let v = self.register.A;
        self.and(v);
    }

    //0xa8
    fn xor_b(&mut self) {
        let v = self.register.B;
        self.xor(v);
    }

    //0xa9
    fn xor_c(&mut self) {
        let v = self.register.C;
        self.xor(v);
    }

    //0xaa
    fn xor_d(&mut self) {
        let v = self.register.D;
        self.xor(v);
    }

    //0xab
    fn xor_e(&mut self) {
        let v = self.register.E;
        self.xor(v);
    }

    //0xac
    fn xor_h(&mut self) {
        let v = self.register.H;
        self.xor(v);
    }

    //0xad
    fn xor_l(&mut self) {
        let v = self.register.L;
        self.xor(v);
    }

    //0xae
    fn xor_hl(&mut self) {
        let v = self.memory.read_byte(self.register.get_hl());
        self.xor(v);
    }

    //0xaf
    fn xor_a(&mut self) {
        let v = self.register.A;
        self.xor(v);
    }

    //0xb0
    fn or_b(&mut self) {
        let v = self.register.B;
        self.or(v);
    }

    //0xb1
    fn or_c(&mut self) {
        let v = self.register.C;
        self.or(v);
    }

    //0xb2
    fn or_d(&mut self) {
        let v = self.register.D;
        self.or(v);
    }

    //0xb3
    fn or_e(&mut self) {
        let v = self.register.E;
        self.or(v);
    }

    //0xb4
    fn or_h(&mut self) {
        let v = self.register.H;
        self.or(v);
    }

    //0xb5
    fn or_l(&mut self) {
        let v = self.register.L;
        self.or(v);
    }

    //0xb6
    fn or_hl(&mut self) {
        let v = self.memory.read_byte(self.register.get_hl());
        self.or(v);
    }

    //0xb7
    fn or_a(&mut self) {
        let v = self.register.A;
        self.or(v);
    }

    //0xb8
    fn cp_b(&mut self) {
        let v = self.register.B;
        self.cp(v);
    }

    //0xb9
    fn cp_c(&mut self) {
        let v = self.register.C;
        self.cp(v);
    }

    //0xba
    fn cp_d(&mut self) {
        let v = self.register.D;
        self.cp(v);
    }

    //0xbb
    fn cp_e(&mut self) {
        let v = self.register.E;
        self.cp(v);
    }

    //0xbc
    fn cp_h(&mut self) {
        let v = self.register.H;
        self.cp(v);
    }

    //0xbd
    fn cp_l(&mut self) {
        let v = self.register.L;
        self.cp(v);
    }

    //0xbe
    fn cp_hl(&mut self) {
        let v = self.memory.read_byte(self.register.get_hl());
        self.cp(v);
    }

    //0xbf
    fn cp_a(&mut self) {
        let v = self.register.A;
        self.cp(v);
    }

    //0xc0
    fn ret_nz(&mut self) {
        if !self.register.flag_get(Z) { 
            self.register.PC = self.pop_stack();
        }
    }

    //0xc1
    fn pop_bc(&mut self) {
        let v = self.pop_stack();
        self.register.set_bc(v);
    }

    //0xc2
    fn jp_nz_nn(&mut self, operand: u16) {
        if !self.register.flag_get(Z) { 
            self.register.PC = operand;
        }
    }

    //0xc3 
    fn jp_nn(&mut self, operand: u16) {
        self.register.PC = operand;
    }

    //0xc4 
    fn call_nz_nn(&mut self, operand: u16) {
        if !self.register.flag_get(Z) {
            let v = self.register.PC;
            self.push_stack(v);
            self.register.PC = operand;
        }
    }

    //0xc5
    fn push_bc(&mut self) {
        let v = self.register.get_bc();
        self.push_stack(v);
    }

    //0xc6
    fn add_a_n(&mut self, operand: u8) {
        self.add_a(operand);
    }

    //0xc7
    fn rst_0(&mut self) {
        let v = self.register.PC;
        self.push_stack(v);
        self.register.PC = 0x00;
    }

    //0xc8
    fn ret_z(&mut self) {
        if self.register.flag_get(Z) { 
            self.register.PC = self.pop_stack();
        }
    }

    //0xc9
    fn ret(&mut self) {
        self.register.PC = self.pop_stack();
    }

    //0xca
    fn jp_z_nn(&mut self, operand: u16) {
        if self.register.flag_get(Z) {
            self.register.PC = operand;
        }
    }

    //0xcc
    fn call_z_nn(&mut self, operand: u16) {
        if self.register.flag_get(Z) {
            let v = self.register.PC;
            self.push_stack(v);
            self.register.PC = operand;
        }
    }

    //0xcd
    fn call_nn(&mut self, operand: u16) {
        let v = self.register.PC;
        self.push_stack(v);
        self.register.PC = operand;
    }

    //0xce
    fn adc_a_n(&mut self, operand: u8) {
        self.add_a(operand);
    }

    //0xcf
    fn rst_8(&mut self) {
        let v = self.register.PC;
        self.push_stack(v);
        self.register.PC = 0x08;
    }

    //0xd0
    fn ret_nc(&mut self) {
        if !self.register.flag_get(C) { 
            self.register.PC = self.pop_stack();
        }
    }

    //0xd1
    fn pop_de(&mut self) {
        let v = self.pop_stack();
        self.register.set_de(v);
    }

    //0xd2
    fn jp_nc_nn(&mut self, operand: u16) {
        if !self.register.flag_get(C) { 
            self.register.PC = operand;
        }
    }

    //0xd4
    fn call_nc_nn(&mut self, operand: u16) {
        if !self.register.flag_get(C) {
            let v = self.register.PC;
            self.push_stack(v);
            self.register.PC = operand;
        }
    }

    //0xd5
    fn push_de(&mut self) {
        let v = self.register.get_de();
        self.push_stack(v);
    }

    //0xd6
    fn sub_a_n(&mut self, operand: u8) {
        self.sub_a(operand);
    }

    //0xd7
    fn rst_10(&mut self) {
        let v = self.register.PC;
        self.push_stack(v);
        self.register.PC = 0x10;
    }

    //0xd8
    fn ret_c(&mut self) {
        if self.register.flag_get(C) { 
            self.register.PC = self.pop_stack();
        }
    }

    //0xd9
    fn reti(&mut self) {
        self.memory.master = true;
        self.register.PC = self.pop_stack();
    }

    //0xda
    fn jp_c_nn(&mut self, operand: u16) {
        if self.register.flag_get(C) {
            self.register.PC = operand;
        }
    }

    //0xdc
    fn call_c_nn(&mut self, operand: u16) {
        if self.register.flag_get(C) {
            let v = self.register.PC;
            self.push_stack(v);
            self.register.PC = operand;
        }
    }

    //0xde
    fn sbc_a_n(&mut self, operand: u8) {
        self.sbc_a(operand);
    }

    //0xdf
    fn rst_18(&mut self) {
        let v = self.register.PC;
        self.push_stack(v);
        self.register.PC = 0x18;
    }

    //0xe0
    fn ldh_n_a(&mut self, operand: u8) {
        let v = 0xff00 | (operand as u16);
        let a = self.register.A;
        self.memory.write_byte(v, a);
    }

    //0xe1
    fn pop_hl(&mut self) {
        let v = self.pop_stack();
        self.register.set_hl(v);
    }

    //0xe2
    fn ldh_c_a(&mut self) {
        let v = 0xff00 | (self.register.C as u16);
        let a = self.register.A;
        self.memory.write_byte(v, a);
    }

    //0xe5
    fn push_hl(&mut self) {
        let v = self.register.get_hl();
        self.push_stack(v);
    }

    //0xe6
    fn and_n(&mut self, operand: u8) {
        self.and(operand);
    }

    //0xe7
    fn rst_20(&mut self) {
        let v = self.register.PC;
        self.push_stack(v);
        self.register.PC = 0x20;
    }

    //0xe8
    fn add_sp_n(&mut self, operand: u8) {
        let v = operand as i8 as i16 as u16;
        if (self.register.SP & 0x000f) + (v & 0x000f) > 0x000f { self.register.flag_set(H) } else { self.register.flag_reset(H) }
        if (self.register.SP & 0x00ff) + (v & 0x00ff) > 0x00ff { self.register.flag_set(C) } else { self.register.flag_reset(C) }
        self.register.flag_reset(Z);
        self.register.flag_reset(N);
        self.register.SP += v;
    }

    //0xe9
    fn jp_hl(&mut self) {
        self.register.PC = self.register.get_hl();
    }

    //0xea
    fn ld_nn_a(&mut self, operand: u16) {
        self.memory.write_byte(operand, self.register.A);
    }

    //0xee
    fn xor_n(&mut self, operand: u8) {
        self.xor(operand);
    }

    //0xef
    fn rst_28(&mut self) {
        let v = self.register.PC;
        self.push_stack(v);
        self.register.PC = 0x28;
    }

    //0xf0
    fn ldh_a_n(&mut self, operand: u8) {
        let v = (operand as u16) | 0xff00;
        self.register.A = self.memory.read_byte(v);
    }

    //0xf1
    fn pop_af(&mut self) {
        let v = self.pop_stack();
        self.register.set_af(v);
    }

    //0xf3
    fn di(&mut self) {
        self.memory.master = false;
    }
    
    //0xf5
    fn push_af(&mut self) {
        let v = self.register.get_af();
        self.push_stack(v);
    }

    //0xf6
    fn or_n(&mut self, operand: u8) {
        self.or(operand);
    }

    //0xf7
    fn rst_30(&mut self) {
        let v = self.register.PC;
        self.push_stack(v);
        self.register.PC = 0x30;
    }

    //0xf8
    fn ldhl_sp_d(&mut self, operand: u8) {
        let v = self.register.SP + operand as u16;
        if (self.register.SP & 0x000f) + (v & 0x000f) > 0x000f { self.register.flag_set(H) } else { self.register.flag_reset(H) }
        if (self.register.SP & 0x00ff) + (v & 0x00ff) > 0x00ff { self.register.flag_set(C) } else { self.register.flag_reset(C) }
        self.register.flag_reset(Z);
        self.register.flag_reset(N);
        self.register.set_hl(v);
    }

    //0xf9
    fn ld_sp_hl(&mut self) {
        self.register.SP = self.register.get_hl();
    }

    //0xfa
    fn ld_a_nn(&mut self, operand: u16) {
        self.register.A = self.memory.read_byte(operand);
    }

    //0xfb
    fn ei(&mut self) {
        self.memory.master = true;
    }

    //0xfe
    fn cp_n(&mut self, operand: u8) {
        self.cp(operand);
    }

    //0xff
    fn rst_38(&mut self) {
        let v = self.register.PC;
        self.push_stack(v);
        self.register.PC = 0x38;
    }
}