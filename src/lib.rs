//! An opinionated plug-and-play library for error handling in Discord bots made
//! with [`poise`].
//!
//! To get started, see [`on_error`].
//!
//! # Examples
//!
//! [Goober Bot] is a Discord bot which uses [`poise_error`][crate], here's how it looks:
//!
//! ![Screenshot 2025-01-22 at 6 23 00 PM](https://github.com/user-attachments/assets/aef54d4b-8cde-4d96-aa06-434598fe1326)
//!
//! ![Screenshot 2025-01-22 at 6 24 01 PM](https://github.com/user-attachments/assets/bc4cc74a-9a9b-4d2d-ac5f-e7a5f18d9a02)
//!
//! ```
//! use poise::{ChoiceParameter, command};
//! use poise_error::{
//!     UserError,
//!     anyhow::{self, anyhow, bail},
//! };
//!
//! #[derive(ChoiceParameter)]
//! enum ErrorKind {
//!     User,
//!     Internal,
//!     Panic,
//! }
//!
//! /// Fails intentionally
//! #[command(slash_command)]
//! async fn error(
//!     _ctx: poise_error::Context<'_>,
//!     #[description = "Kind of error to return"] kind: ErrorKind,
//! ) -> anyhow::Result<()> {
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
    BoxFuture, CreateReply, FrameworkError,
    serenity_prelude::{
        Mentionable,
        colours::css::{DANGER, WARNING},
    },
};
use serenity::all::{
    CreateAllowedMentions, CreateComponent, CreateContainer, CreateSeparator, CreateTextDisplay,
    MessageFlags,
};
use thiserror::Error;
use tracing::{error, warn};

pub use anyhow;

/// A shorthand for the [`poise::Context`] enum.
///
/// The `E` generic is set to [`anyhow::Error`] and the `U` generic is set to
/// [`()`][unit] by default, though that can be changed (e.g.
/// `poise_error::Context<'_, MyType>`).
pub type Context<'a, U = ()> = poise::Context<'a, U, anyhow::Error>;

/// An anticipated error made by a user.
///
/// Returning this error from a command instead of only [`anyhow::Error`] will
/// present the user with a message stating that *they* have made an error as
/// opposed to the bot having made an error. If given a chain of errors, only
/// the last error in the chain is shown to the user. This error is not encased
/// in a codeblock like other errors so that you may take advantage of Discord's
/// formatting. As such, it's recommended you capitalize the first letter of
/// your error message.
///
/// # Examples
///
/// ```
/// use std::str::FromStr;
///
/// use poise_error::{
///     anyhow::{self, bail},
///     UserError,
/// };
///
/// #[poise::command(prefix_command, slash_command)]
/// async fn command(ctx: poise_error::Context<'_>) -> anyhow::Result<()> {
///     bail!(UserError::from_str("You *stink!*").unwrap())
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

/// Removes duplicates from an error's chain.
///
/// This function does not retain any error types; all errors in a chain will
/// be turned into strings.
///
/// Used internally by [`poise_error`][crate], see [`try_handle_error`].
pub fn dedup_error_chain(error: &mut anyhow::Error) {
    let mut chain: Vec<String> = error.chain().map(|err| err.to_string()).collect();

    chain.dedup();

    let mut chain = chain.into_iter().rev();
    let mut deduped_error = anyhow::anyhow!(chain.next().unwrap());

    for message in chain {
        deduped_error = deduped_error.context(message);
    }

    *error = deduped_error;
}

