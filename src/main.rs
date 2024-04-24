use clap::Parser;
use gel_lang::{transpile, Args};
use miette::IntoDiagnostic;

fn main() -> miette::Result<()> {
    let args = Args::parse();

    let mut writer = args.writer().into_diagnostic()?;

    let prettify = args.prettify;
    let nodes = args.to_parser()?;
    transpile(nodes, &mut writer, prettify).into_diagnostic()?;
    // for tag in tags {
    //     write!(writer, "{tag}").into_diagnostic()?;
    // }

    Ok(())
}
