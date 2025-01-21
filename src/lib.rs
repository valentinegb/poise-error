use std::{cell::LazyCell, convert::Infallible, str::FromStr};

use anyhow::anyhow;
use poise::{
    serenity_prelude::{
        colours::css::{DANGER, WARNING},
        CreateEmbed, CreateEmbedFooter, Mentionable,
    },
    BoxFuture, CreateReply, FrameworkError,
};
use thiserror::Error;
use tracing::{error, warn};

pub type Error = anyhow::Error;

#[derive(Error, Debug)]
#[error(transparent)]
pub struct UserError(#[from] pub Error);

impl From<String> for UserError {
    fn from(value: String) -> Self {
        UserError(anyhow!(value))
    }
}

impl FromStr for UserError {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(s.to_string().into())
    }
}

async fn try_handle_error<U>(error: FrameworkError<'_, U, Error>) -> Result<(), Error> {
    const MAYBE_BOT_ERROR: LazyCell<CreateEmbedFooter> = LazyCell::new(|| {
        CreateEmbedFooter::new(
            "If you believe this is an error on the bot's end, please contact a developer.",
        )
    });
    const BOT_ERROR: LazyCell<CreateEmbedFooter> = LazyCell::new(|| {
        CreateEmbedFooter::new(
            "This isn't supposed to happen! If you have the time, please contact a developer.",
        )
    });

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
                                .footer(MAYBE_BOT_ERROR.to_owned())
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
                                .footer(BOT_ERROR.to_owned())
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
                            .footer(MAYBE_BOT_ERROR.to_owned())
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
                            .footer(BOT_ERROR.to_owned())
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
                            .description(format!("*Whooaa maaan... going a little fast there duuude... you should really **cool down** some... for like, **~{} seconds**...*", remaining_cooldown.as_secs()))
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
                                .footer(MAYBE_BOT_ERROR.to_owned())
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
                                .footer(MAYBE_BOT_ERROR.to_owned())
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

pub fn on_error<U>(error: FrameworkError<'_, U, Error>) -> BoxFuture<'_, ()>
where
    U: Send + Sync,
{
    Box::pin(async move {
        if let Err(err) = try_handle_error(error).await {
            error!("Failed to handle error: {err:#}");
        }
    })
}
