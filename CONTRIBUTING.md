# Contributing

Bug reports, missing niri options and pull requests are welcome.

1. Set up a build as described in [Building](README.md#building).
2. Make the change on a branch of your fork and run `cargo test --workspace`.
3. Open a pull request against `master`. CI builds and tests it, and checks dependencies
   against the advisory database; both checks must pass before it's merged.

Keep pull requests focused on one thing. For a larger change, open an issue first so we can
agree on the approach.

New niri options go in `crates/schema/src/lib.rs`, with the same names and defaults as niri's
documentation. Changes to how the config is written belong in `crates/doc`, with a test.

Security issues: see [SECURITY.md](SECURITY.md).

By contributing you agree that your work is licensed under the GPL-3.0, like the rest of the project.
