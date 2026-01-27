use portable_pty::{native_pty_system,CommandBuilder, PtySize};
use std::io::{Read, Write};

use std::thread;


fn main() -> anyhow::Result<()>{
    let pty_system = native_pty_system();

    let pair = pty_system.openpty(PtySize{
        rows: 24,
        cols:80,
        pixel_width:0,
        pixel_height:0,
    })?;

    let  cmd = CommandBuilder::new("/bin/bash");

    let _child = pair.slave.spawn_command(cmd)?;

    let mut reader = pair.master.try_clone_reader()?;
    let mut writer = pair.master.take_writer()?;
    

    thread::spawn(move || {
        let mut buffer = [0u8; 1024];
        loop {
            let n = reader.read(&mut buffer).unwrap();
            print!("{}", String::from_utf8_lossy(&buffer[..n]));
        }
    });

    // Forward user input to shell
    let stdin = std::io::stdin();
    let mut input = String::new();
    loop {
        input.clear();
        stdin.read_line(&mut input)?;
        writer.write_all(input.as_bytes())?;
    }


}
