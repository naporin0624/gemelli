//! Grabs one frame from a capture device and writes it as a PPM file so the
//! decode path (format negotiation, stride handling, channel order) can be
//! inspected visually. Usage: `cargo run -p gemelli-core --example dump_frame -- <index> <out.ppm>`
use gemelli_core::capture::{CaptureSource, NokhwaSource, list_devices};
use gemelli_core::selector::{DeviceSelector, device_line};
use std::io::Write;

fn main() {
    let mut args = std::env::args().skip(1);
    let index: u32 = args.next().and_then(|v| v.parse().ok()).unwrap_or(0);
    let out_path = args.next().unwrap_or_else(|| "frame.ppm".to_string());

    let devices = match list_devices() {
        Ok(devices) => {
            for device in &devices {
                println!("device {}", device_line(device));
            }
            devices
        }
        Err(error) => {
            eprintln!("list failed: {error}");
            std::process::exit(1);
        }
    };
    println!("opening index {index}");

    let device = match DeviceSelector::Index(index).resolve(&devices) {
        Ok(device) => device,
        Err(error) => {
            eprintln!("select failed: {error}");
            std::process::exit(1);
        }
    };

    let mut source = match NokhwaSource::open(device, None) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("open failed: {error}");
            std::process::exit(1);
        }
    };

    // The first frames may arrive before the stream settles; take a few.
    let mut frame = None;
    for _ in 0..5 {
        match source.next_frame() {
            Ok(captured) => frame = Some(captured),
            Err(error) => {
                eprintln!("frame read failed: {error}");
                std::process::exit(1);
            }
        }
    }
    let Some(frame) = frame else {
        eprintln!("no frame captured");
        std::process::exit(1);
    };

    println!("frame: {}x{} ({} bytes BGRA)", frame.width(), frame.height(), frame.data().len());

    let rgb: Vec<u8> = frame.data().chunks_exact(4).flat_map(|px| [px[2], px[1], px[0]]).collect();
    let written = std::fs::File::create(&out_path).and_then(|mut file| {
        writeln!(file, "P6\n{} {}\n255", frame.width(), frame.height())?;
        file.write_all(&rgb)
    });
    if let Err(error) = written {
        eprintln!("write failed: {error}");
        std::process::exit(1);
    }
    println!("wrote {out_path}");
}
