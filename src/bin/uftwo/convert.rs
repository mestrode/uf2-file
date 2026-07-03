use anyhow::Error;
use clap::Parser;
use clap_num::maybe_hex;
use std::{
    ffi::OsStr,
    fs::File,
    io::{Read, Write},
    path::PathBuf,
};
use uftwo::{reader, Uf2File};

#[derive(Parser)]
pub struct Cmd {
    #[arg(value_name = "INPUT")]
    input_path: PathBuf,
    #[arg(value_name = "OUTPUT")]
    output_path: Option<PathBuf>,
    /// Target address in flash memory. (example: 0x08000000)
    #[clap(long, value_parser=maybe_hex::<u32>)]
    target_addr: u32,
    /// Family ID.
    #[clap(long)]
    family_id: Option<u32>,
}

impl Cmd {
    pub fn run(self) -> anyhow::Result<()> {
        let extension = match self.input_path.extension() {
            Some(ext) => ext,
            None => {
                return Err(Error::msg("Input file missing extension."));
            }
        };

        let input_uf2 =
            extension == OsStr::new("uf2") || extension == OsStr::new("UF2");

        let output_path = if let Some(path) = self.output_path {
            path
        } else {
            let mut path = self.input_path.clone();

            if !input_uf2 {
                // add extension
                path.set_extension("uf2");
            } else {
                path.set_extension("bin");
            }

            path
        };

        println!("Converting {:?} to {:?}", self.input_path, output_path);

        if input_uf2 {
            uf2_to_bin(self.input_path, output_path)
        } else {
            bin_to_uf2(
                self.input_path,
                output_path,
                self.target_addr,
                self.family_id,
            )
        }
    }
}

/// Binary to UF2.
fn bin_to_uf2(
    input: PathBuf,
    output: PathBuf,
    target_addr: u32,
    family_id: Option<u32>,
) -> anyhow::Result<()> {
    let mut input_file = File::open(input)?;
    let mut output_file = File::create(output)?;

    let mut binary = Vec::new();
    input_file.read_to_end(&mut binary)?;

    let mut uf2_file = Uf2File::new();
    uf2_file.add_payload(&binary, family_id)?;
    uf2_file.set_target_addresses(target_addr);

    uf2_file.to_writer(&mut output_file)?;

    println!(
        "Written {} bytes into {} blocks.",
        binary.len(),
        uf2_file.len()
    );

    output_file.flush()?;

    Ok(())
}

/// UF2 to binary.
fn uf2_to_bin(input: PathBuf, output: PathBuf) -> anyhow::Result<()> {
    let mut output_file = File::create(output)?;

    let uf2_file = reader::from_bytes(&std::fs::read(&input)?)?;

    let payload = uf2_file
        .get_payload(None)
        .ok_or_else(|| Error::msg("No payload found"))?;

    output_file.write_all(&payload)?;

    println!(
        "Read {} bytes from {} blocks.",
        payload.len(),
        uf2_file.len()
    );

    output_file.flush()?;

    Ok(())
}
