//! An opinionated plug-and-play library for error handling in Discord bots made
//! with [poise].
//!
//! To get started, see [on_error].
//!
//! # Examples
//!
//! [Goober Bot] is a Discord bot which uses `poise_error`, here's how it looks:
//!
//! <!-- errors screenshot -->
//!
//! <!-- logs screenshot -->
//!
//! ```ignore
//! /// Fails intentionally
//! #[command(slash_command)]
//! async fn error(
//!     _ctx: Context<'_>,
//!     #[description = "Kind of error to return"] kind: ErrorKind,
//! ) -> Result<(), poise_error::anyhow::Error> {
//!     match kind {
//!         ErrorKind::User => bail!(UserError(
//!             anyhow!("This is an example of a user error")
//!                 .context("This is an example of extra context")
//!         )),
//!         ErrorKind::Internal => Err(anyhow!("This is an example of an internal error")
//!             .context("This is an example of extra context")),
//!         ErrorKind::Panic => panic!("This is an example of a panic"),
//!     }
//! }
//! ```
//!
//! [Goober Bot]: https://github.com/valentinegb/goober-bot

use std::{convert::Infallible, str::FromStr};

use poise::{
    serenity_prelude::{
        colours::css::{DANGER, WARNING},
        CreateEmbed, CreateEmbedFooter, Mentionable,
    },
    BoxFuture, CreateReply, FrameworkError,
};
use thiserror::Error;
use tracing::{error, warn};

pub use anyhow;

/// An anticipated error made by a user.
///
/// Returning *this* error from a command instead of only [anyhow::Error] will
/// present the user with an embed stating that *they* have made an error as
/// opposed to the bot having made an error.
///
/// # Examples
///
/// ```
/// use std::str::FromStr;
///
/// use poise_error::{
///     anyhow::{bail, Error},
///     UserError,
/// };
///
/// #[poise::command(prefix_command, slash_command)]
/// async fn command(ctx: poise::Context<'_, (), Error>) -> Result<(), Error> {
///     bail!(UserError::from_str("You stink!").unwrap())
/// }
/// ```
#[derive(Error, Debug)]
#[error(transparent)]
pub struct UserError(#[from] pub anyhow::Error);

impl From<String> for UserError {
    fn from(value: String) -> Self {
        UserError(anyhow::anyhow!(value))
    }
}

impl FromStr for UserError {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(s.to_string().into())
    }
}

