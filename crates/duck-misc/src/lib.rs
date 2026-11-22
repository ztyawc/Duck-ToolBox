//! Miscellaneous tools, each with its own configuration and lifecycle.

mod language;
mod store;
mod timezone;

use clap::Subcommand;
use duck_core::{BoxFuture, ClapFeature, CommandResult, Context, FeatureInfo, IntoCommandResult};

pub static FEATURE: ClapFeature<Command> = ClapFeature::new(
    FeatureInfo {
        id: "misc",
        summary: "Miscellaneous tools: per-app language and timezone spoofing",
        contract: 2,
    },
    dispatch,
);

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Process-local language overrides; never changes the device language.
    Language {
        #[command(subcommand)]
        command: RuleCommand,
    },
    /// Process-local timezone overrides; never changes the device timezone or clock.
    Timezone {
        #[command(subcommand)]
        command: RuleCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum RuleCommand {
    Status,
    /// Save rules from stdin, guarded by the status revision. Does not enable injection.
    Save {
        #[arg(long, required = true)]
        stdin_json: bool,
    },
    /// Enable saved rules from stdin: {"revision":"..."}.
    Enable {
        #[arg(long, required = true)]
        stdin_json: bool,
    },
    /// Stop injection in future app processes, including with a corrupt configuration.
    Disable,
}

fn dispatch<'a>(command: Command, ctx: &'a Context) -> BoxFuture<'a, CommandResult> {
    Box::pin(std::future::ready(match command {
        Command::Language { command } => match command {
            RuleCommand::Status => {
                language::status(ctx).into_command("misc.language.status", store::code_of)
            }
            RuleCommand::Save { .. } => {
                language::save(ctx).into_command("misc.language.save", store::code_of)
            }
            RuleCommand::Enable { .. } => {
                language::enable(ctx).into_command("misc.language.enable", store::code_of)
            }
            RuleCommand::Disable => {
                language::disable(ctx).into_command("misc.language.disable", store::code_of)
            }
        },
        Command::Timezone { command } => match command {
            RuleCommand::Status => {
                timezone::status(ctx).into_command("misc.timezone.status", store::code_of)
            }
            RuleCommand::Save { .. } => {
                timezone::save(ctx).into_command("misc.timezone.save", store::code_of)
            }
            RuleCommand::Enable { .. } => {
                timezone::enable(ctx).into_command("misc.timezone.enable", store::code_of)
            }
            RuleCommand::Disable => {
                timezone::disable(ctx).into_command("misc.timezone.disable", store::code_of)
            }
        },
    }))
}
