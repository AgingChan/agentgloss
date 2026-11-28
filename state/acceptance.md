# agentgloss acceptance matrix

Evidence was refreshed on 2026-11-28 on macOS arm64 (Rust 1.100, host target
`aarch64-apple-darwin`). The v1.0.2 release gate covers the static musl build,
the editor bridge, and the VS Code extension without changing the sidecar
schema.

Counts: 128 library, 17 CLI integration, 35 TUI integration, 11 bridge
integration, and 1 performance test (192 Rust; two ignored diagnostics), plus
19 VS Code extension tests (211 automated tests total).

| Feature | Evidence |
| --- | --- |
| Markdown parser and source map | `parses_headings_paragraphs_and_paths`, `parses_lists_tables_quotes_and_code`, `empty_crlf_and_no_trailing_newline_inputs`, `multi_paragraph_lists_and_footnotes_are_separate_blocks`, `empty_table_cells_have_inline_source_ranges`, `layout::tests::wraps_and_maps_source`, `media_area_starts_after_the_gutter` |
| List/footnote layout | `adjacent_list_items_have_no_blank_separator`, `nested_items_are_indented_without_blank_separators`, `multi_paragraph_items_indent_continuations`, `wrapped_list_items_use_a_hanging_indent` |
| Review anchors and re-anchoring | `review::tests::exact_and_moved_reanchoring`, `duplicate_quotes_use_context`, `whitespace_and_block_fallback`, `inserted_and_deleted_text_fall_back_to_the_block`, `heading_moves_update_the_heading_path`, `unicode_quotes_and_normalized_whitespace_reanchor`, `jaccard_fallback_requires_half_the_tokens`, `missing_text_goes_stale` |
| Rounds, approval, signoff | `approval_invalidated_by_change`, `signoff_policy_sends_feedback_then_approves`, `tests/cli.rs::request_after_edit_increments_round`, `two_round_end_to_end_flow`, `tests/tui.rs::file_change_reanchors_and_updates_round` |
| Sidecar safety | `unsupported_version_is_rejected_without_backup`, `unsupported_v2_shape_is_rejected_without_backup`, `corrupt_sidecar_is_backed_up`, `lock_prevents_a_second_writer` |
| Prompt/JSON export | `comment_prompt_is_a_stable_golden`, `prompt_sanitizes_control_characters`, `approved_prompt`, `quotes_preserve_blank_lines_and_bodies_stay_verbatim`, `resolve_command_includes_a_custom_sidecar`, `json_filtering_and_exit_code` |
| TUI layout and input | `tests/tui.rs::renders_at_80x24_and_wide`, `renders_at_100x30`, `empty_document_message_is_reachable`, `help_overlay_fits_and_scrolls_at_80x24`, `standard_mode_uses_plain_brackets_for_headings`, `mouse_click_and_drag_select_lines`, `mouse_actions_do_not_bypass_an_open_editor` |
| Comments/suggestions/drafts | `comment_editor_creates_item`, `suggestion_editor_creates_item`, `edit_existing_comment`, `drafts_from_an_older_revision_are_discarded`, `update_methods_validate_and_refresh_timestamps`, `reload_clears_selection_and_recomputes_search` |
| Control-character safety | `sanitize::tests::clean_text_is_borrowed`, `escapes_control_characters`, `replaces_c1_controls_and_keeps_layout_whitespace`, `layout::tests::control_characters_are_display_safe`, `prompt_sanitizes_control_characters` |
| Images and Mermaid | `resolves_and_encodes_local_png`, `rasterizes_svg`, `decodes_webp_and_preserves_pixel_data`, `auto_fallback_retries_the_mmdc_pipeline_after_any_failure`, `cache_key_changes_with_content_and_target_size`, `stale_worker_results_are_discarded_after_clear_encodings`, `tests/tui.rs::partially_visible_media_keeps_a_stable_cache_key` |
| Agent handshake | `tests/cli.rs::request_status_export_resolve_flow`, `wait_returns_feedback_and_times_out`, `wait_returns_approval`, `resolve_validates_the_document`; recipe tests in `agent::tests` including duplicate/orphan marker handling |
| tmux adapter | `tests/cli.rs::tmux_adapter_builds_popup_and_detach_commands`; sentinel propagation and result-file fallback |
| Editor bridge | `tests/bridge.rs`: state purity, sync idempotence, comment/suggestion/edit/resolve/unresolve/delete/reattach/signoff, expect-hash and line-range validation, lock behaviour, corrupt/v2 sidecars, CRLF and missing-newline selections, custom sidecars |
| VS Code extension | 19 Vitest/jsdom tests for annotation ranges, selection reduction, bridge envelopes, link/image security, and the manifest display-name/menu/icon contributions (including the 128 × 128 icon); `npm run check`, `npm run build`, and `npm run package` pass; VSIX installs into an isolated VS Code 1.145 profile |
| Marketplace icon | `render_brand_icon` renders the full-bleed square source at exactly 128 × 128 and fails unless all four corners are opaque ink; the VSIX contains the same PNG; the trusted-publishing workflow publishes tagged releases |
| VS Code entry points | Editor-title preview button, Explorer file context menu, editor-tab context menu, and editor context menu are declared in `vscode/package.json`; `test/manifest.test.ts` guards the contributions |

## Manual checks

1. Rust baseline and full suite: fmt, clippy (`-D warnings`), and all tests
   pass on macOS arm64 with `--target aarch64-apple-darwin`.
2. Bridge end-to-end flow is covered by `tests/bridge.rs` and was exercised
   against a temporary spec; sidecars remain schema version 1.
3. VSIX v1.0.2 built (1 MB) and its manifest verified (version, display name
   "Agent Gloss", branded light/dark editor icons, full-bleed 128 px extension
   icon/gallery banner, and four menu contributions). An earlier VSIX was
   installed into an isolated VS Code profile; the v1.0.2 release asset is
   published on GitHub under publisher `InnopiaPteLtd`.
4. PTY close check passed on 2026-11-28: a `script` PTY ran the TUI, SIGHUP
   produced exit 1, and the sidecar was saved. The panic-hook/mouse-cleanup
   paths are unit-checked in code review, not driven in a debugger.
5. The extension activates in an isolated VS Code 1.145 profile with no
   extension-host errors. Remaining manual/visual check before a release: open
   the webview in a real VS Code window and exercise the rendered selection →
   signoff flow, including Mermaid fidelity.
6. The static musl release gate is v1.0.2; this pass was verified on the macOS
   host target before tagging.

## Verdict (2026-11-28)

All P1 and P3 items from the 2026-11-21 QA report are addressed with tests.
The remaining work is the broader P2 test-matrix expansion, not a feature or
release blocker. The sidecar schema and agent handshake are unchanged.
