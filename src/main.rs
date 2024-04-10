use std::io::Write;

use clap::Parser;
use gel_lang::{transpile_tag, Args};
use miette::IntoDiagnostic;

fn main() -> miette::Result<()> {
    let args = Args::parse();

    let mut writer = args.writer().into_diagnostic()?;

    let nodes = args.to_parser()?;
    let tags = transpile_tag(nodes);
    for tag in tags {
        write!(writer, "{tag}").into_diagnostic()?;
    }

    Ok(())
}
