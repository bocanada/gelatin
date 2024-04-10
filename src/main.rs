use clap::Parser;
use gel_lang::{transpile_tag, Args};

fn main() -> miette::Result<()> {
    let args = Args::parse();

    let nodes = args.to_parser()?;
    let tags = transpile_tag(nodes);
    for tag in tags {
        println!("{tag}");
    }

    Ok(())
}
