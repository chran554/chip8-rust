extern crate rmp_serde as rmps;
extern crate serde;
//#[macro_use]
extern crate serde_derive;

use std::{cmp, env, process};
use std::fs::File;
use std::io::Read;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::str::FromStr;
use rand::Rng;

use rmps::Serializer;
//use serde::{Deserialize, Serialize};
//use rmps::{Deserializer, Serializer};
use serde::Serialize;


type Address = usize;
type Counter = u8;
type Word = u8;
type Instruction = u16;

const MEMORY_SIZE: Address = 0x0FFF + 1;
const PROGRAM_START_ADDRESS: Address = 0x200;
const FONT_START_ADDRESS: Address = 0x050;
const REGISTER_INDEX_FLAG: usize = 0xF;
const SCREEN_WIDTH: u8 = 64;
const SCREEN_HEIGHT: u8 = 32;
const VIDEO_MEMORY_SIZE: usize = 64 * 32 / 8;
const PIXEL_ON: u8 = 0x01;
const PIXEL_OFF: u8 = 0x00;

struct Chip8 {
    memory: [Word; MEMORY_SIZE],
    pc: Address,
    i: Address,
    stack: Vec<Address>,
    timer: Counter,
    //sound_timer: Counter,
    v: [Word; 16],
    video_memory: [u8; VIDEO_MEMORY_SIZE],
}

fn main() {
    let mut chip8 = Chip8::new();

    // Load a rom
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("No path to rom provided.");
        process::exit(1);
    }

    chip8.load_rom(&args[1]);
    chip8.pc = PROGRAM_START_ADDRESS;
    loop {
        chip8.execute_instruction();
    }
}

const FONT: [Word; 80] = [
    0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
    0x20, 0x60, 0x20, 0x20, 0x70, // 1
    0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
    0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
    0x90, 0x90, 0xF0, 0x10, 0x10, // 4
    0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
    0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
    0xF0, 0x10, 0x20, 0x40, 0x40, // 7
    0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
    0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
    0xF0, 0x90, 0xF0, 0x90, 0x90, // A
    0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
    0xF0, 0x80, 0x80, 0x80, 0xF0, // C
    0xE0, 0x90, 0x90, 0x90, 0xE0, // D
    0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
    0xF0, 0x80, 0xF0, 0x80, 0x80, // F
];

impl Chip8 {
    fn new() -> Self {
        let mut chip8 = Self {
            memory: [0; MEMORY_SIZE],
            pc: 0,
            i: 0,
            stack: vec![],
            timer: 0,
            // sound_timer: 0,
            v: [0; 16],
            video_memory: [0; VIDEO_MEMORY_SIZE],
        };

        // Load Font into memory at font start address
        let font_start_address = FONT_START_ADDRESS;
        let font_end_address = FONT_START_ADDRESS + FONT.len() - 1;
        chip8.memory[font_start_address..=font_end_address].copy_from_slice(&FONT);
        return chip8;
    }

    fn load_rom(&mut self, rom_file_path: &String) {
        let mut f = File::open(rom_file_path).unwrap_or_else(|e| {
            println!("{}", e);
            process::exit(1);
        });

        let mut rom_bytes = Vec::new();
        let byte_count = f.read_to_end(&mut rom_bytes).unwrap_or_else(|e| {
            println!("{}", e);
            process::exit(1);
        });

        println!("Loaded ROM file '{}' ({} bytes).", rom_file_path, byte_count);

        if byte_count > (MEMORY_SIZE - PROGRAM_START_ADDRESS) {
            panic!("ROM file '{}' is too large ({} bytes) to load into memory!", rom_file_path, byte_count)
        }

        for (i, v) in rom_bytes.iter().enumerate() {
            self.memory[PROGRAM_START_ADDRESS..][i] = *v;
        }
    }

    fn read_instruction(memory: [Word; MEMORY_SIZE], address: Address) -> Instruction {
        // Chip8 is big endian
        return ((memory[address as usize] as Instruction) << 8) | (memory[(address + 1) as usize] as Instruction);
    }

