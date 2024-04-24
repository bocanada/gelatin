#![feature(iter_intersperse, box_patterns)]
#![warn(clippy::pedantic, clippy::nursery)]
mod errors;
mod gelatin;
mod transpiler;

use clap::{Parser as ClapParser, ValueEnum};
use gelatin::{ast::Node, Parser};
use miette::IntoDiagnostic;
use sqlparser::dialect::{GenericDialect, MsSqlDialect, PostgreSqlDialect};
use std::{io, path::PathBuf};
use transpiler::Transpiler;

#[derive(ValueEnum, Copy, Clone, Debug, PartialEq, Eq)]
pub enum SqlDialect {
    /// A generic SQL dialect.
    Generic,
    /// Postgres SQL dialect.
    Pg,
    /// SQL Server dialect.
    Mssql,
}

#[derive(Debug, ClapParser)]
#[command(version, about)]
pub struct Args {
    file: PathBuf,

    /// Where to output the GEL script to.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Whether to prettify the output or not.
    #[arg(short, long)]
    pub prettify: bool,

    /// SQL dialect to parse queries.
    #[arg(short, long, default_value_t = SqlDialect::Generic)]
    pub dialect: SqlDialect,
}

impl Args {
    /// # Panics
    /// This method may panic if the file is not a valid utf-8 string.
    #[must_use]
    pub fn file_name(&self) -> &str {
        self.file.to_str().expect("a valid file name")
    }

    /// # Errors
    /// Returns `Err` if the output file cannot be opened.
    pub fn writer(&self) -> io::Result<io::BufWriter<Box<dyn io::Write>>> {
        if let Some(ref output) = self.output {
            let output = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(output)?;
            Ok(io::BufWriter::new(Box::new(output)))
        } else {
            let stdout = std::io::stdout();
            let stdout = stdout.lock();
            Ok(io::BufWriter::new(Box::new(stdout)))
        }
    }

    /// # Errors
    /// Returns `Err` if the input file cannot be read.
    pub fn read_file_to_string(&self) -> io::Result<String> {
        std::fs::read_to_string(&self.file)
    }

    /// # Errors
    /// Returns `Err` if the parsing fails.
    pub fn to_parser(self) -> miette::Result<Vec<Node>> {
        let source = self.read_file_to_string().into_diagnostic()?;
        match self.dialect {
            SqlDialect::Generic => {
                Parser::new_with_dialect(self.file_name(), &source, GenericDialect {}).parse()
            }
            SqlDialect::Pg => {
                Parser::new_with_dialect(self.file_name(), &source, PostgreSqlDialect {}).parse()
            }
            SqlDialect::Mssql => {
                Parser::new_with_dialect(self.file_name(), &source, MsSqlDialect {}).parse()
            }
        }
    }
}

impl std::fmt::Display for SqlDialect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.to_possible_value()
            .expect("no values are skipped")
            .get_name()
            .fmt(f)
    }
}

/// # Errors
/// Returns `Err` if the write to `sink` fails.
pub fn transpile<W>(input: Vec<Node>, sink: W, prettify: bool) -> xml::writer::Result<()>
where
    W: io::Write,
{
    let mut t = Transpiler::new(sink, prettify);

    t.transpile(input)
}
