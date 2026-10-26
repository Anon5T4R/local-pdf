Most examples live in the top-level [examples](https://github.com/emilk/egui/tree/main/examples/) folder and use `eframe` to create a native or web window.

This folder contains one stand-alone `egui` example, [`text_perf.rs`](text_perf.rs). It is a CPU-only text/layout/tessellation performance harness: it drives `egui::Context` and `Fonts` directly, writes raw timings and a JSON summary, and does not open a window or render with a GPU. The `skrifa` dev-dependency is used only for a fixture cmap coverage check. Paired benchmark runs must pass the same `--font` file to every build being compared; a normal run should specify `--features default_fonts` or `--font`.

There are also plenty of examples in [the online demo](https://www.egui.rs/#demo). You can find the source code for it at <https://github.com/emilk/egui/tree/main/crates/egui_demo_lib>.

To learn how to set up `eframe` for web and native, go to <https://github.com/emilk/eframe_template/> and follow the instructions there!
