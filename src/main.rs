//! Command line entry point, the cavalry binary.
//!
//! Subcommands
//!   capture  store one rescue record from an input file
//!   list     print the book as a list
//!   propose  print the proposals a review works from
//!   confirm  store the confirmation a review settled
//!   queue    print the records still waiting for a human
//!
//! The argument parsing is clap, adopted under the maintainer's
//! tool choice ruling. Before clap sees the arguments, a
//! preprocess walks them the way the Python parser's own scan
//! walked them. It joins onto their flags the values that scan
//! would have taken, fires help where that scan fired it, and
//! gathers the positional values in the order they stand, so
//! the functional outcomes of the two command lines agree, the
//! same subcommands and arguments, misuse at exit status 2,
//! help at exit status 0, and unambiguous long flag
//! abbreviations still executing. Help text and malformed
//! invocation error text are clap's own forms. The documents
//! the tool emits and the one error line discipline for its own
//! refusals are unchanged. A failure the Python tool lets
//! escape uncaught answers at status 1, and a standard output
//! failure that surfaces only at its exit flush answers at
//! status 120, while one that surfaces during the write is a
//! system error the Python command line catches and answers at
//! status 2 with the platform error line. Errors in main ride
//! on anyhow. The library's error types derive thiserror.

use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, CommandFactory, Parser, Subcommand};

use cavalry::judgment::{FixturePlayer, JudgmentError, JudgmentSource};
use cavalry::pyjson::{Value, decimal_digit_value};
use cavalry::records::{self, RecordError};

#[derive(Parser)]
#[command(
    subcommand_required = true,
    infer_long_args = true,
    disable_help_subcommand = true
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Store one rescue record from an input file.
    #[command(infer_long_args = true, args_override_self = true)]
    Capture(CaptureArgs),
    /// Print the book as a list.
    #[command(infer_long_args = true, args_override_self = true)]
    List(ListArgs),
    /// Print the proposals a review works from.
    #[command(infer_long_args = true, args_override_self = true)]
    Propose(ProposeArgs),
    /// Store the confirmation a review settled.
    #[command(infer_long_args = true, args_override_self = true)]
    Confirm(ConfirmArgs),
    /// Print the records still waiting for a human.
    #[command(infer_long_args = true, args_override_self = true)]
    Queue(QueueArgs),
}

/// The value parser of a positional path argument, reading
/// the value as the path it stands for, the empty string
/// included. The path parser clap derives refuses an empty
/// value in its own voice, ahead of the tool, whose path
/// discipline answers an empty path in the path library's
/// form, as the current directory. A positional path is
/// therefore read by this parser, so the empty value reaches
/// the tool's own handling in every positional slot, and a
/// value whose bytes are not text reaches it unchanged, as
/// it reaches the Python parser.
#[derive(Clone, Debug)]
struct PathValueParser;

impl clap::builder::TypedValueParser for PathValueParser {
    type Value = PathBuf;

    fn parse_ref(
        &self,
        _cmd: &clap::Command,
        _arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        Ok(PathBuf::from(value))
    }
}

#[derive(Args)]
struct CaptureArgs {
    /// Path to the rescue record JSON file to store.
    #[arg(value_parser = PathValueParser)]
    input_file: PathBuf,
    /// Path to the records file to append to.
    #[arg(value_parser = PathValueParser)]
    records_file: PathBuf,
    /// Path to a directory of recorded judgment fixtures.
    #[arg(long)]
    fixtures: Option<PathBuf>,
}

#[derive(Args)]
struct ListArgs {
    /// Path to the records file to read.
    #[arg(value_parser = PathValueParser)]
    records_file: PathBuf,
}

#[derive(Args)]
struct ProposeArgs {
    /// Path to the records file to read.
    #[arg(value_parser = PathValueParser)]
    records_file: PathBuf,
}

#[derive(Args)]
struct QueueArgs {
    /// Path to the records file to read.
    #[arg(value_parser = PathValueParser)]
    records_file: PathBuf,
}

#[derive(Args)]
struct ConfirmArgs {
    /// Path to the confirmation JSON file to store.
    #[arg(value_parser = PathValueParser)]
    confirmation_file: PathBuf,
    /// Path to the records file to append to.
    #[arg(value_parser = PathValueParser)]
    records_file: PathBuf,
}

/// A failure the Python tool lets escape uncaught.
///
/// The binary carries the failure in its own type and main
/// answers it with status 1 and one error line, where every
/// other refusal answers with status 2.
#[derive(Debug)]
struct Uncaught(String);

