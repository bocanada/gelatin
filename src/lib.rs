#![feature(iter_intersperse, box_patterns)]
mod errors;
mod gelatin;
mod transpiler;

use clap::Parser;
use gelatin::{ast::Node, GelatinParser};
use miette::IntoDiagnostic;
use std::{io, path::PathBuf};
use transpiler::{tags::Tag, Transpiler};

#[derive(Debug, Parser)]
#[command(version, about)]
pub struct Args {
    file: PathBuf,
}

impl Args {
    pub fn file_name(&self) -> &str {
        self.file.to_str().unwrap()
    }

    pub fn read_file_to_string(&self) -> io::Result<String> {
        std::fs::read_to_string(&self.file)
    }

    pub fn to_parser(self) -> miette::Result<Vec<Node>> {
        let source = self.read_file_to_string().into_diagnostic()?;
        let mut parser = GelatinParser::new(self.file_name(), &source);
        parser.parse()
    }
}

pub fn transpile_tag(input: Vec<Node>) -> Vec<Tag> {
    let mut t = Transpiler::new();

    input.into_iter().map(|node| t.as_tags(node)).collect()
}