    fn execute_instruction(&mut self) {
        let instruction = Chip8::read_instruction(self.memory, self.pc);
        self.pc += 2;

        let instruction_type = ((instruction & 0xF000) >> 12) as u8;
        let x = ((instruction & 0x0F00) >> 8) as u8;
        let y = ((instruction & 0x00F0) >> 4) as u8;
        let z = ((instruction & 0x000F) >> 0) as u8;
        let n = (instruction & 0x000F) as u8;
        let nn = (instruction & 0x00FF) as u8;
        let nnn = (instruction & 0x0FFF) as u16;

        match (instruction_type, x, y, z) {
            // 00E0: Clear screen
            (0x0, 0x0, 0xE, 0x0) => {}

            // 00EE: Return from a subroutine
            (0x0, 0x0, 0xE, 0xE) => {}

            // 1NNN: Jump to address NNN
            (0x1, _, _, _) => {
                if (self.pc - 2) == nnn as usize {
                    // TODO clear screen, set sound timer to 0, set timer to 0, set key press states to 0
                    println!("Ended emulator execution due to infinite loop.");
                    process::exit(0);
                }

                self.pc = nnn as Address
            }

            // 2NNN: Jump to subroutine (see also 00EE)
            (0x2, _, _, _) => {
                self.stack.push(self.pc);
                self.pc = nnn as Address
            }

            // 3XNN: Skip next instruction if register X equals NN (see also 4XNN)
            (0x3, _, _, _) => {
                if self.v[x as usize] == (nn as Word) {
                    self.pc += 2
                }
            }

            // 4XNN: Skip next instruction if register X NOT equals NN (see also 3XNN)
            (0x4, _, _, _) => {
                if self.v[x as usize] != (nn as Word) {
                    self.pc += 2
                }
            }

            // 5XY0: Skip next instruction if register X equals register Y (see also 9XY0)
            (0x5, _, _, 0x0) => {
                if self.v[x as usize] == self.v[y as usize] {
                    self.pc += 2;
                }
            }

            // 6XNN: Set register X to value NN
            (0x6, _, _, _) => {
                self.v[x as usize] = nn as Word;
            }

            // 7XNN: Add the value NN to VX.
            (0x7, _, _, _) => {
                // NOTE: overflow flag is not affected by this instruction if result > 0xFF. If result wraps to zero when overflow i.e. VX = (VX + NN) % 0xFF.
                self.v[x as usize] = self.v[x as usize].wrapping_add(nn);
            }

            // 8XY0: Set VX to the value of VY
            (0x8, _, _, 0x0) => {
                self.v[x as usize] = self.v[y as usize];
            }

            // 8XY1: VX is set to the bitwise/binary logical disjunction (OR) of VX and VY. VY is not affected.
            (0x8, _, _, 0x1) => {
                self.v[x as usize] |= self.v[y as usize];
            }

            // 8XY2: VX is set to the bitwise/binary logical conjunction (AND) of VX and VY. VY is not affected.
            (0x8, _, _, 0x2) => {
                self.v[x as usize] &= self.v[y as usize];
            }

            // 8XY3: VX is set to the bitwise/binary exclusive OR (XOR) of VX and VY. VY is not affected.
            (0x8, _, _, 0x3) => {
                self.v[x as usize] ^= self.v[y as usize];
            }

            // 8XY4: VX is set to the value of VX plus the value of VY. VY is not affected. Carry flag in register VF is set if overflow
            (0x8, _, _, 0x4) => {
                let result = self.v[x as usize] as u16 + self.v[y as usize] as u16;
                self.v[REGISTER_INDEX_FLAG] = if result > 0xFF { 1 } else { 0 };
                self.v[x as usize] = (result % 0x100) as u8;
            }

            // 8XY5: subtract VY from VX and put the result in VX. VY is not affected.
            (0x8, _, _, 0x5) => {
                let wrapped: bool;
                (self.v[x as usize], wrapped) = self.v[x as usize].overflowing_sub(self.v[y as usize]);
                self.v[REGISTER_INDEX_FLAG] = if wrapped { 0 } else { 1 };
            }

            // 8XY6: (Strict COSMAC: Copy VY to VX and) shift VX 1 bit to the right. VF is set to the bit that was shifted out.
            (0x8, _, _, 0x6) => {
                // if configuration.ModeStrictCosmac {
                //     self.v[x as usize] = self.v[y as usize];
                // }
                self.v[REGISTER_INDEX_FLAG] = (self.v[x as usize] & 0b00000001) >> 0;
                self.v[x as usize] = self.v[x as usize] >> 1;
            }

            // 8XY7: subtract VX from VY and put the result in VX. VY is not affected.
            (0x8, _, _, 0x7) => {
                let wrapped: bool;
                (self.v[x as usize], wrapped) = self.v[y as usize].overflowing_sub(self.v[x as usize]);
                self.v[REGISTER_INDEX_FLAG] = if wrapped { 0 } else { 1 }
            }

            // 8XYE: (Strict COSMAC: Copy VY to VX and) shift VX 1 bit to the left. VF is set to the bit that was shifted out.
            (0x8, _, _, 0xE) => {
                // if configuration.ModeStrictCosmac {
                //     self.v[x as usize] = self.v[y as usize];
                // }
                self.v[REGISTER_INDEX_FLAG] = (self.v[x as usize] & 0b10000000) >> 7;
                self.v[x as usize] = self.v[x as usize] << 1;
            }

            // 9XY0: Skip next instruction if register X NOT equals register Y (see also 5XY0)
            (0x9, _, _, 0x0) => {
                if self.v[x as usize] != self.v[y as usize] {
                    self.pc += 2;
                }
            }

            // ANNN: Sets the index register I to the value NNN.
            (0xA, _, _, _) => {
                self.i = nnn as Address;
            }

            (0xB, _, _, _) => {
                //if configuration.ModeStrictCosmac || configuration.ModeRomCompatibility {
                // BNNN: Jump to the address NNN plus the value in the register V0.
                self.pc = nnn as usize + self.v[0x0] as usize;
                //} else {
                // B(X)NNN: Jump to the address NNN plus the value in the register VX.
                //   self.pc = nnn + uint16(self.v[x as usize])
                // }
            }

            // CXNN: Generates a random number, binary ANDs it with the value NN, and puts the result in VX.
            (0xC, _, _, _) => {
                let mut rng = rand::thread_rng();
                let rnd: u8 = rng.gen();
                self.v[x as usize] = (rnd & 0x00FF) & nn;
            }

            // DXYN: Draw an N pixels tall sprite from the memory location that the I-index register is holding to the screen,
            (0xD, _, _, _) => {
                // at the horizontal X coordinate in VX and the Y coordinate in VY.
                let pixel_x = self.v[x as usize] % SCREEN_WIDTH;
                let pixel_y = self.v[y as usize] % SCREEN_HEIGHT;
                self.v[REGISTER_INDEX_FLAG] = 0;

                for sprite_y in 0..n {
                    let pixel_bit_values = self.memory[self.i + sprite_y as Address];

                    for sprite_x in 0..8 {
                        if (sprite_x < SCREEN_WIDTH) & &(sprite_y < SCREEN_HEIGHT) {
                            let pixel_value = (pixel_bit_values >> sprite_x) & 0b00000001;

                            let result_pixel_value = self.xor_pixel(pixel_x + (7 - sprite_x), pixel_y + sprite_y, pixel_value);
                            if (pixel_value == 1) & &(result_pixel_value == 0) {
                                self.v[REGISTER_INDEX_FLAG] = 1;
                            }
                        }
                    }
                }

                self.update_screen()
            }

            // EX9E: Skip next instruction if key denoted by VX is pressed at the moment
            (0xE, _, 0x9, 0xE) => {
                if self.is_key_pressed(self.v[x as usize]) {
                    self.pc += 2;
                }
            }

            // EXA1: Skip next instruction if key denoted by VX is NOT pressed at the moment
            (0xE, _, 0xA, 0x1) => {
                if !self.is_key_pressed(self.v[x as usize]) {
                    self.pc += 2;
                }
            }

            // FX07: Sets VX to the current value of the delay timer
            (0xF, _, 0x0, 0x7) => {
                self.v[x as usize] = self.timer
            }

            // FX15: Sets the delay timer to the value in VX
            (0xF, _, 0x1, 0x5) => {
                self.timer = self.v[x as usize]
            }

            // FX18: Sets the sound timer to the value in VX
            (0xF, _, 0x1, 0x8) => {
                //self.UpdateSound(self.v[x as usize] > 0);
                //self.SoundTimer = remappedSoundValue(self.v[x as usize])
                //fmt.Printf("Sound on (value %d, sound timer set to %d, %d msec)\n", chip8.V[x], chip8.SoundTimer, int(math.Round(float64(chip8.SoundTimer)*1000.0/60.0)))
            }

            // FX1E: Add to index. The index register I will get the value in VX added to it.
            (0xF, _, 0x1, 0xE) => {
                let result = self.i + (self.v[x as usize]) as Address;

                //if !configuration.ModeStrictCosmac {
                //    if result > 0xFFF {
                //        // Register I would point outside memory range
                //        self.v[REGISTER_INDEX_FLAG] = 1
                //    } else {
                //        self.v[REGISTER_INDEX_FLAG] = 0
                //    }
                //}

                self.i = result & 0x0FFF
            }

            // FX0A: This instruction "blocks", it stops executing instructions and wait for key input. Value of key is stored in VX.
            (0xF, _, 0x0, 0xA) => {
                let pressed_key_code = self.get_pressed_key();
                if pressed_key_code != 0xFF {
                    self.v[x as usize] = pressed_key_code;
                } else {
                    self.pc -= 2; // Do not advance in program, do this instruction over again (loop)
                }
            }

            // FX29: Set index register to point at font character address. The character code is stored in VX
            (0xF, _, 0x2, 0x9) => {
                // Each character is 5 bytes in height
                self.i = FONT_START_ADDRESS + ((self.v[x as usize]) * 5) as usize
            }

            // FX33: Binary-coded decimal conversion
            (0xF, _, 0x3, 0x3) => {
                // It takes the number in VX and converts it to three decimal digits,
                // storing these digits in memory at the start address in the index register I.
                self.memory[self.i + 0] = (self.v[x as usize] / 100) % 10;
                self.memory[self.i + 1] = (self.v[x as usize] / 10) % 10;
                self.memory[self.i + 2] = (self.v[x as usize] / 1) % 10;
            }

            // FX55: Store V registers in memory
            (0xF, _, 0x5, 0x5) => {
                // The value of each variable register from V0 to VX inclusive
                // (if X is 0, then only V0) will be stored in successive memory addresses,
                // starting with the one that’s pointed to by register I.

                //if configuration.ModeStrictCosmac && !configuration.ModeRomCompatibility {
                //    for i: = uint8(0);(i <= x) && (i <= 0xF);i ++ {
                //        chip8.Memory[chip8.I] = chip8.V[i]
                //        chip8.I ++
                //    }
                //} else {
                for i in 0..cmp::min(x as usize, 0xF as usize) {
                    self.memory[self.i + i] = self.v[i];
                }
                //}
            }

            // FX65: Load registers from memory
            (0xF, _, 0x6, 0x5) => {
                // Takes the value stored at the memory addresses and loads them into the variable registers.
                //if configuration.ModeStrictCosmac && !configuration.ModeRomCompatibility {
                //    for i := uint8(0); (i <= x) && (i <= 0xF); i++ {
                //        chip8.V[i] = chip8.Memory[chip8.I]
                //        chip8.I++
                //    }
                //} else {
                for i in 0..cmp::min(x as usize, 0xF as usize) {
                    self.v[i] = self.memory[self.i + i];
                }
                //}
            }

            _ => {
                println!("unimplemented instruction: {:#06x} at address {:#05x}", instruction, self.pc);
                panic!("unimplemented instruction");
            }
        }
    }

