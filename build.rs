use clap_complete::{generate_to, shells::Bash};
use clap::CommandFactory;
use std::env;
use std::io::Error;

include!("src/cli.rs");

fn main() -> Result<(), Error> {
    let outdir = match env::var_os("OUT_DIR") {
        None => return Ok(()),
        Some(outdir) => outdir,
    };

    let mut cmd = Opts::command();

    let path = generate_to(
        Bash,
        &mut cmd, 
        "invogen",
        outdir,
    )?;

    println!("cargo:rerun-if-changed=src/cli.rs");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:warning=completion file is generated: {:?}", path);

    Ok(())
}