async fn try_handle_error<U>(
    error: FrameworkError<'_, U, anyhow::Error>,
) -> Result<(), anyhow::Error> {
    const MAYBE_BOT_ERROR: &str =
        "If you believe this is an error on the bot's end, please contact a developer.";
    const BOT_ERROR: &str =
        "This isn't supposed to happen! If you have the time, please contact a developer.";

    match error {
        FrameworkError::Setup { error, .. } => error!("Failed to complete setup: {error:#}"),
        FrameworkError::EventHandler { error, event, .. } => error!(
            "Failed to handle event {:?}: {error:#}",
            event.snake_case_name(),
        ),
        FrameworkError::Command { error, ctx, .. } => {
            let invocation_string = ctx.invocation_string();
            let description = format!("```\n{error:?}\n```");

            if error.is::<UserError>() {
                warn!("User made an error when invoking {invocation_string:?}: {error:#}");
                ctx.send(
                    CreateReply::default()
                        .embed(
                            CreateEmbed::new()
                                .title("You seem to have made an error")
                                .description(description)
                                .footer(CreateEmbedFooter::new(MAYBE_BOT_ERROR))
                                .color(WARNING),
                        )
                        .reply(true)
                        .ephemeral(true),
                )
                .await?;
            } else {
                error!("An error occurred whilst executing {invocation_string:?}: {error:#}");
                ctx.send(
                    CreateReply::default()
                        .embed(
                            CreateEmbed::new()
                                .title("An internal error has occurred")
                                .description(description)
                                .footer(CreateEmbedFooter::new(BOT_ERROR))
                                .color(DANGER),
                        )
                        .reply(true)
                        .ephemeral(true),
                )
                .await?;
            }
        }
        FrameworkError::SubcommandRequired { ctx } => {
            warn!("User attempted to invoke a command, which requires a subcommand, without a subcommand: {:?}", ctx.invocation_string());

            let prefix = ctx.prefix();

            ctx.send(
                CreateReply::default()
                    .embed(
                        CreateEmbed::new()
                            .title("Subcommand required")
                            .description(format!(
                                "You must specify one of the following subcommands:\n\n{}",
                                ctx.command()
                                    .subcommands
                                    .iter()
                                    .map(|subcommand| {
                                        if prefix == ctx.framework().bot_id.mention().to_string() {
                                            format!("- {prefix} `{}`", subcommand.qualified_name)
                                        } else {
                                            format!("- `{prefix}{}`", subcommand.qualified_name)
                                        }
                                    })
                                    .collect::<Vec<_>>()
                                    .join("\n"),
                            ))
                            .color(WARNING),
                    )
                    .reply(true)
                    .ephemeral(true),
            )
            .await?;
        }
        FrameworkError::CommandPanic { ctx, .. } => {
            ctx.send(
                CreateReply::default()
                    .embed(
                        CreateEmbed::new()
                            .title("Panicked")
                            .description("A really bad error happened and the bot panicked! You should contact a bot developer and tell them to check the logs.")
                            .color(DANGER),
                    )
                    .reply(true)
                    .ephemeral(true),
            )
            .await?;
        }
        FrameworkError::ArgumentParse {
            error, input, ctx, ..
        } => {
            let invocation_string = ctx.invocation_string();
            let description = match input {
                Some(input) => {
                    format!("Failed to parse {input:?} from {invocation_string:?} into an argument: {error}")
                }
                None => {
                    format!("Failed to parse an argument from {invocation_string:?}: {error}")
                }
            };

            warn!("{description}");
            ctx.send(
                CreateReply::default()
                    .embed(
                        CreateEmbed::new()
                            .title("Failed to parse argument")
                            .description(description)
                            .footer(CreateEmbedFooter::new(MAYBE_BOT_ERROR))
                            .color(WARNING),
                    )
                    .reply(true)
                    .ephemeral(true),
            )
            .await?;
        }
        FrameworkError::CommandStructureMismatch {
            description, ctx, ..
        } => {
            error!(
                "Mismatch between registered command and poise command for `/{}`: {description}",
                ctx.command.qualified_name,
            );
            ctx.send(
                CreateReply::default()
                    .embed(
                        CreateEmbed::new()
                            .title("Command structure mismatch")
                            .description(format!("```\n{description}\n```"))
                            .footer(CreateEmbedFooter::new(BOT_ERROR))
                            .color(DANGER),
                    )
                    .reply(true)
                    .ephemeral(true),
            )
            .await?;
        }
        FrameworkError::CooldownHit {
            remaining_cooldown,
            ctx,
            ..
        } => {
            warn!("User hit cooldown with {:?}", ctx.invocation_string());
            ctx.send(
                CreateReply::default()
                    .embed(
                        CreateEmbed::new()
                            .title("Cooldown hit")
                            .description(format!("You must wait **~{} seconds** before you can use this command again.", remaining_cooldown.as_secs()))
                            .color(WARNING),
                    )
                    .reply(true)
                    .ephemeral(true),
            )
            .await?;
        }
        FrameworkError::MissingBotPermissions {
            missing_permissions,
            ctx,
            ..
        } => {
            warn!(
                "Bot is lacking permissions for {:?}: {missing_permissions}",
                ctx.invocation_string()
            );
            ctx.send(
                CreateReply::default()
                    .embed(
                        CreateEmbed::new()
                            .title("Lacking bot permissions")
                            .description(format!("The bot requires the following permissions to execute this command: **{missing_permissions}**"))
                            .color(WARNING),
                    )
                    .reply(true)
                    .ephemeral(true),
            )
            .await?;
        }
        FrameworkError::MissingUserPermissions {
            missing_permissions,
            ctx,
            ..
        } => match missing_permissions {
            Some(missing_permissions) => {
                warn!(
                    "User is lacking permissions for {:?}: {missing_permissions}",
                    ctx.invocation_string(),
                );
                ctx.send(
                    CreateReply::default()
                        .embed(
                            CreateEmbed::new()
                                .title("Lacking user permissions")
                                .description(format!("You must have the following permissions to execute this command: **{missing_permissions}**"))
                                .color(WARNING),
                        )
                        .reply(true)
                        .ephemeral(true),
                )
                .await?;
            }
            None => {
                warn!(
                    "User is lacking permissions for {:?}",
                    ctx.invocation_string(),
                );
                ctx.send(
                    CreateReply::default()
                        .embed(
                            CreateEmbed::new()
                                .title("Lacking user permissions")
                                .description("You do not have the permissions needed to execute this command")
                                .color(WARNING),
                        )
                        .reply(true)
                        .ephemeral(true),
                )
                .await?;
            }
        },
        FrameworkError::NotAnOwner { ctx, .. } => {
            warn!(
                "Non owner attempted to invoke {:?}",
                ctx.invocation_string(),
            );
            ctx.send(
                CreateReply::default()
                    .embed(
                        CreateEmbed::new()
                            .title("Owner only command")
                            .description("You must be an owner to use this command.")
                            .color(WARNING),
                    )
                    .reply(true)
                    .ephemeral(true),
            )
            .await?;
        }
        FrameworkError::GuildOnly { ctx, .. } => {
            warn!(
                "User attempted to invoke {:?} outside of a guild",
                ctx.invocation_string(),
            );
            ctx.send(
                CreateReply::default()
                    .embed(
                        CreateEmbed::new()
                            .title("Server only command")
                            .description("You cannot use this command outside of a server.")
                            .color(WARNING),
                    )
                    .reply(true)
                    .ephemeral(true),
            )
            .await?;
        }
        FrameworkError::DmOnly { ctx, .. } => {
            warn!(
                "User attempted to invoke {:?} outside of DMs",
                ctx.invocation_string(),
            );
            ctx.send(
                CreateReply::default()
                    .embed(
                        CreateEmbed::new()
                            .title("DMs only command")
                            .description("You cannot use this command outside of DMs.")
                            .color(WARNING),
                    )
                    .reply(true)
                    .ephemeral(true),
            )
            .await?;
        }
        FrameworkError::NsfwOnly { ctx, .. } => {
            warn!(
                "User attempted to invoke {:?} outside of an NSFW channel",
                ctx.invocation_string(),
            );
            ctx.send(
                CreateReply::default()
                    .embed(
                        CreateEmbed::new()
                            .title("NSFW command")
                            .description("You cannot use this command outside of an NSFW channel.")
                            .color(WARNING),
                    )
                    .reply(true)
                    .ephemeral(true),
            )
            .await?;
        }
        FrameworkError::CommandCheckFailed { error, ctx, .. } => match error {
            Some(error) => {
                warn!("Check failed for {:?}: {error:#}", ctx.invocation_string());
                ctx.send(
                    CreateReply::default()
                        .embed(
                            CreateEmbed::new()
                                .title("Check failed")
                                .description(format!("```\n{error:?}\n```"))
                                .footer(CreateEmbedFooter::new(MAYBE_BOT_ERROR))
                                .color(WARNING),
                        )
                        .reply(true)
                        .ephemeral(true),
                )
                .await?;
            }
            None => {
                warn!("Check failed for {:?}", ctx.invocation_string());
                ctx.send(
                    CreateReply::default()
                        .embed(
                            CreateEmbed::new()
                                .title("Check failed")
                                .description("That's all I know. ¯\\\\_(ツ)_/¯")
                                .footer(CreateEmbedFooter::new(MAYBE_BOT_ERROR))
                                .color(WARNING),
                        )
                        .reply(true)
                        .ephemeral(true),
                )
                .await?;
            }
        },
        FrameworkError::DynamicPrefix { error, msg, .. } => {
            error!("Dynamic prefix failed for {msg:?}: {error:#}");
        }
        FrameworkError::UnknownCommand {
            prefix,
            msg_content,
            ..
        } => {
            warn!("Recognized prefix {prefix:?} but did not recognize command {msg_content:?}");
        }
        FrameworkError::UnknownInteraction { interaction, .. } => {
            warn!(
                "Received interaction for an unknown command: {:?}",
                interaction.data.name,
            );
        }
        other => {
            warn!("Not prepared to handle unfamiliar kind of error, falling back to default `on_error` function");
            poise::builtins::on_error(other).await?;
        }
    }

    Ok(())
}

/// Plug this into your [poise::FrameworkOptions] to let `poise_error` handle
/// your bot's errors.
///
/// [anyhow::Error] is the error type expected to be returned from commands.
///
/// # Examples
///
/// ```
/// use poise_error::on_error;
///
/// let framework = poise::Framework::builder()
///     .options(poise::FrameworkOptions {
///         on_error,
///         ..Default::default()
///     })
/// #     .setup(|ctx, _ready, framework| {
/// #         Box::pin(async move { Ok(()) })
/// #     })
///     .build();
/// ```
pub fn on_error<U>(error: FrameworkError<'_, U, anyhow::Error>) -> BoxFuture<'_, ()>
where
    U: Send + Sync,
{
    Box::pin(async move {
        if let Err(err) = try_handle_error(error).await {
            error!("Failed to handle error: {err:#}");
        }
    })
}
