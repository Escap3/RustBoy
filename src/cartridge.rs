use std::io::prelude::*;
use std::fs::File;
use std::path;
use std::ptr::write;
use std::io;
use memory::Memory;

// https://realboyemulator.files.wordpress.com/2013/01/gbcpuman.pdf Page 11
#[derive(Debug)]
enum CartridgeType {
    RomOnly                 = 0x00,
    RomMBC1                 = 0x01,
    RomMBC1Ram              = 0x02,
    RomMBC1RamBatt          = 0x03,
    RomMBC2                 = 0x05,
    RomMBC2Batt             = 0x06,
    RomRam                  = 0x08,
    RomRamBatt              = 0x09,
    RomMMM01                = 0x0b,
    RomMMM01SRam            = 0x0c,
    RomMMM01SRamBatt        = 0x0d,
    RomMBC3TimerBatt        = 0x0f,
    RomMBC3TimerRamBatt     = 0x10,
    RomMBC3                 = 0x11,
    RomMBC3Ram              = 0x12,
    RomMBC3RamBatt          = 0x13,
    RomMBC5                 = 0x19,
    RomMBC5Ram              = 0x1a,
    RomMBC5RamBatt          = 0x1b,
    RomMBC5Rumble           = 0x1c,
    RomMBC5RumbleSRam       = 0x1d,
    RomMBC5RumbleSRamBatt   = 0x1e,
    PocketCamera            = 0x1f,
    BundaiTamas             = 0xfd,
    HudsonHUC3              = 0xfe,
    HudsonHUC1              = 0xff,
}

const ROM_TYPE_OFFSET: u16 = 0x147;
const ROM_SIZE_OFFSET: u16 = 0x148;
const ROM_NAME_OFFSET: u16 = 0x134;
const ROM_RAM_OFFSET:  u16 = 0x149;

#[derive(Debug)]
pub enum LoadError {
    LoadError,
    RomType,
    RomSize,
}

pub type LoadResult = Result<i32, LoadError>;

pub fn load_rom(filename: &str, mem: &mut Memory) -> LoadResult {       
    let mut data = vec![];

    let path = path::PathBuf::from(filename);
    try!(File::open(&path).and_then(|mut f| f.read_to_end(&mut data)).map_err(|_| LoadError::LoadError));
    if data.len() < 0x180 { 
        return Err(LoadError::RomSize)
    }

    let rom_type = data[ROM_TYPE_OFFSET as usize];
    println!("Romtype: {}", rom_type);
    // if rom_type != CartridgeType::RomOnly as u8 {
    //     return Err(LoadError::RomType)
    // } 

    let mut name = String::with_capacity(16);
    for i in 0..16 {
        match data[i + ROM_NAME_OFFSET as usize] {
            0 => break,     
            c => name.push(c as char),
        }
    }
    println!("Name: {:?}", name);
        
    let romsize = rom_size(data[ROM_SIZE_OFFSET as usize]);
    println!("Romsize: {}", romsize * 16);
    if romsize * 16 != 32 {
        return Err(LoadError::RomSize);
    }

    let ramsize = ram_size(data[ROM_RAM_OFFSET as usize]);
    println!("Ram size: {}", ramsize);

    for (idx, element) in data.into_iter().enumerate() {
        mem.write_byte(idx as u16, element);
    }

    Ok(1)
}

fn ram_size(size: u8) -> u8 {
    match size {
        0 => 0,
        1 => 2,
        2 => 8,
        3 => 32,
        4 => 128,
        _ => 0,
    }
}

fn rom_size(size: u8) -> u8 {
    match size {
        0 => 2,
        1 => 4,
        2 => 8,
        3 => 16,
        4 => 32,
        5 => 64,
        6 => 128,
        0x52 => 72,
        0x53 => 80,
        0x54 => 96,
        _ => 0,
    }
}

