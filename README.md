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

<!-- errors screenshot -->

<!-- logs screenshot -->

```rust
/// Fails intentionally
#[command(slash_command)]
async fn error(
    _ctx: Context<'_>,
    #[description = "Kind of error to return"] kind: ErrorKind,
) -> Result<(), poise_error::anyhow::Error> {
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
