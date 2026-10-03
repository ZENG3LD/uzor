# uzor-typography

Markdown subset in, PDF, per-page PNG, DOCX, or PPTX out. Composition goes through `uzor-typeset`, `uzor-text`, `uzor-figures`, and `uzor-export`.

Built-in themes are `light` and `dark`. Built-in presets are `column`, `notice`, `filing`, `long`, and `dense`. Any other `--theme` or `--preset` name is a TOML file under `$PRESS_HOME/themes` or `$PRESS_HOME/presets`. The public binary does not ship marks, wordmarks, or named product themes.

```text
uzor-typography content.md --format doc --theme light --preset column --out out --name sample
```

`--format` is `doc`, `deck`, or `report`. `--emit` is `pdf` (default), `docx`, `both`, or `pptx` (`pptx` requires `deck`).

`publish = false` until the crate is added to the uzor publish list.
