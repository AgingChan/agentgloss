# mdreview acceptance matrix

Evidence was produced on 2026-11-21 with the static musl build and the full
`cargo test --all-features` suite. Counts are 82 library, 15 CLI, 30 TUI, and
1 performance test (128 total).

| Feature | Evidence |
| --- | --- |
| Markdown parser and source map | `document::tests::parses_headings_paragraphs_and_paths`, `parses_lists_tables_quotes_and_code`, `empty_crlf_and_no_trailing_newline_inputs`, `parses_task_rules_footnotes_and_nested_lists`, `source_ranges_and_lines_round_trip`, `layout::tests::wraps_and_maps_source`, `emoji_graphemes_are_not_split` |
| Review anchors and re-anchoring | `review::tests::exact_and_moved_reanchoring`, `duplicate_quotes_use_context`, `whitespace_and_block_fallback`, `inserted_and_deleted_text_fall_back_to_the_block`, `heading_moves_update_the_heading_path`, `unicode_quotes_and_normalized_whitespace_reanchor`, `jaccard_fallback_requires_half_the_tokens`, `missing_text_goes_stale` |
| Rounds and approval | `review::tests::approval_invalidated_by_change`, `tests/cli.rs::request_after_edit_increments_round`, `two_round_end_to_end_flow`, `tests/tui.rs::file_change_reanchors_and_updates_round` |
| Prompt/JSON export | `review::export::tests::comment_prompt_is_a_stable_golden`, `approved_prompt`, `approved_all_lists_archived_items_without_action_instructions`, `quotes_preserve_blank_lines_and_bodies_stay_verbatim`, `numbering_follows_anchor_order_not_insertion_order`, `empty_and_stale_only_exports_are_clear`, `json_filtering_and_exit_code`; CLI `request_status_export_resolve_flow`, `export_does_not_mutate_the_review_lifecycle` |
| TUI at 80x24 / 120x40 | `tests/tui.rs::renders_at_80x24_and_wide`, `renders_at_100x30`, `narrow_terminal_shows_focused_panel_overlay`, `tiny_terminal_does_not_panic` |
| Vim, standard, and mouse input | `tui::app::tests::navigation_and_focus`, `block_and_heading_motions`, `standard_mode_navigation_and_comment`; `tests/tui.rs::mouse_click_and_drag_select_lines`, `wheel_and_mouse_disabled`, `shift_tab_cycles_focus_backwards`, `footer` and panel click tests |
| Comments and suggestions | `tui::app::tests::comment_editor_creates_item`, `suggestion_editor_creates_item`, `edit_existing_comment`, `resolve_and_delete_items`, `draft_resumes_after_reopen`, `suggestion_original_has_no_trailing_newline_and_survives_edits`; `review::tests::reattach_refreshes_suggestion_original` |
| Images | `media::tests::resolves_and_encodes_local_png`, `absolute_image_paths_and_raster_formats`, `decodes_webp_and_preserves_pixel_data`, `svg_raster_has_visible_pixels`, `enforces_file_and_pixel_limits`, `rejects_remote_and_missing_images`, `explicit_protocol_overrides_detection`, `derives_font_size_from_terminal_pixels_when_plausible`; `tests/tui.rs::image_media_is_prepared_and_cached`, `loaded_image_rows_do_not_contain_the_media_label`, `remote_image_renders_as_a_link_without_starting_a_worker` |
| Mermaid | `media::tests::renders_a_simple_mermaid_diagram`, `renders_sequence_and_class_diagrams`, `failed_mermaid_render_is_cached`, `fake_mmdc_program_returns_svg`, `auto_engine_falls_back_to_mmdc_when_the_rust_parser_fails`; `tests/tui.rs::invalid_mermaid_renders_source_fallback`, `mermaid_source_engine_renders_source_without_starting_a_worker`, `enter_toggles_only_the_mermaid_block_source` |
| Agent handshake | `tests/cli.rs::request_status_export_resolve_flow`, `wait_returns_feedback_and_times_out`, `wait_returns_approval`, `two_round_end_to_end_flow`; recipe tests in `agent::tests` |
| tmux adapter | `tests/cli.rs::tmux_adapter_builds_popup_and_detach_commands`: popup flags, feedback (21), approved (20), abort (22), popup failure (1/2), result-file fallback, custom sidecar, detached window; a direct `__tui` PTY run wrote the abort sentinel 22 to `MDREVIEW_RESULT_FILE` |
| Static binary | `file` reports `ELF 64-bit LSB pie executable ... static-pie linked`; `ldd` reports `statically linked`; `--version` prints `mdreview 0.1.0` |

## Manual checks

1. `./init.sh` and `./harness/gate.sh` — final gate evidence is recorded in the
   release session.
2. TUI smoke test in a real tmux pane — the release binary rendered Markdown,
   outline/review panes, and a half-block Mermaid diagram; `q` restored the
   terminal and exited 1 (aborted). Fixture: `tests/fixtures/spec.md`.
3. `mdreview export tests/fixtures/spec.md` emits the numbered prompt and exits
   10 when feedback exists; covered by CLI integration tests.
4. Reopen after editing — covered by reload/re-anchor tests; manual re-run is
   documented in `README.md`.
5. Install recipes — covered by `agent::tests` and `tests/cli.rs::install_writes_recipes`.
6. Real `mdreview review --tmux` popup — command construction and all sentinel
   outcomes are covered with a fake `tmux`. `__tui` now writes its outcome to
   `MDREVIEW_RESULT_FILE`, so the adapter no longer depends solely on tmux
   propagating the popup status; a real popup run remains a manual desktop check.
7. Kitty/WezTerm image pixels — not available in this headless WSL session.
   Protocol selection, encoding, and half-block fallback are tested; visual
   confirmation on Kitty/WezTerm remains a manual check for the user's desktop.
8. Mermaid Rust engine and invalid-diagram source fallback — covered by media
   and TUI tests; also visible in the PTY fixture.
9. `harness/qa.sh` now validates the model invocation and report shape, filters
   the harness `[claude-code:...]` warning, and exits non-zero without
   overwriting the last good report. Both success and failure paths were
   exercised with a fake `claude`.