    fn xor_pixel(&mut self, x: u8, y: u8, pixel_value: u8) -> u8 {
        if (x >= SCREEN_WIDTH) || (y >= SCREEN_HEIGHT) {
            return PIXEL_OFF;
        } else {
            let current_pixel_value = self.get_pixel(x, y);
            let new_pixel_value = if current_pixel_value != pixel_value { PIXEL_ON } else { PIXEL_OFF };
            self.set_pixel(x, y, new_pixel_value);
            return new_pixel_value;
        }
    }

    fn get_pixel(&mut self, x: u8, y: u8) -> u8 {
        let byte_index: usize = (y as usize * SCREEN_WIDTH as usize / 8) + (x as usize / 8);
        let bit_index: u8 = 7 - (x % 8);
        let pixel: u8 = (self.video_memory[byte_index] >> bit_index) & 0x01;

        return pixel;
    }

    fn set_pixel(&mut self, x: u8, y: u8, v: u8) {
        let byte_index: usize = (y as usize * SCREEN_WIDTH as usize / 8) + (x as usize / 8);
        let bit_index: u8 = 7 - (x % 8);
        if v == 1 {
            self.video_memory[byte_index] = self.video_memory[byte_index] | (0x01 << bit_index);
        } else if v == 0 {
            self.video_memory[byte_index] = self.video_memory[byte_index] & ((0x01 << bit_index) ^ 0xFF);
        }
    }

