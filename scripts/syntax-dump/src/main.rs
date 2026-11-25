//! Precompiles a Sublime syntax into the syntect dump that Acta embeds, so
//! Acta itself needs no YAML loader (markdown_view/highlight.rs).
//!
//! From the repository root:
//!
//! ```text
//! cargo run --manifest-path scripts/syntax-dump/Cargo.toml -- \
//!     assets/syntaxes/powershell/PowerShell.sublime-syntax \
//!     assets/syntaxes/powershell/PowerShell.syntax.bin
//! ```
//!
//! Run it again after updating the `.sublime-syntax` file or syntect (the
//! dump is tied to syntect's version: keep this crate's pin equal to Acta's).

use syntect::parsing::SyntaxDefinition;

fn main() {
    let mut args = std::env::args().skip(1);
    let (Some(input), Some(output)) = (args.next(), args.next()) else {
        eprintln!("usage: syntax-dump <in.sublime-syntax> <out.syntax.bin>");
        std::process::exit(2);
    };
    let text = std::fs::read_to_string(&input).expect("the syntax file reads");
    // `true`: Acta parses lines with their newline (two-face's
    // `extra_newlines` set), and the syntax must be compiled the same way.
    let syntax = SyntaxDefinition::load_from_str(&text, true, None).expect("the syntax parses");
    std::fs::write(&output, syntect::dumps::dump_binary(&syntax)).expect("the dump writes");
    println!("{} -> {output}", syntax.name);
}
