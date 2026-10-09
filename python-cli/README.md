# czar-cli

`czar-cli` is the optional Python distribution that carries the native `czar` CLI binary for `uv`-first local development.

Most Python task projects should depend on the lightweight SDK:

```bash
uv add czar
```

Projects that also need local Czar tooling can opt in explicitly:

```bash
uv add "czar[cli]"
```

This package is not meant to be imported directly. It exists so Python users can get the Rust-built CLI through Python packaging without installing Cargo.