    fn update_screen(&mut self) {
        // self.print_screen();

        #[allow(non_snake_case)]
        #[derive(Serialize)]
        struct PeripheralStateMessage {
            sound: bool,
            keys: u16,
            screen: Vec<u8>,
            screenWidth: u8,
            screenHeight: u8,
        }

        let message = PeripheralStateMessage {
            sound: false,
            keys: 0x0000,
            screen: self.video_memory.to_vec(),
            screenWidth: 64,
            screenHeight: 32,
        };

        let mut buf = Vec::new();
        message.serialize(&mut Serializer::new(&mut buf).with_struct_map()).unwrap();

        // println!("MsgPack data size: {}", buf.len());

        send_peripheral_state(&mut buf)
    }

    /*
    fn print_screen(&mut self) {
        println!();
        for y in 0..SCREEN_HEIGHT {
            for x in 0..SCREEN_WIDTH {
                let pixel_value = self.get_pixel(x, y);
                print!("{}", if pixel_value == PIXEL_ON { "██" } else { "░░" });
            }
            println!();
        }
    }
     */

    fn is_key_pressed(&self, _key_code: Word) -> bool {
        return false;
    }

    fn get_pressed_key(&self) -> u8 {
        return 0xFF;
    }
}

fn send_peripheral_state(buf: &mut Vec<u8>) {
    let localhost_address = "192.168.1.228:9991";

    let destination_multicast_address = Ipv4Addr::from_str("224.0.0.8").expect("Could not parse IP address");
    let multicast_interface = Ipv4Addr::UNSPECIFIED;

    let socket = UdpSocket::bind(localhost_address).expect("couldn't bind to address");
    socket.join_multicast_v4(&destination_multicast_address, &multicast_interface).expect("TODO: panic message");
    socket.set_multicast_loop_v4(true).expect("rsg");
    socket.set_multicast_ttl_v4(255).expect("3regfred");
    let addr = SocketAddr::from_str("224.0.0.8:9999").expect("Could not parse IP");
    socket.send_to(&buf.as_slice(), addr).expect("TODO: panic message");
    drop(socket);
}