impl std::fmt::Display for Uncaught {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Uncaught {}

/// A standard output failure the Python tool discovers only
/// when its buffer is flushed as the tool exits.
///
/// The Python tool holds its printed output in a buffer of
/// 8192 bytes, so a failure under a smaller output is never
/// discovered during the run. It surfaces at the shutdown
/// flush, where the Python tool reports it as an exception
/// ignored and exits with status 120 and no traceback, so
/// the binary carries the failure in its own type and main
/// answers it with status 120 and one error line.
#[derive(Debug)]
struct FailedFlush(String);

impl std::fmt::Display for FailedFlush {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for FailedFlush {}

/// The size of the buffer the Python tool holds its printed
/// output in, in bytes. A printed output of this size or more
/// is written through as it is printed, so a failure under it
/// surfaces during the run, where the Python command line
/// catches it as a system error and answers with the platform
/// error line at status 2, and a smaller output is flushed
/// only as the tool exits.
const STDOUT_BUFFER_BYTES: usize = 8192;

fn print_out(text: &str) -> anyhow::Result<()> {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    lock.write_all(text.as_bytes())
        .and_then(|()| lock.flush())
        .map_err(|error| {
            if text.len() < STDOUT_BUFFER_BYTES {
                anyhow::Error::new(FailedFlush(format!("stdout: {}", error)))
            } else {
                match error.raw_os_error() {
                    Some(errno) => anyhow::Error::msg(records::system_line(errno, None)),
                    None => anyhow::Error::msg(format!("stdout: {}", error)),
                }
            }
        })
}

/// Carry a records error into main, keeping the uncaught kind
/// apart so the exit status follows it.
fn records_failure(error: RecordError) -> anyhow::Error {
    match error {
        RecordError::Uncaught(message) => anyhow::Error::new(Uncaught(message)),
        other => anyhow::Error::new(other),
    }
}

/// Carry a judgment error into main, the same division.
fn judgment_failure(error: JudgmentError) -> anyhow::Error {
    match error {
        JudgmentError::Uncaught(message) => anyhow::Error::new(Uncaught(message)),
        other => anyhow::Error::new(other),
    }
}

fn cmd_capture(args: &CaptureArgs) -> anyhow::Result<i32> {
    // An empty fixtures value reads exactly as no fixtures flag
    // at all, as the Python command line reads it, and the
    // player loads before the input is read, as it loads there.
    let player = match &args.fixtures {
        Some(directory) if !directory.as_os_str().is_empty() => {
            Some(FixturePlayer::load(directory).map_err(judgment_failure)?)
        }
        _ => None,
    };
    let source: Option<&dyn JudgmentSource> =
        player.as_ref().map(|item| item as &dyn JudgmentSource);
    let stored =
        records::capture(&args.input_file, &args.records_file, source).map_err(records_failure)?;
    let field = |name: &str| stored.get(name).and_then(Value::as_str).unwrap_or("");
    print_out(&format!(
        "stored {} as {}\n",
        field("id"),
        field("rescue_class")
    ))?;
    Ok(0)
}

fn cmd_list(args: &ListArgs) -> anyhow::Result<i32> {
    let records_read = records::read_records(&args.records_file).map_err(records_failure)?;
    print_out(&records::render_list(&records_read))?;
    Ok(0)
}

fn cmd_propose(args: &ProposeArgs) -> anyhow::Result<i32> {
    let records_read = records::read_records(&args.records_file).map_err(records_failure)?;
    let class_list = records::shipped_class_list();
    let text = records::render_proposals(&records_read, &class_list).map_err(records_failure)?;
    print_out(&text)?;
    Ok(0)
}

fn cmd_queue(args: &QueueArgs) -> anyhow::Result<i32> {
    let records_read = records::read_records(&args.records_file).map_err(records_failure)?;
    print_out(&records::render_queue(&records_read))?;
    Ok(0)
}

fn cmd_confirm(args: &ConfirmArgs) -> anyhow::Result<i32> {
    let entry =
        records::confirm(&args.confirmation_file, &args.records_file).map_err(records_failure)?;
    let field = |name: &str| entry.get(name).and_then(Value::as_str).unwrap_or("");
    print_out(&format!(
        "confirmed {} as {}\n",
        field("corrects"),
        field("confirmed_class")
    ))?;
    Ok(0)
}

/// The exit status an error from a command answers with.
///
/// A failure the Python tool lets escape uncaught answers at
/// status 1. A standard output failure discovered only at the
/// exit flush answers with status 120, the status the Python
/// tool exits with when it reports that failure as an exception
/// ignored. Every other refusal the tool makes answers with
/// status 2.
fn refusal_status(error: &anyhow::Error) -> u8 {
    if error.downcast_ref::<FailedFlush>().is_some() {
        return 120;
    }
    if error.downcast_ref::<Uncaught>().is_some() {
        return 1;
    }
    2
}

/// Whether the token after its dash reads as a negative number
/// under the argparse pattern, one run of decimal digits, or a
/// run of digits, a point, and another run. The digit set is the
/// full Unicode decimal set, as it is in a Python text pattern.
fn is_negative_number(token: &str) -> bool {
    let Some(body) = token.strip_prefix('-') else {
        return false;
    };
    let digit = |ch: char| decimal_digit_value(ch).is_some();
    if !body.is_empty() && body.chars().all(digit) {
        return true;
    }
    match body.split_once('.') {
        Some((whole, fraction)) => {
            whole.chars().all(digit) && !fraction.is_empty() && fraction.chars().all(digit)
        }
        None => false,
    }
}

/// Whether the negative number pattern matches the token, where
/// the pattern's end anchor also matches just before one
/// trailing newline, as a Python text pattern's does.
fn negative_number_matches(token: &str) -> bool {
    is_negative_number(token) || token.strip_suffix('\n').is_some_and(is_negative_number)
}

/// Whether a token reads as a positional string rather than as
/// an option, the classification the Python parser makes when it
/// meets the token. A bare word and a lone dash are positional.
/// A dash token carrying a space reads as positional whatever
/// its prefix, the one exception being the short help family,
/// whose letter after the dash decides ahead of the space rule,
/// as the Python parser refuses a help token with a trailing
/// word in a value position rather than consuming it. A dash
/// token without a space is positional only when it matches
/// the negative number pattern. Any long token without a space
/// is an option.
fn reads_as_positional(token: &str) -> bool {
    if token == "--" {
        return false;
    }
    if !token.starts_with('-') || token == "-" {
        return true;
    }
    if token.chars().nth(1) == Some('h') {
        return false;
    }
    if token.contains(' ') {
        return true;
    }
    if token.starts_with("--") {
        return false;
    }
    negative_number_matches(token)
}

/// One subcommand of the command line, with the long flags the
/// derive model declares for it.
///
/// The flag tables of the preprocess come from clap's own model
/// of the derive structs, read through the command factory, so
/// the derive model stays the one owner of the flag set and this
/// layer restates none of it.
struct SubcommandSpec {
    name: String,
    value_flags: Vec<String>,
    longs: Vec<String>,
}

fn subcommand_specs() -> Vec<SubcommandSpec> {
    Cli::command()
        .get_subcommands()
        .map(|sub| {
            let mut value_flags: Vec<String> = Vec::new();
            let mut longs: Vec<String> = Vec::new();
            for arg in sub.get_arguments() {
                let Some(long) = arg.get_long() else {
                    continue;
                };
                let name = format!("--{}", long);
                if !longs.contains(&name) {
                    longs.push(name.clone());
                }
                if matches!(arg.get_action(), clap::ArgAction::Set) {
                    value_flags.push(name.clone());
                }
            }
            if !longs.iter().any(|long| long == "--help") {
                longs.push("--help".to_string());
            }
            SubcommandSpec {
                name: sub.get_name().to_string(),
                value_flags,
                longs,
            }
        })
        .collect()
}

/// How a token reads as a help token on the current parser.
enum HelpRead {
    /// Not a help token.
    No,
    /// The help action fires.
    Fire,
    /// A help token carrying an explicit value, an error in both
    /// parsers. The scan carries the token in the form clap
    /// refuses in its own voice, and nothing later can fire help.
    Refuse(OsString),
}

/// Read a token as a help token against the long flags of the
/// parser the scan stands on. The Python parser fires help when
/// its scan reaches the token, ahead of extra tokens it has
/// already collected, and it refuses a help token that carries an
/// explicit value, ahead of any help token that follows. A short
/// help token fires when its bundle continues with anything but
/// an equals sign or a dash, the two continuations the parser
/// reads as an explicit value arriving with the token.
fn help_read(token: &str, longs: &[String]) -> HelpRead {
    if token.starts_with("--") {
        let name = token.split('=').next().unwrap_or(token);
        if name.len() > 2 {
            let matches: Vec<&String> =
                longs.iter().filter(|long| long.starts_with(name)).collect();
            if matches.len() == 1 && matches[0] == "--help" {
                if token.contains('=') {
                    return HelpRead::Refuse(OsString::from(token));
                }
                return HelpRead::Fire;
            }
        }
        return HelpRead::No;
    }
    if token.starts_with('-') && token.chars().nth(1) == Some('h') {
        return match token.chars().nth(2) {
            None => HelpRead::Fire,
            Some('=') => HelpRead::Refuse(OsString::from(format!("--help={}", &token[3..]))),
            Some('-') => HelpRead::Refuse(OsString::from(format!("--help={}", &token[2..]))),
            Some(_) => HelpRead::Fire,
        };
    }
    HelpRead::No
}

/// The outcome of the preprocessed argument walk.
enum Preprocess {
    /// Hand these arguments to clap.
    Run(Vec<OsString>),
    /// Print the help of the named subcommand parser, or of the
    /// top level parser when the name is absent, at status 0.
    FireHelp(Option<String>),
}

/// Walk the arguments the way the Python parser's scan walked
/// them and prepare them for clap.
///
/// The Python parser consumes its arguments from the left. A
/// value flag takes the next token when that token reads as a
/// positional string, and this walk joins such a value onto its
/// flag in the equals form. Positional values are gathered in
/// the order they stand and handed to clap behind a separator,
/// so a value that reads as an option string reaches its slot
/// as a value, and the separator itself is consumed wherever it
/// stands, one separator only, with every token after it a
/// positional value, a second separator among them. A value
/// flag that meets an option token or the separator is an error
/// the parser exits on at once, as are a word the subcommand
/// slot cannot take and a help token carrying an explicit
/// value, and past any of those this walk hands the arguments
/// to clap as they stand, since clap refuses the same
/// invocation in its own voice. An option token the current
/// parser does not declare is only collected by the scan, ahead
/// of the help check on the tokens after it, and it stands in
/// the stream handed to clap, which refuses it there. Help
/// fires where the scan reaches it, ahead of extra tokens
/// already collected, and this walk answers a fired help token
/// itself.
fn preprocess(tokens: &[OsString]) -> Preprocess {
    let specs = subcommand_specs();
    let root_longs: Vec<String> = vec!["--help".to_string()];
    let mut raw: Vec<OsString> = Vec::new();
    let mut out: Vec<OsString> = Vec::new();
    let mut values: Vec<OsString> = Vec::new();
    let mut current: Option<usize> = None;
    let mut pending: Option<usize> = None;
    // The place in the out stream of the value flag's standing
    // token. The command line declares one value flag, and an
    // empty value for it reads as no flag at all while a later
    // flag still overrides an earlier one, so the walk keeps
    // the standing token replaceable. Clap refuses an empty
    // attached value in its own voice, so an empty value is
    // carried as the flag's absence rather than as a token.
    let mut value_slot: Option<usize> = None;
    let mut blocked = false;
    let mut separated = false;
    for token in tokens {
        if separated {
            raw.push(token.clone());
            values.push(token.clone());
            continue;
        }
        if blocked {
            raw.push(token.clone());
            continue;
        }
        let text = token.to_string_lossy().into_owned();
        if let Some(slot) = pending {
            raw.push(token.clone());
            if text != "--" && reads_as_positional(&text) {
                pending = None;
                if text.is_empty() {
                    out.remove(slot);
                    value_slot = None;
                } else {
                    let mut joined = out[slot].clone();
                    joined.push("=");
                    joined.push(token);
                    out[slot] = joined;
                }
            } else {
                blocked = true;
            }
            continue;
        }
        if text == "--" {
            raw.push(token.clone());
            separated = true;
            continue;
        }
        let longs: &[String] = match current {
            Some(index) => &specs[index].longs,
            None => &root_longs,
        };
        match help_read(&text, longs) {
            HelpRead::Fire => {
                return Preprocess::FireHelp(current.map(|index| specs[index].name.clone()));
            }
            HelpRead::Refuse(replacement) => {
                blocked = true;
                raw.push(replacement);
                continue;
            }
            HelpRead::No => {}
        }
        raw.push(token.clone());
        if current.is_none() {
            if let Some(index) = specs.iter().position(|spec| spec.name == text) {
                current = Some(index);
                out.push(token.clone());
                continue;
            }
            if reads_as_positional(&text) {
                blocked = true;
            } else {
                // An option token at the top level is only
                // collected, and a help token after it still
                // fires.
                out.push(token.clone());
            }
            continue;
        }
        let spec = &specs[current.expect("a subcommand is selected")];
        if text.starts_with("--") {
            let name = text.split('=').next().unwrap_or(&text);
            let matched: Vec<&String> = spec
                .value_flags
                .iter()
                .filter(|flag| flag.starts_with(name))
                .collect();
            if matched.len() == 1 {
                match text.split_once('=') {
                    Some((_, value)) => {
                        // The attached form. An empty value
                        // clears any standing token and stands as
                        // no flag, and a full one replaces it.
                        if let Some(old) = value_slot.take() {
                            out.remove(old);
                        }
                        if !value.is_empty() {
                            out.push(token.clone());
                            value_slot = Some(out.len() - 1);
                        }
                    }
                    None => {
                        if let Some(old) = value_slot.take() {
                            out.remove(old);
                        }
                        out.push(token.clone());
                        pending = Some(out.len() - 1);
                        value_slot = Some(out.len() - 1);
                    }
                }
                continue;
            }
            // A long token no declared flag takes hold of is
            // a value when it reads as a positional string, as
            // the Python parser reads a token carrying a space
            // as a value whatever its prefix. The space test
            // stands ahead of the option reading here, as it
            // stands in the classification itself, or a file
            // named with two dashes and a space could never
            // reach its slot.
            if reads_as_positional(&text) {
                values.push(token.clone());
                continue;
            }
            // An option this parser does not declare is only
            // collected, and clap refuses it in the stream.
            out.push(token.clone());
            continue;
        }
        if reads_as_positional(&text) {
            values.push(token.clone());
        } else {
            // A short option token this parser does not declare
            // is likewise only collected.
            out.push(token.clone());
        }
    }
    if blocked || pending.is_some() {
        return Preprocess::Run(raw);
    }
    if !values.is_empty() {
        out.push(OsString::from("--"));
        out.extend(values);
    }
    Preprocess::Run(out)
}

/// Answer a clap error the way the command line answers it.
///
/// An error clap prints to standard error, a malformed
/// invocation, is printed there and answered with its own exit
/// code. An error clap prints to standard output is the help
/// text, and help output takes the failed write discipline of
/// every document the tool prints, because the Python tool's
/// help goes to the same buffered standard output its documents
/// do. The help is rendered and passed through print_out, so a
/// standard output that cannot take the bytes answers at the
/// status the failure family carries, 120 under a small output
/// and the platform error line at status 2 under a larger one,
/// with the tool's one error line, rather than exiting 0 over a
/// print whose failure was discarded.
fn answer_clap_error(error: clap::Error) -> ExitCode {
    if error.use_stderr() {
        let code = error.exit_code();
        let _ = error.print();
        return ExitCode::from(code as u8);
    }
    let text = error.render().to_string();
    match print_out(&text) {
        Ok(()) => ExitCode::from(error.exit_code() as u8),
        Err(failure) => {
            eprintln!("error: {}", failure);
            ExitCode::from(refusal_status(&failure))
        }
    }
}

fn main() -> ExitCode {
    // The Python reader reaches documents nested thousands of
    // containers deep before its own ceiling refuses them, and
    // the port's reader, writer, and value walks recurse once per
    // container on the deepest path, as the Python tool's do. The
    // binary therefore runs on a worker thread with a stack sized
    // for that depth, so a document at the ceiling is refused or
    // carried on the tool's own terms and never on the stack's.
    match std::thread::Builder::new()
        .stack_size(512 * 1024 * 1024)
        .spawn(run)
    {
        Ok(handle) => handle.join().unwrap_or(ExitCode::from(2)),
        Err(_) => run(),
    }
}

fn run() -> ExitCode {
    let mut args = std::env::args_os();
    let program = args.next().unwrap_or_else(|| OsString::from("cavalry"));
    let rest: Vec<OsString> = args.collect();
    let joined = match preprocess(&rest) {
        Preprocess::Run(joined) => joined,
        Preprocess::FireHelp(subcommand) => {
            let mut argv = vec![program];
            if let Some(name) = subcommand {
                argv.push(OsString::from(name));
            }
            argv.push(OsString::from("--help"));
            return match Cli::try_parse_from(argv) {
                Ok(_) => ExitCode::from(0),
                Err(error) => answer_clap_error(error),
            };
        }
    };
    let cli = match Cli::try_parse_from(std::iter::once(program).chain(joined)) {
        Ok(cli) => cli,
        Err(error) => return answer_clap_error(error),
    };
    let outcome = match &cli.command {
        Command::Capture(args) => cmd_capture(args),
        Command::List(args) => cmd_list(args),
        Command::Propose(args) => cmd_propose(args),
        Command::Confirm(args) => cmd_confirm(args),
        Command::Queue(args) => cmd_queue(args),
    };
    match outcome {
        Ok(code) => ExitCode::from(code as u8),
        Err(error) => {
            eprintln!("error: {}", error);
            ExitCode::from(refusal_status(&error))
        }
    }
}
