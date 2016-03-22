use std::io;

// https://realboyemulator.files.wordpress.com/2013/01/gbcpuman.pdf Page 62
#[derive(Copy, Clone)]
pub enum Flags {
    Z = 0b10000000, // Flag ZERO
    N = 0b01000000, // Flag NEGATIVE
    H = 0b00100000, // Flag HALFCARRY
    C = 0b00010000, // Flag CARRY
}

pub struct Registers {
    pub A: u8,
    pub F: u8,
    pub B: u8,
    pub C: u8,
    pub D: u8,
    pub E: u8,
    pub H: u8,
    pub L: u8,
    pub SP: u16,
    pub PC: u16,
}

// https://realboyemulator.files.wordpress.com/2013/01/gbcpuman.pdf Page 17
impl Registers {
    pub fn new() -> Registers {
        Registers {
            A: 0x01,
            F: 0xb0,
            B: 0x00,
            C: 0x13,
            D: 0x00,
            E: 0xd8,
            H: 0x01,
            L: 0x4d,
            PC: 0x0100,
            SP: 0xfffe,
        }
    }

    pub fn reset(&mut self) {
        self.A = 0x01;
        self.F = 0xb0;
        self.B = 0x00;
        self.C = 0x13;
        self.D = 0x00;
        self.E = 0xd8;
        self.H = 0x01;
        self.L = 0x4d;
        self.PC = 0x0100;
        self.SP = 0xfffe;
    }

    pub fn get_af(&self) -> u16 {
        ((self.A as u16) << 8) | ((self.F & 0xf0) as u16)
    }

    pub fn get_bc(&self) -> u16 {
        ((self.B as u16) << 8) | (self.C as u16)
    }

    pub fn get_de(&self) -> u16 {
        ((self.D as u16) << 8) | (self.E as u16)
    }

    pub fn get_hl(&self) -> u16 {
        ((self.H as u16) << 8) | (self.L as u16)
    }

    pub fn set_af(&mut self, operand: u16) {
        self.A = (operand >> 8) as u8;
        self.F = (operand & 0x00f0) as u8;
    }

    pub fn set_bc(&mut self, operand: u16) {
        self.B = (operand >> 8) as u8;
        self.C = (operand & 0x00ff) as u8;
    }

    pub fn set_de(&mut self, operand: u16) {
        self.D = (operand >> 8) as u8;
        self.E = (operand & 0x00ff) as u8;
    }

    pub fn set_hl(&mut self, operand: u16) {
        self.H = (operand >> 8) as u8;
        self.L = (operand & 0x00ff) as u8;
    }

    pub fn flag_set(&mut self, flag: Flags) {
        let mask = flag as u8;
        self.F |= mask;
    }

    pub fn flag_reset(&mut self, flag: Flags) {
        let mask = flag as u8;
        self.F &= !mask;
    }

    pub fn flag_get(&self, flag: Flags) -> bool {
        let mask = flag as u8;
        self.F & mask > 0
    }

    pub fn debug_register(&self) {
        println!("AF {:X}", self.get_af());
        println!("BC {:X}", self.get_bc());
        println!("DE {:X}", self.get_de());
        println!("HL {:X}", self.get_hl());
        println!("PC {:X}", self.PC);
        println!("SP {:X}", self.SP);
        println!("Z {:?} ,N {:?}, H {:?}, C {:?}", self.flag_get(Flags::Z), self.flag_get(Flags::N), self.flag_get(Flags::H), self.flag_get(Flags::C));
        //if self.PC == 0x282a {
        //            println!("AF {:X}", self.get_af());
        //println!("BC {:X}", self.get_bc());
        //println!("DE {:X}", self.get_de());
        //println!("HL {:X}", self.get_hl());
        //println!("PC {:X}", self.PC);
        //println!("SP {:X}", self.SP);
        //println!("Z {:?} ,N {:?}, H {:?}, C {:?}", self.flag_get(Flags::Z), self.flag_get(Flags::N), self.flag_get(Flags::H), self.flag_get(Flags::C));
        //    let mut input = String::new();
        //    match io::stdin().read_line(&mut input) {
        //        Ok(n) => {  }
        //        Err(error) => println!("error: {}", error),
        //    }
        //}
    }
}