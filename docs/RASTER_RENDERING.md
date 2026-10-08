# Raster receipt rendering

The preview uses one UI point per printer dot at the default 203 DPI. Small
images keep the same scale as full-width receipt slices. Only ESC/POS image
modes change the scale; horizontal positioning remains in printer dots. Images
are clipped at the paper edge rather than resized to fit.

`GS v 0` prints a raster image and advances by its height. Adjacent raster slices
are drawn without UI spacing. An explicit `LF` after a slice still feeds the
current line spacing: use `ESC 3 0` when that feed should be zero.

`ESC *` stores a column-format bit image on the current line. Its image height
is counted toward the subsequent line feed. For example, a 24-dot image followed
by `LF` with `ESC 3 24` has no gap; `ESC 3 30` leaves six dots of whitespace.
This accounting survives the server draining receipt elements after TCP reads.

Blank lines and `ESC d n` use the configured line spacing. `ESC J n` feeds `n`
vertical motion units, treated as dots at the default 203 DPI, rather than `n`
arbitrary UI separators. Text lines reserve at least their requested line spacing
and their rendered glyph height.

Regression tests in `src/raster_tests.rs` cover pixel preservation, every TCP
split position in a mixed-width raster receipt, byte-at-a-time input, column-image
line feeds, intentional whitespace, image modes, UI image geometry, and a live
TCP receipt on an ephemeral port:

```sh
cargo test --bin escpresso
```

References:

- [GS v 0](https://download4.epson.biz/sec_pubs/pos/reference_en/escpos/gs_lv_0.html)
- [ESC *](https://download4.epson.biz/sec_pubs/pos/reference_en/escpos/esc_asterisk.html)
- [LF](https://download4.epson.biz/sec_pubs/pos/reference_en/escpos/lf.html)
- [ESC 3](https://download4.epson.biz/sec_pubs/pos/reference_en/escpos/esc_3.html)

The preview remains a standard-mode approximation. Printer-specific motion-unit
settings and page-mode composition are outside this rendering fix.
