# `poise_error`

An opinionated plug-and-play library for error handling in Discord bots made
with [`poise`].

To get started, plug `poise_error::on_error` into your `poise::FrameworkOptions`
to let `poise_error` handle your bot's errors.

```rust
use poise_error::on_error;

let framework = poise::Framework::builder()
    .options(poise::FrameworkOptions {
        on_error,
        ..Default::default()
    })
    .build();
```

See the [docs] for more information.

## Examples

[Goober Bot] is a Discord bot which uses `poise_error`, here's how it looks:

![Screenshot 2025-01-22 at 6 23 00 PM](https://github.com/user-attachments/assets/aef54d4b-8cde-4d96-aa06-434598fe1326)

![Screenshot 2025-01-22 at 6 24 01 PM](https://github.com/user-attachments/assets/bc4cc74a-9a9b-4d2d-ac5f-e7a5f18d9a02)

```rust
use poise::{ChoiceParameter, command};
use poise_error::{
    UserError,
    anyhow::{self, anyhow, bail},
};

#[derive(ChoiceParameter)]
enum ErrorKind {
    User,
    Internal,
    Panic,
}

/// Fails intentionally
#[command(slash_command)]
async fn error(
    _ctx: poise_error::Context<'_>,
    #[description = "Kind of error to return"] kind: ErrorKind,
) -> anyhow::Result<()> {
    match kind {
        ErrorKind::User => bail!(UserError(
            anyhow!("This is an example of a user error")
                .context("This is an example of extra context")
        )),
        ErrorKind::Internal => Err(anyhow!("This is an example of an internal error")
            .context("This is an example of extra context")),
        ErrorKind::Panic => panic!("This is an example of a panic"),
    }
}
```

[`poise`]: https://github.com/serenity-rs/poise
[docs]: https://docs.rs/poise_error
[Goober Bot]: https://github.com/valentinegb/goober-bot