/// Handles errors given by [`poise`].
///
/// Used internally by [`on_error`]. You can use this instead of [`on_error`] if
/// you would like to extend the functionality of [`poise_error`][crate].
///
/// # Examples
///
/// ```
/// use poise::FrameworkError;
/// use poise_error::{dedup_error_chain, try_handle_error};
/// # use thiserror::Error;
/// use tracing::error;
///
/// # #[derive(Error, Debug)]
/// # #[error("this is my special error :)")]
/// # struct SpecialError;
/// #
/// async fn my_custom_error_handler<U>(
///     error: FrameworkError<'_, U, anyhow::Error>,
/// ) -> Result<(), anyhow::Error> {
///     match error {
///         FrameworkError::CommandCheckFailed {
///             error: Some(error), ..
///         } if error.is::<SpecialError>() => {
///             // Handle special error case
///         }
///         other => try_handle_error(other).await?,
///     }
///
///     Ok(())
/// }
///
/// let framework = poise::Framework::builder()
///     .options(poise::FrameworkOptions {
///         on_error: |error| {
///             Box::pin(async move {
///                 if let Err(mut err) = my_custom_error_handler(error).await {
///                     dedup_error_chain(&mut err);
///                     error!("Failed to handle error: {err:#}");
///                 }
///             })
///         },
///         ..Default::default()
///     })
///     .setup(|ctx, _ready, framework| {
///         Box::pin(async move { Ok(()) })
///     })
///     .build();
/// ```
pub async fn try_handle_error<U: Send + Sync + 'static>(
    error: FrameworkError<'_, U, anyhow::Error>,
) -> Result<(), anyhow::Error> {
    const MAYBE_BOT_ERROR_FOOTER: &str =
        "-# If you believe this is an error on the bot's end, please contact a developer.";
    const BOT_ERROR_FOOTER: &str =
        "-# This isn't supposed to happen! If you have the time, please contact a developer.";

    match error {
        FrameworkError::Command { mut error, ctx, .. } => {
            let invocation_string = ctx.invocation_string();
            let is_user_error = error.is::<UserError>();

            dedup_error_chain(&mut error);

            if is_user_error {
                ctx.send(
                    CreateReply::default()
                        .flags(MessageFlags::IS_COMPONENTS_V2)
                        .components(&[CreateComponent::Container(
                            CreateContainer::new(&[
                                CreateComponent::TextDisplay(CreateTextDisplay::new(
                                    "### You seem to have made an error",
                                )),
                                CreateComponent::TextDisplay(CreateTextDisplay::new(format!(
                                    "{error}",
                                ))),
                                CreateComponent::Separator(CreateSeparator::new(true)),
                                CreateComponent::TextDisplay(CreateTextDisplay::new(
                                    MAYBE_BOT_ERROR_FOOTER,
                                )),
                            ])
                            .accent_color(WARNING),
                        )])
                        .reply(true)
                        .ephemeral(true)
                        .allowed_mentions(CreateAllowedMentions::new()),
                )
                .await?;
            } else {
                error!("An error occurred whilst executing {invocation_string:?}: {error:#}");
                ctx.send(
                    CreateReply::default()
                        .flags(MessageFlags::IS_COMPONENTS_V2)
                        .components(&[CreateComponent::Container(
                            CreateContainer::new(&[
                                CreateComponent::TextDisplay(CreateTextDisplay::new(
                                    "### An internal error has occurred",
                                )),
                                CreateComponent::TextDisplay(CreateTextDisplay::new(format!(
                                    "```\n{error:?}\n```",
                                ))),
                                CreateComponent::Separator(CreateSeparator::new(true)),
                                CreateComponent::TextDisplay(CreateTextDisplay::new(
                                    BOT_ERROR_FOOTER,
                                )),
                            ])
                            .accent_color(DANGER),
                        )])
                        .reply(true)
                        .ephemeral(true),
                )
                .await?;
            }
        }
        FrameworkError::SubcommandRequired { ctx } => {
            warn!(
                "User attempted to invoke a command, which requires a subcommand, without a subcommand: {:?}",
                ctx.invocation_string(),
            );

            let prefix = ctx.prefix();

            ctx.send(
                CreateReply::default()
                    .flags(MessageFlags::IS_COMPONENTS_V2)
                    .components(&[CreateComponent::Container(
                        CreateContainer::new(&[
                            CreateComponent::TextDisplay(CreateTextDisplay::new(
                                "### Subcommand required",
                            )),
                            CreateComponent::TextDisplay(CreateTextDisplay::new(format!(
                                "You must specify one of the following subcommands:\n\n{}",
                                ctx.command()
                                    .subcommands
                                    .iter()
                                    .map(|subcommand| {
                                        if prefix == ctx.framework().bot_id().mention().to_string()
                                        {
                                            format!("- {prefix} `{}`", subcommand.qualified_name)
                                        } else {
                                            format!("- `{prefix}{}`", subcommand.qualified_name)
                                        }
                                    })
                                    .collect::<Vec<_>>()
                                    .join("\n"),
                            ))),
                        ])
                        .accent_color(WARNING),
                    )])
                    .reply(true)
                    .ephemeral(true),
            )
            .await?;
        }
        FrameworkError::CommandPanic { ctx, .. } => {
            ctx.send(
                CreateReply::default()
                    .flags(MessageFlags::IS_COMPONENTS_V2)
                    .components(&[CreateComponent::Container(
                        CreateContainer::new(&[
                            CreateComponent::TextDisplay(CreateTextDisplay::new(
                                "### Panicked",
                            )),
                            CreateComponent::TextDisplay(CreateTextDisplay::new("A really bad error happened and the bot panicked! You should contact a bot developer and tell them to check the logs.")),
                        ])
                        .accent_color(DANGER),
                    )])
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
                    format!(
                        "Failed to parse {input:?} from {invocation_string:?} into an argument: {error}",
                    )
                }
                None => {
                    format!("Failed to parse an argument from {invocation_string:?}: {error}")
                }
            };

            warn!("{description}");
            ctx.send(
                CreateReply::default()
                    .flags(MessageFlags::IS_COMPONENTS_V2)
                    .components(&[CreateComponent::Container(
                        CreateContainer::new(&[
                            CreateComponent::TextDisplay(CreateTextDisplay::new(
                                "### Failed to parse argument",
                            )),
                            CreateComponent::TextDisplay(CreateTextDisplay::new(description)),
                            CreateComponent::Separator(CreateSeparator::new(true)),
                            CreateComponent::TextDisplay(CreateTextDisplay::new(
                                MAYBE_BOT_ERROR_FOOTER,
                            )),
                        ])
                        .accent_color(WARNING),
                    )])
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
                    .flags(MessageFlags::IS_COMPONENTS_V2)
                    .components(&[CreateComponent::Container(
                        CreateContainer::new(&[
                            CreateComponent::TextDisplay(CreateTextDisplay::new(
                                "### Command structure mismatch",
                            )),
                            CreateComponent::TextDisplay(CreateTextDisplay::new(format!(
                                "```\n{description}\n```"
                            ))),
                            CreateComponent::Separator(CreateSeparator::new(true)),
                            CreateComponent::TextDisplay(CreateTextDisplay::new(BOT_ERROR_FOOTER)),
                        ])
                        .accent_color(DANGER),
                    )])
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
                    .flags(MessageFlags::IS_COMPONENTS_V2)
                    .components(&[CreateComponent::Container(
                        CreateContainer::new(&[
                            CreateComponent::TextDisplay(CreateTextDisplay::new(
                                "### Cooldown hit",
                            )),
                            CreateComponent::TextDisplay(CreateTextDisplay::new(format!("You must wait **~{} seconds** before you can use this command again.", remaining_cooldown.as_secs()))),
                        ])
                        .accent_color(WARNING),
                    )])
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
                    .flags(MessageFlags::IS_COMPONENTS_V2)
                    .components(&[CreateComponent::Container(
                        CreateContainer::new(&[
                            CreateComponent::TextDisplay(CreateTextDisplay::new(
                                "### Lacking bot permissions",
                            )),
                            CreateComponent::TextDisplay(CreateTextDisplay::new(format!("The bot requires the following permissions to execute this command: **{missing_permissions}**"))),
                        ])
                        .accent_color(WARNING),
                    )])
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
                        .flags(MessageFlags::IS_COMPONENTS_V2)
                        .components(&[CreateComponent::Container(
                            CreateContainer::new(&[
                                CreateComponent::TextDisplay(CreateTextDisplay::new(
                                    "### Lacking user permissions",
                                )),
                                CreateComponent::TextDisplay(CreateTextDisplay::new(format!("You must have the following permissions to execute this command: **{missing_permissions}**"))),
                            ])
                            .accent_color(WARNING),
                        )])
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
                        .flags(MessageFlags::IS_COMPONENTS_V2)
                        .components(&[CreateComponent::Container(
                            CreateContainer::new(&[
                                CreateComponent::TextDisplay(CreateTextDisplay::new(
                                    "### Lacking user permissions",
                                )),
                                CreateComponent::TextDisplay(CreateTextDisplay::new("You do not have the permissions needed to execute this command")),
                            ])
                            .accent_color(WARNING),
                        )])
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
                    .flags(MessageFlags::IS_COMPONENTS_V2)
                    .components(&[CreateComponent::Container(
                        CreateContainer::new(&[
                            CreateComponent::TextDisplay(CreateTextDisplay::new(
                                "### Owner only command",
                            )),
                            CreateComponent::TextDisplay(CreateTextDisplay::new(
                                "You must be an owner to use this command.",
                            )),
                        ])
                        .accent_color(WARNING),
                    )])
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
                    .flags(MessageFlags::IS_COMPONENTS_V2)
                    .components(&[CreateComponent::Container(
                        CreateContainer::new(&[
                            CreateComponent::TextDisplay(CreateTextDisplay::new(
                                "### Server only command",
                            )),
                            CreateComponent::TextDisplay(CreateTextDisplay::new(
                                "You cannot use this command outside of a server.",
                            )),
                        ])
                        .accent_color(WARNING),
                    )])
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
                    .flags(MessageFlags::IS_COMPONENTS_V2)
                    .components(&[CreateComponent::Container(
                        CreateContainer::new(&[
                            CreateComponent::TextDisplay(CreateTextDisplay::new(
                                "### DMs only command",
                            )),
                            CreateComponent::TextDisplay(CreateTextDisplay::new(
                                "You cannot use this command outside of DMs.",
                            )),
                        ])
                        .accent_color(WARNING),
                    )])
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
                    .flags(MessageFlags::IS_COMPONENTS_V2)
                    .components(&[CreateComponent::Container(
                        CreateContainer::new(&[
                            CreateComponent::TextDisplay(CreateTextDisplay::new(
                                "### NSFW command",
                            )),
                            CreateComponent::TextDisplay(CreateTextDisplay::new(
                                "You cannot use this command outside of an NSFW channel.",
                            )),
                        ])
                        .accent_color(WARNING),
                    )])
                    .reply(true)
                    .ephemeral(true),
            )
            .await?;
        }
        FrameworkError::CommandCheckFailed { error, ctx, .. } => match error {
            Some(mut error) => {
                dedup_error_chain(&mut error);
                error!("Check errored for {:?}: {error:#}", ctx.invocation_string());
                ctx.send(
                    CreateReply::default()
                        .flags(MessageFlags::IS_COMPONENTS_V2)
                        .components(&[CreateComponent::Container(
                            CreateContainer::new(&[
                                CreateComponent::TextDisplay(CreateTextDisplay::new(
                                    "### Failed to perform check",
                                )),
                                CreateComponent::TextDisplay(CreateTextDisplay::new(format!(
                                    "```\n{error:?}\n```",
                                ))),
                                CreateComponent::Separator(CreateSeparator::new(true)),
                                CreateComponent::TextDisplay(CreateTextDisplay::new(
                                    BOT_ERROR_FOOTER,
                                )),
                            ])
                            .accent_color(DANGER),
                        )])
                        .reply(true)
                        .ephemeral(true),
                )
                .await?;
            }
            None => {
                warn!("Check failed for {:?}", ctx.invocation_string());
            }
        },
        FrameworkError::DynamicPrefix { mut error, msg, .. } => {
            dedup_error_chain(&mut error);
            error!("Dynamic prefix failed for a message: {error:#}\n{msg:#?}");
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
        FrameworkError::PermissionFetchFailed { ctx, .. } => {
            error!("Failed to fetch permissions");
            ctx.send(
                CreateReply::default()
                    .flags(MessageFlags::IS_COMPONENTS_V2)
                    .components(&[CreateComponent::Container(
                        CreateContainer::new(&[
                            CreateComponent::TextDisplay(CreateTextDisplay::new(
                                "### Failed to fetch permissions",
                            )),
                            CreateComponent::TextDisplay(CreateTextDisplay::new("The bot attempted to fetch permissions for you or for the bot, but failed to do so.")),
                            CreateComponent::Separator(CreateSeparator::new(true)),
                            CreateComponent::TextDisplay(CreateTextDisplay::new(
                                BOT_ERROR_FOOTER,
                            )),
                        ])
                        .accent_color(DANGER),
                    )])
                    .reply(true)
                    .ephemeral(true),
            )
            .await?;
        }
        FrameworkError::NonCommandMessage {
            mut error,
            framework: _,
            msg,
            ..
        } => {
            dedup_error_chain(&mut error);
            error!("An error occurred in the non-command message callback: {error:#}\n{msg:#?}");
        }
        other => {
            warn!(
                "Not prepared to handle unfamiliar kind of error, falling back to default `on_error` function",
            );
            poise::builtins::on_error(other).await?;
        }
    }

    Ok(())
}

/// Plug this into your [`poise::FrameworkOptions`] to let
/// [`poise_error`][crate] handle your bot's errors.
///
/// [`anyhow::Error`] is the error type expected to be returned from commands.
/// If you would like to handle some errors before allowing
/// [`poise_error`][crate] to handle any, see [`try_handle_error`].
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
///     .setup(|ctx, _ready, framework| {
///         Box::pin(async move { Ok(()) })
///     })
///     .build();
/// ```
pub fn on_error<U>(error: FrameworkError<'_, U, anyhow::Error>) -> BoxFuture<'_, ()>
where
    U: Send + Sync + 'static,
{
    Box::pin(async move {
        if let Err(mut err) = try_handle_error(error).await {
            dedup_error_chain(&mut err);
            error!("Failed to handle error: {err:#}");
        }
    })
}
