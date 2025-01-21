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

[`poise`]: https://github.com/serenity-rs/poise
[docs]: https://docs.rs/poise_error
