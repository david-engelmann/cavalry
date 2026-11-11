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
    about = "The troop that rides in after the fact and writes down what a human responder actually did, because the agent log alone cannot be trusted.",
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

/// The hand set the preprocess walk reads help tokens
/// against on the root, holding the help long alone
/// because the root command declares no other long
/// (CV-60). The set stands in one function so the pin
/// of CV-60 can read the same table the walk reads
/// (CV-64).
fn root_longs() -> Vec<String> {
    vec!["--help".to_string()]
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
    let root_longs: Vec<String> = root_longs();
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Derive the root long set from the built command in
    /// the manner subcommand_specs derives the long set of
    /// a subcommand, collecting the long of every argument
    /// the root command carries and completing the set with
    /// the help long where it stands absent.
    fn derived_root_longs() -> Vec<String> {
        let mut command = Cli::command();
        command.build();
        let mut longs: Vec<String> = Vec::new();
        for arg in command.get_arguments() {
            let Some(long) = arg.get_long() else {
                continue;
            };
            let name = format!("--{long}");
            if !longs.contains(&name) {
                longs.push(name);
            }
        }
        if !longs.iter().any(|long| long == "--help") {
            longs.push("--help".to_string());
        }
        longs
    }

    /// CV-60. The hand set the preprocess walk reads help
    /// tokens against on the root holds the help long alone,
    /// and it holds it because the root command declares no
    /// other long. This pin derives the root long set from
    /// the built command and asserts the derived set is the
    /// one help long, so a long added to the root command,
    /// declared as an argument or raised by an attribute,
    /// fails here in the open instead of leaving the hand
    /// set standing behind the parser in silence.
    ///
    /// CV-64 adds the second assertion, that the table
    /// root_longs returns equals the derived set, so an
    /// edit to the table alone fails here as well.
    #[test]
    fn root_long_set_is_the_one_help_long() {
        assert_eq!(derived_root_longs(), vec!["--help".to_string()]);
        assert_eq!(root_longs(), derived_root_longs());
    }

    /// Derive the short of the help argument of every
    /// command in the tree, the root command first and
    /// then its subcommands in their own order, walking
    /// the tree recursively so a command standing at any
    /// depth stands inside the derivation, finding each
    /// argument by its help long in the manner
    /// derived_root_longs collects the longs of the root
    /// command, and standing absent in a command's place
    /// where that command carries no argument with the
    /// help long.
    fn derived_help_shorts() -> Vec<Option<char>> {
        fn help_short(command: &clap::Command) -> Option<char> {
            for arg in command.get_arguments() {
                if arg.get_long() == Some("help") {
                    return arg.get_short();
                }
            }
            None
        }
        fn collect(command: &clap::Command, shorts: &mut Vec<Option<char>>) {
            shorts.push(help_short(command));
            for sub in command.get_subcommands() {
                collect(sub, shorts);
            }
        }
        let mut command = Cli::command();
        command.build();
        let mut shorts = Vec::new();
        collect(&command, &mut shorts);
        shorts
    }

    /// CV-63. The two reads that test the second character
    /// of a token against the letter h, the short help test
    /// of reads_as_positional and the short help branch of
    /// help_read, test that character in every context the
    /// walk reaches, on the root and behind any command in
    /// the tree alike, and they rest on the help argument
    /// of every command in the tree carrying the short h.
    /// This pin derives the help short of every command
    /// from the built command and asserts each one is the
    /// letter h, so a help short customized to another
    /// letter on any command, or a command left with no
    /// help argument at all, fails here in the open instead
    /// of leaving the two reads standing behind the parser
    /// in silence.
    #[test]
    fn every_help_short_is_the_letter_h() {
        let shorts = derived_help_shorts();
        assert_eq!(shorts.len(), 6);
        assert!(shorts.iter().all(|short| *short == Some('h')));
    }

    /// The longs of one named subcommand's spec, for the
    /// help reads that stand behind a subcommand.
    fn spec_longs(name: &str) -> Vec<String> {
        subcommand_specs()
            .into_iter()
            .find(|spec| spec.name == name)
            .expect("the subcommand stands in the specs")
            .longs
    }

    /// Read a token as a help token and report the outcome
    /// in a comparable form, whether it fired and the
    /// refusal's replacement where it refused.
    fn read_of(token: &str, longs: &[String]) -> (bool, Option<String>) {
        match help_read(token, longs) {
            HelpRead::Fire => (true, None),
            HelpRead::Refuse(replacement) => {
                (false, Some(replacement.to_string_lossy().into_owned()))
            }
            HelpRead::No => (false, None),
        }
    }

    /// CV-80. The help long read in full fires.
    #[test]
    fn help_long_exact_fires() {
        assert_eq!(read_of("--help", &root_longs()), (true, None));
    }

    /// CV-80. A prefix of the help long that matches it
    /// alone fires, at either length.
    #[test]
    fn help_long_prefix_fires() {
        assert_eq!(read_of("--hel", &root_longs()), (true, None));
        assert_eq!(read_of("--h", &root_longs()), (true, None));
    }

    /// CV-80. The help long carrying an explicit value
    /// refuses, and the refusal carries the token itself.
    #[test]
    fn help_long_with_value_refused() {
        assert_eq!(
            read_of("--help=x", &root_longs()),
            (false, Some("--help=x".to_string()))
        );
    }

    /// CV-80. A prefix of the help long carrying an
    /// explicit value refuses the same way.
    #[test]
    fn help_long_prefix_with_value_refused() {
        assert_eq!(
            read_of("--hel=x", &root_longs()),
            (false, Some("--hel=x".to_string()))
        );
    }

    /// CV-80. The separator is no help reading, its name
    /// standing at the two character floor.
    #[test]
    fn double_dash_is_no_reading() {
        assert_eq!(read_of("--", &root_longs()), (false, None));
    }

    /// CV-80. A long token matching nothing in the set is
    /// no help reading.
    #[test]
    fn unknown_long_is_no_reading() {
        assert_eq!(read_of("--bogus", &root_longs()), (false, None));
    }

    /// CV-80. Behind capture, the fixtures long itself is
    /// no help reading, the unique match standing on a
    /// long that is not the help long.
    #[test]
    fn fixtures_long_is_no_reading() {
        assert_eq!(read_of("--fixtures", &spec_longs("capture")), (false, None));
    }

    /// CV-80. Behind capture, a prefix of the fixtures
    /// long is no help reading either.
    #[test]
    fn fixtures_prefix_is_no_reading() {
        assert_eq!(read_of("--f", &spec_longs("capture")), (false, None));
        assert_eq!(read_of("--fi", &spec_longs("capture")), (false, None));
    }

    /// CV-80. The bare short help token fires.
    #[test]
    fn short_help_fires() {
        assert_eq!(read_of("-h", &root_longs()), (true, None));
    }

    /// CV-80. A short help token whose bundle continues
    /// with another letter fires.
    #[test]
    fn short_help_bundle_fires() {
        assert_eq!(read_of("-hx", &root_longs()), (true, None));
    }

    /// CV-80. A short help token with an equals
    /// continuation refuses, in the rewritten long form.
    #[test]
    fn short_help_equals_refused() {
        assert_eq!(
            read_of("-h=x", &root_longs()),
            (false, Some("--help=x".to_string()))
        );
    }

    /// CV-80. A short help token with a dash continuation
    /// refuses, the dash standing in the rewritten form.
    #[test]
    fn short_help_dash_refused() {
        assert_eq!(
            read_of("-h-x", &root_longs()),
            (false, Some("--help=-x".to_string()))
        );
    }

    /// CV-80. A short token whose second character is not
    /// h is no help reading.
    #[test]
    fn non_help_short_is_no_reading() {
        assert_eq!(read_of("-x", &root_longs()), (false, None));
    }

    /// CV-80. A lone dash is no help reading.
    #[test]
    fn lone_dash_is_no_reading() {
        assert_eq!(read_of("-", &root_longs()), (false, None));
    }

    /// CV-80. An empty token is no help reading.
    #[test]
    fn empty_token_is_no_reading() {
        assert_eq!(read_of("", &root_longs()), (false, None));
    }

    /// The expected stream in the run helper's own form.
    fn expect(tokens: &[&str]) -> Vec<String> {
        tokens.iter().map(|token| token.to_string()).collect()
    }

    /// Walk the tokens and return the stream handed to
    /// clap, as text.
    fn run_stream(tokens: &[&str]) -> Vec<String> {
        let owned: Vec<OsString> = tokens.iter().map(OsString::from).collect();
        match preprocess(&owned) {
            Preprocess::Run(stream) => stream
                .iter()
                .map(|token| token.to_string_lossy().into_owned())
                .collect(),
            Preprocess::FireHelp(target) => {
                panic!("expected a run, got help for {target:?}")
            }
        }
    }

    /// Walk the tokens and return the fired help's
    /// target, the subcommand's name or none at the root.
    fn fired_help(tokens: &[&str]) -> Option<String> {
        let owned: Vec<OsString> = tokens.iter().map(OsString::from).collect();
        match preprocess(&owned) {
            Preprocess::FireHelp(target) => target,
            Preprocess::Run(stream) => {
                panic!("expected help, got a run of {} tokens", stream.len())
            }
        }
    }

    /// CV-80. An empty argument list runs empty.
    #[test]
    fn empty_argv_runs_empty() {
        assert_eq!(run_stream(&[]), expect(&[]));
    }

    /// CV-80. A subcommand alone selects its parser and
    /// runs as it stands.
    #[test]
    fn subcommand_alone_runs() {
        assert_eq!(run_stream(&["capture"]), expect(&["capture"]));
    }

    /// CV-80. Help at the root fires for the top level.
    #[test]
    fn root_help_fires_top_level() {
        assert_eq!(fired_help(&["--help"]), None);
    }

    /// CV-80. A prefix of the help long at the root fires
    /// the same way.
    #[test]
    fn root_help_prefix_fires() {
        assert_eq!(fired_help(&["--hel"]), None);
    }

    /// CV-80. Help behind each subcommand fires for that
    /// subcommand, all five of them.
    #[test]
    fn subcommand_help_fires_named() {
        for name in ["capture", "list", "propose", "confirm", "queue"] {
            assert_eq!(
                fired_help(&[name, "--help"]),
                Some(name.to_string()),
                "help behind {name}"
            );
        }
    }

    /// CV-80. A help token carrying a value at the root
    /// blocks the walk, and the raw stream carries the
    /// token as it stood.
    #[test]
    fn root_help_value_blocks_as_it_stood() {
        assert_eq!(run_stream(&["--help=x"]), expect(&["--help=x"]));
    }

    /// CV-80. A short help token carrying a value at the
    /// root blocks the walk, and the raw stream carries
    /// the rewritten long form in its place.
    #[test]
    fn root_short_help_value_rewritten() {
        assert_eq!(run_stream(&["-h=x"]), expect(&["--help=x"]));
    }

    /// CV-80. A positional string at the root blocks the
    /// walk, the raw stream standing as it stood.
    #[test]
    fn root_positional_blocks() {
        assert_eq!(run_stream(&["bogus"]), expect(&["bogus"]));
    }

    /// CV-80. Once the walk is blocked, a help token
    /// behind the block does not fire.
    #[test]
    fn help_after_blocked_does_not_fire() {
        assert_eq!(
            run_stream(&["bogus", "--help"]),
            expect(&["bogus", "--help"])
        );
    }

    /// CV-80. An option token collected at the root does
    /// not stand in help's way, and the help behind it
    /// fires.
    #[test]
    fn collected_option_then_help_fires() {
        assert_eq!(fired_help(&["--bogus", "--help"]), None);
    }

    /// CV-80. An option token at the root is only
    /// collected, and the stream carries it as it stood.
    #[test]
    fn root_option_collected() {
        assert_eq!(run_stream(&["--bogus"]), expect(&["--bogus"]));
    }

    /// CV-80. A subcommand named after a collected option
    /// still selects its parser.
    #[test]
    fn option_then_subcommand_selects() {
        assert_eq!(
            run_stream(&["--bogus", "capture"]),
            expect(&["--bogus", "capture"])
        );
    }

    /// CV-80. A subcommand's name behind a subcommand
    /// reads as a positional value and is gathered behind
    /// the separator.
    #[test]
    fn subcommand_name_behind_subcommand_is_a_value() {
        assert_eq!(
            run_stream(&["capture", "queue"]),
            expect(&["capture", "--", "queue"])
        );
    }

    /// CV-80. A separator standing first gathers every
    /// token after it as a value, the subcommand's name
    /// among them.
    #[test]
    fn separator_first_gathers_all() {
        assert_eq!(run_stream(&["--", "capture"]), expect(&["--", "capture"]));
    }

    /// CV-80. Behind the separator a help token is a
    /// value like any other.
    #[test]
    fn separator_tail_takes_help_as_value() {
        assert_eq!(
            run_stream(&["capture", "--", "--help"]),
            expect(&["capture", "--", "--help"])
        );
    }

    /// CV-80. A second separator behind the first is a
    /// value too, standing in the gathered stream.
    #[test]
    fn second_separator_is_a_value() {
        assert_eq!(
            run_stream(&["capture", "--", "--", "x"]),
            expect(&["capture", "--", "--", "x"])
        );
    }

    /// CV-80. A bare fixtures flag joins the value that
    /// follows it in the equals form.
    #[test]
    fn fixtures_bare_value_joins() {
        assert_eq!(
            run_stream(&["capture", "--fixtures", "dir"]),
            expect(&["capture", "--fixtures=dir"])
        );
    }

    /// CV-80. A fixtures flag whose value's place is
    /// taken by an option token blocks the walk, the raw
    /// stream standing as it stood.
    #[test]
    fn fixtures_pending_meets_option_blocks() {
        assert_eq!(
            run_stream(&["capture", "--fixtures", "--help"]),
            expect(&["capture", "--fixtures", "--help"])
        );
    }

    /// CV-80. A fixtures flag whose value's place is
    /// taken by the separator blocks the walk the same
    /// way.
    #[test]
    fn fixtures_pending_meets_separator_blocks() {
        assert_eq!(
            run_stream(&["capture", "--fixtures", "--"]),
            expect(&["capture", "--fixtures", "--"])
        );
    }

    /// CV-80. A fixtures flag standing last, its value
    /// never arriving, hands the raw stream over.
    #[test]
    fn fixtures_pending_at_end_runs_raw() {
        assert_eq!(
            run_stream(&["capture", "--fixtures"]),
            expect(&["capture", "--fixtures"])
        );
    }

    /// CV-80. An empty value for the bare fixtures flag
    /// removes the standing token, and the flag stands
    /// as no flag at all.
    #[test]
    fn fixtures_empty_value_clears_flag() {
        assert_eq!(
            run_stream(&["capture", "--fixtures", ""]),
            expect(&["capture"])
        );
    }

    /// CV-80. A value carrying a space reads as a
    /// positional string and joins the same way.
    #[test]
    fn fixtures_space_value_joins() {
        assert_eq!(
            run_stream(&["capture", "--fixtures", "a b"]),
            expect(&["capture", "--fixtures=a b"])
        );
    }

    /// CV-80. An abbreviated fixtures flag joins its
    /// value in the form the flag was given in.
    #[test]
    fn fixtures_abbreviated_flag_joins_as_given() {
        assert_eq!(
            run_stream(&["capture", "--fix", "dir"]),
            expect(&["capture", "--fix=dir"])
        );
    }

    /// CV-80. An attached fixtures value stands in the
    /// stream as it stood.
    #[test]
    fn fixtures_attached_value_stands() {
        assert_eq!(
            run_stream(&["capture", "--fixtures=dir"]),
            expect(&["capture", "--fixtures=dir"])
        );
    }

    /// CV-80. An empty attached fixtures value stands as
    /// no flag at all.
    #[test]
    fn fixtures_attached_empty_stands_as_no_flag() {
        assert_eq!(
            run_stream(&["capture", "--fixtures="]),
            expect(&["capture"])
        );
    }

    /// CV-80. A bare fixtures flag after an attached one
    /// replaces the standing token, and its joined value
    /// is the one the stream carries.
    #[test]
    fn fixtures_bare_overrides_attached() {
        assert_eq!(
            run_stream(&["capture", "--fixtures=a", "--fixtures", "b"]),
            expect(&["capture", "--fixtures=b"])
        );
    }

    /// CV-80. An attached fixtures flag after a joined
    /// one replaces it the same way.
    #[test]
    fn fixtures_attached_overrides_joined() {
        assert_eq!(
            run_stream(&["capture", "--fixtures", "a", "--fixtures=b"]),
            expect(&["capture", "--fixtures=b"])
        );
    }

    /// CV-80. An empty attached fixtures value after a
    /// standing one clears it and stands as no flag.
    #[test]
    fn fixtures_attached_empty_clears_standing() {
        assert_eq!(
            run_stream(&["capture", "--fixtures=a", "--fixtures="]),
            expect(&["capture"])
        );
    }

    /// CV-80. A negative number in the value's place
    /// reads as a positional string and joins.
    #[test]
    fn fixtures_negative_value_joins() {
        assert_eq!(
            run_stream(&["capture", "--fixtures", "-1"]),
            expect(&["capture", "--fixtures=-1"])
        );
    }

    /// CV-80. A help-like token in the value's place does
    /// not read as a positional string and blocks the
    /// walk.
    #[test]
    fn fixtures_help_like_value_blocks() {
        assert_eq!(
            run_stream(&["capture", "--fixtures", "-hx"]),
            expect(&["capture", "--fixtures", "-hx"])
        );
    }

    /// CV-80. A positional behind queue is gathered
    /// behind the separator.
    #[test]
    fn queue_positional_gathered() {
        assert_eq!(
            run_stream(&["queue", "book.json"]),
            expect(&["queue", "--", "book.json"])
        );
    }

    /// CV-80. Two positionals behind queue are gathered
    /// in their order.
    #[test]
    fn queue_two_positionals_gathered() {
        assert_eq!(
            run_stream(&["queue", "a.json", "b.json"]),
            expect(&["queue", "--", "a.json", "b.json"])
        );
    }

    /// CV-80. An unknown long behind queue is only
    /// collected, standing in the stream as it stood.
    #[test]
    fn queue_unknown_long_collected() {
        assert_eq!(
            run_stream(&["queue", "--bogus"]),
            expect(&["queue", "--bogus"])
        );
    }

    /// CV-80. A collected long behind queue stands ahead
    /// of the separator the gathered values follow.
    #[test]
    fn queue_collected_then_positional() {
        assert_eq!(
            run_stream(&["queue", "--bogus", "book.json"]),
            expect(&["queue", "--bogus", "--", "book.json"])
        );
    }

    /// CV-80. An unknown short behind queue is only
    /// collected the same way.
    #[test]
    fn queue_unknown_short_collected() {
        assert_eq!(run_stream(&["queue", "-x"]), expect(&["queue", "-x"]));
    }

    /// CV-80. A negative number behind queue reads as a
    /// positional string and is gathered.
    #[test]
    fn queue_negative_reads_as_positional() {
        assert_eq!(run_stream(&["queue", "-1"]), expect(&["queue", "--", "-1"]));
    }

    /// CV-80. A long token carrying a space behind queue
    /// reads as a positional string and is gathered, the
    /// space test standing ahead of the option reading.
    #[test]
    fn queue_space_long_reads_as_positional() {
        assert_eq!(
            run_stream(&["queue", "--two words"]),
            expect(&["queue", "--", "--two words"])
        );
    }

    /// CV-80. The fixtures long behind queue, which
    /// declares no value flag, is only collected and
    /// consumes nothing.
    #[test]
    fn queue_fixtures_long_collected_not_consumed() {
        assert_eq!(
            run_stream(&["queue", "--fixtures"]),
            expect(&["queue", "--fixtures"])
        );
    }

    /// CV-80. Both of capture's positionals are gathered
    /// in their order.
    #[test]
    fn capture_positionals_gathered() {
        assert_eq!(
            run_stream(&["capture", "rec.json", "book.json"]),
            expect(&["capture", "--", "rec.json", "book.json"])
        );
    }

    /// CV-80. A fixtures flag standing before a
    /// positional stays in place while the positional is
    /// gathered behind the separator.
    #[test]
    fn flag_before_positional_orders_stream() {
        assert_eq!(
            run_stream(&["capture", "--fixtures", "dir", "rec.json"]),
            expect(&["capture", "--fixtures=dir", "--", "rec.json"])
        );
    }

    /// CV-80. A positional standing before the fixtures
    /// flag lands in the same stream, the gathered values
    /// always following the one separator.
    #[test]
    fn positional_before_flag_orders_stream() {
        assert_eq!(
            run_stream(&["capture", "rec.json", "--fixtures", "dir"]),
            expect(&["capture", "--fixtures=dir", "--", "rec.json"])
        );
    }

    /// CV-80. Help behind a collected extra behind a
    /// subcommand still fires.
    #[test]
    fn collected_extras_then_help_fires() {
        assert_eq!(
            fired_help(&["queue", "--bogus", "--help"]),
            Some("queue".to_string())
        );
    }

    /// CV-80. A help token carrying a value behind a
    /// subcommand blocks the walk there too.
    #[test]
    fn subcommand_help_value_blocks() {
        assert_eq!(
            run_stream(&["queue", "--help=x"]),
            expect(&["queue", "--help=x"])
        );
    }

    /// CV-80. A short help token behind a subcommand
    /// fires for that subcommand.
    #[test]
    fn subcommand_short_help_fires() {
        assert_eq!(fired_help(&["queue", "-h"]), Some("queue".to_string()));
    }

    /// CV-80. A short help bundle at the root fires for
    /// the top level.
    #[test]
    fn root_short_help_bundle_fires() {
        assert_eq!(fired_help(&["-hx"]), None);
    }

    /// CV-80. Behind the separator after a joined flag,
    /// the tail's tokens are gathered as values.
    #[test]
    fn joined_flag_then_separator_tail() {
        assert_eq!(
            run_stream(&["capture", "--fixtures", "dir", "--", "x"]),
            expect(&["capture", "--fixtures=dir", "--", "x"])
        );
    }

    /// CV-80. An empty token behind a subcommand reads
    /// as a positional string and is gathered.
    #[test]
    fn empty_token_behind_subcommand_is_a_value() {
        assert_eq!(run_stream(&["queue", ""]), expect(&["queue", "--", ""]));
    }

    /// CV-80. An empty token at the root reads as a
    /// positional string and blocks the walk.
    #[test]
    fn empty_token_at_root_blocks() {
        assert_eq!(run_stream(&[""]), expect(&[""]));
    }

    /// CV-80. Two separators at the root run as the
    /// separator and one gathered value, the second
    /// separator itself.
    #[test]
    fn double_separator_at_root() {
        assert_eq!(run_stream(&["--", "--"]), expect(&["--", "--"]));
    }

    /// CV-80. When a later fixtures flag removes the
    /// standing token, the removal shifts the stream and
    /// the gathered positional still follows the
    /// separator.
    #[test]
    fn override_removal_shifts_stream() {
        assert_eq!(
            run_stream(&["capture", "rec.json", "--fixtures=a", "--fixtures=b"]),
            expect(&["capture", "--fixtures=b", "--", "rec.json"])
        );
    }

    /// CV-80. An abbreviated fixtures flag standing
    /// last, its value never arriving, hands the raw
    /// stream over as it stood.
    #[test]
    fn fixtures_prefix_alone_runs_raw() {
        assert_eq!(
            run_stream(&["capture", "--fi"]),
            expect(&["capture", "--fi"])
        );
    }

    /// CV-80. Once the fixtures value has joined, a help
    /// token behind it fires for capture.
    #[test]
    fn fixtures_value_then_help_fires() {
        assert_eq!(
            fired_help(&["capture", "--fixtures", "dir", "--help"]),
            Some("capture".to_string())
        );
    }
}
