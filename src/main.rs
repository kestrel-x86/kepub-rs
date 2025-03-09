mod converter;
mod errors;
mod lmnt;
use std::{fs::File, io::ErrorKind, path::Path};

use clap::Parser;
use errors::{io_err, ConverterError};
use zip::ZipArchive;

#[derive(Parser)]
struct Args {
    // Input epub zip
    input: String,

    /// Output directory
    out_dir: String,

    /// Remove calibre metadata
    #[arg(long, default_value_t = false)]
    strip_calibre: bool,
}

fn main() -> Result<(), ConverterError> {
    let mut args = Args::parse();
    if !std::fs::metadata(&args.input).is_ok_and(|m| m.is_file()) {
        return Err(io_err!(
            ErrorKind::NotFound,
            "Path {} does not exist or is not a file",
            args.input
        ));
    }

    // If dest is empty, set to parent dir of input file
    if args.out_dir.is_empty() {
        let p = Path::new(&args.input);
        args.out_dir = match p.parent().and_then(|pd| pd.to_str()) {
            Some(d) => d.to_string(),
            None => {
                return Err(io_err!(
                    ErrorKind::Other,
                    "Cannot get parent directory of file {}",
                    args.input
                ));
            }
        };
    }

    let out_path = get_out_file_path(&args)?;
    let in_file = File::open(args.input)?;
    let mut zip_arch = ZipArchive::new(in_file)?;

    let conv = converter::Converter::new()?;
    conv.convert(&mut zip_arch, &out_path)?;

    return Ok(());
}

fn get_out_file_path(args: &Args) -> Result<String, ConverterError> {
    let og_fname = match Path::new(&args.input)
        .file_name()
        .and_then(|oss| oss.to_str())
    {
        Some(s) => s,
        None => {
            return Err(io_err!(
                std::io::ErrorKind::Other,
                "Unable to separate filename from {}",
                args.input
            ))
        }
    };

    let mut out_fname = Path::new(&args.out_dir).join(og_fname);
    out_fname.set_extension("kepub");
    return match out_fname.to_str() {
        Some(o) => Ok(o.to_string()),
        None => Err(io_err!(
            std::io::ErrorKind::InvalidData,
            "Unable to create output file path"
        )),
    };
}
