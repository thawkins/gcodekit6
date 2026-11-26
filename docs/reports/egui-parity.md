# egui Front End Parity Checklist (M6 cut-over)

Date: 2026-11-26. Branch `claude/egui-m6`, base `6841d84`.

One row per user-visible feature from `docs/user/*.md` and the SPEC.md feature list
(section 2.2 and the panels/dialogs of section 5). Rows marked *(GTK)* are user-visible GTK
features that the docs do not list but that a user of the GTK build would miss.

Status values:

- **Ported**: the egui build has it, and the evidence names a test (`file::test`, test
  names without the `test_` prefix are in `crates/gcodekit5-ui-egui/tests/` unless the path says
  `app/`, which is `crates/gcodekit5-app/tests/`).
- **Gap**: the GTK build has it and the egui build does not.
- **Manual check needed**: the egui code exists but no automated test covers the
  user-visible part, or the check needs real hardware or another OS. No manual checks have
  been run: the GUI was not run for this report.
- **Not applicable**: neither front end has it (the doc describes a feature the GTK build
  never had), or it is not a front-end feature.

## Shell, windows and platform

| Feature | Source doc | egui status | Evidence |
| --- | --- | --- | --- |
| Main window opens maximized with docked panes | SPEC 5.1 | Ported | `test_smoke::main_window_opens_maximized`, `test_workspace::default_layout_has_all_panes` |
| Pane layout persists; small windows keep panes reachable | SPEC 5.1 | Ported | `test_workspace::layout_persists`, `test_workspace::tiny_window_keeps_panes_in_overflow_menu` |
| Menus with every command and its shortcut | SPEC 5.4 | Ported | `test_commands::menu_contains_every_command`, `test_commands::menu_shows_keymap_shortcuts` |
| Every GTK accelerator bound, no conflicts | 90-shortcuts | Ported | `test_commands::every_gtk_accelerator_bound`, `test_commands::no_keymap_conflicts` |
| Quit asks about a running job and unsaved work | 20-gcode-editor | Ported | `test_quit::quit_asks_job_first_then_unsaved_work`, `test_quit::quit_prompts_and_cancel_keeps_app_open` |
| Light / Dark / System theme | 70-settings, SPEC 5.5 | Ported | `test_smoke::dark_palette_applied`, `test_smoke::theme_override_beats_desktop` |
| Translations (UI language, restart on change) | SPEC 6.3 | Ported | `test_translations::every_ui_msgid_is_translated`, `test_settings_window::language_change_requests_restart` |
| HiDPI (1x, 1.5x, 2x hit testing) | SPEC 6.3 | Ported | `test_hidpi::*` (8 tests) |
| App-drawn window frame / system title bar option | *(GTK)* | Ported | `test_frame::*`, `test_settings_window::title_bar_row_follows_decision` |
| About dialog | SPEC 5.3.5 | Ported | `test_commands::settings_and_about_open_windows` |
| Open a G-code file named on the command line | 03-quick-start | Ported | `test_editor::launch_files_open_in_editor` |
| Drag-and-drop a file onto the window | 03-quick-start | Not applicable | No drop handler in the GTK source either |
| Linux build and run | 02-installation | Ported | Whole nextest suite runs on Linux; `test_smoke::app_renders_workspace` |
| Windows and macOS builds | 02-installation, SPEC 2.2 | Manual check needed | Not run on Windows or macOS (CHANGELOG known gap) |
| Windows log-off / macOS logout asks about unsaved work | SPEC 6.2 | Not applicable | Same as GTK: ends without the prompt (CHANGELOG known gap) |

## Connection and devices

| Feature | Source doc | egui status | Evidence |
| --- | --- | --- | --- |
| Serial: refresh ports, pick port, connect/disconnect | 03-quick-start, 04-device-setup | Ported | `test_machine_pane::connect_button_connects`, `app/test_machine_controller::disconnect_emits_disconnected_and_allows_reconnect` |
| TCP/IP connection | 04-device-setup | Manual check needed | Host/port fields and `ConnectionParams::tcp` exist; tests use a loopback transport, never a real socket |
| WebSocket connection | 04-device-setup | Manual check needed | `ConnectionParams::websocket` exists; no test opens a WebSocket |
| Auto-reconnect | 04-device-setup | Manual check needed | Device Manager "Auto Reconnect" and `params.auto_reconnect` are passed through; untested |
| Firmware identification after connect | 04-device-setup | Ported | `test_device_info::device_info_parses_version_build_and_options`, `test_device_info::device_info_shows_reported_firmware_build_and_options` |
| Controller/profile mismatch is reported | 04-device-setup | Ported | `test_device_manager::device_manager_protocol_mismatch_hints_at_the_profile` |
| Device profiles (add, edit, delete, set active) | 04-device-setup | Ported | `test_device_manager::device_manager_add_edit_delete`, `test_device_manager::device_manager_set_active` |
| Device profile loads the GTK file | 04-device-setup | Ported | `test_device_manager::device_manager_gtk_file_loads_and_round_trips` |
| GRBL family controllers | 05-supported-controllers | Ported | `app/test_firmware_settings_model::firmware_settings_grbl_family_detection`, `app/test_device_profiles_model::device_profiles_grbl_family_controllers_save` |
| grblHAL against a real controller or the simulator | 05-supported-controllers | Manual check needed | Never run against the grblHAL simulator (CHANGELOG known gap) |
| TinyG, g2core, Smoothieware, FluidNC streaming | 05-supported-controllers, SPEC 2.2 | Manual check needed | Both front ends stream through the GRBL-style protocol; no test with these firmwares |
| Repetier settings | *(GTK)* | Ported | `test_device_manager::device_manager_repetier_profile_selects_repetier_retrieve`, `test_firmware_settings::firmware_settings_repetier_float_slot_written_as_float` |

## Machine control

| Feature | Source doc | egui status | Evidence |
| --- | --- | --- | --- |
| DRO: machine and work position | 10-machine-control | Ported | `test_machine_pane::dro_shows_two_decimals` |
| DRO in inches | 10-machine-control | Ported | `test_machine_jog::dro_in_inches`, `test_machine_jog::report_inches_converted_for_dro_and_clamp` |
| DRO rotary axes (A, B, C) | 10-machine-control, 6AXIS | Ported | `test_machine_work_zero::rotary_axis_shown_in_degrees`, `test_machine_work_zero::rotary_zero_uses_active_wcs_and_calibration`, `test_machine_work_zero::rotary_zero_refused_when_not_idle_or_job_running`, `app/test_work_zero::zero_rotary_command_uses_p_number_and_calibration` (Zero A/B/C sends `G10 L20 P<n> A<calibration>`) |
| Zero X / Y / Z and Zero All (G10 L20) | 10-machine-control | Ported | `test_machine_work_zero::zero_x_uses_reported_g55`, `test_machine_work_zero::zero_all_needs_confirmation`, `test_machine_work_zero::work_zero_controls_disabled_when_disconnected`, `test_machine_work_zero::zero_refused_when_not_idle`, `test_machine_work_zero::zero_all_refused_when_the_wcs_changed_before_yes`, `test_machine_work_zero::wcs_outside_g54_g59_disables_zero`, `test_machine_work_zero::work_zero_disabled_for_non_grbl_controller`, `app/test_work_zero::zero_axes_command_uses_the_wcs_p_number` (rotary A/B/C zero: see DRO rotary axes) |
| Work coordinate system selector G54-G59 | 10-machine-control, SPEC 5.2.9 | Ported | `test_machine_work_zero::wcs_change_refused_during_stream`, `test_machine_work_zero::console_wcs_change_is_followed_by_zero`, `test_machine_work_zero::stream_selecting_a_wcs_is_followed_by_a_parser_state_query`, `test_machine_work_zero::status_tracks_active_wcs`, `app/test_work_zero::parser_state_wcs_reads_grbl_gc_reply` (the active WCS comes from the controller's `$G` reply) |
| Go to Work Zero | *(GTK)* | Ported | `test_machine_work_zero::go_to_work_zero_refused_when_not_idle`, `app/test_work_zero::go_to_work_zero_command` |
| Enter a position by clicking the DRO | 10-machine-control | Not applicable | Not in the GTK source |
| Step jog with the step presets | 10-machine-control | Ported | `test_machine_pane::jog_x_positive_sends_jog`, `test_machine_jog::jog_step_in_inches` |
| Continuous (press-and-hold) jog | 10-machine-control | Ported | `test_machine_jog::hold_survives_long_press_and_cancels_once_on_release` and 9 more `hold_*` tests |
| Rotary jog (A, B, C) | 10-machine-control, 6AXIS | Ported | `test_machine_rotary_jog::rotary_jog_steps_by_the_rotary_increment`, `test_machine_rotary_jog::rotary_jog_disabled_outside_idle`, `test_machine_rotary_jog::rotary_hold_cancels_on_release`, `test_machine_rotary_jog::rotary_hold_cancelled_when_the_axis_row_goes` (GTK's `-`/`+` buttons in each reported rotary axis row; step in degrees from the rotary jog increment; Continuous holds up to the axis limit) |
| Jog feed rate | 10-machine-control | Ported | `app/test_machine_controller::jog_writes_jog_command` |
| Jog blocked in Alarm and while a job runs | 10-machine-control | Ported | `test_machine_jog::jog_refused_in_home_and_alarm`, `test_machine_jog::hold_cancelled_on_stream_start` |
| Jog limited to machine travel (soft limits) | 10-machine-control, 85-safety | Ported | `test_machine_jog::hold_clamped_to_remaining_travel`, `test_machine_jog::hold_refused_at_limit` |
| Keyboard jog | 10-machine-control, 90-shortcuts | Ported | `test_machine_keyboard_jog::keypad_keys_jog_one_step_each`, `test_machine_keyboard_jog::keys_never_jog_while_a_text_field_has_focus`, `test_machine_keyboard_jog::continuous_key_hold_cancels_once_on_release`, `test_machine_keyboard_jog::continuous_key_hold_cancelled_when_a_text_field_takes_focus` and 4 more, `test_commands::every_gtk_accelerator_bound` (GTK's 8/2/4/6/9/3 in the Machine Control scope; egui, like GTK's `to_unicode`, also takes the top-row digits; the docs' W/A/S/D was never bound. A key press jogs one step, without GTK's auto-repeat. As in GTK the keys work as soon as the Machine Control page is shown, with no click: `test_machine_keyboard_jog::keys_jog_again_after_returning_to_machine_control`) |
| Home all | 03-quick-start, 10-machine-control | Ported | `test_console_pane::console_echoes_home_from_machine_control` |
| Home a single axis | 10-machine-control | Not applicable | Not in the GTK source |
| Unlock ($X) | 10-machine-control | Manual check needed | Button and `Cmd+U` command exist; no test clicks Unlock |
| E-Stop (soft reset), also with Machine Control hidden | 10-machine-control, 85-safety | Ported | `test_status_bar::status_bar_estop_stops_stream_without_machine_control`, `test_console_pane::console_echoes_soft_reset_label` |
| Escape as E-Stop | 10-machine-control, 90-shortcuts | Not applicable | GTK never bound Escape; egui keeps it unbound for safety (`test_machine_pane::escape_is_unbound`); soft reset is `Cmd+Shift+K` (`test_commands::safety_chords_bound_everywhere`) |
| Quick Z-touch and edge probe from Machine Control | PROBING (Quick Probe) | Ported | `test_machine_quick_probe::quick_probe_z_streams_gtk_values_and_shows_the_result`, `test_machine_quick_probe::quick_probe_edges_use_gtk_distance_and_direction`, `test_machine_quick_probe::quick_probe_refused_unless_idle`, `test_machine_quick_probe::quick_probe_disabled_offline_and_during_a_job` (Probe Routines generator and run with GTK's 20 mm / 10 mm at 100 mm/min; the generator adds a 1 mm lift or clearance move after the touch) |
| Machine limits and buffer readout | *(GTK)* | Ported | `test_machine_limits_buffer::machine_limits_follow_grbl_travel`, `test_machine_limits_buffer::rotary_limit_shown_once_axis_reported`, `test_machine_limits_buffer::buffer_shown_from_status_report` |
| Command line in Machine Control | *(GTK)* | Ported | `test_machine_jog::jog_blocked_after_command_line_until_fresh_report` |

## Overrides

| Feature | Source doc | egui status | Evidence |
| --- | --- | --- | --- |
| Feed override +/-1 %, +/-10 %, reset | 12-overrides | Ported | `test_machine_pane::feed_override_plus_10` |
| Spindle override +/-1 %, +/-10 %, reset | 12-overrides | Manual check needed | Same button row as feed; no spindle-specific test |
| Rapid override presets | 12-overrides | Manual check needed | Buttons exist (`overrides.rs` `RAPID`); no test |
| Set an exact override percentage | *(GTK)* | Ported | `test_machine_override_target::feed_set_percent_steps_with_confirmation`, `test_machine_override_target::spindle_set_percent_sends_spindle_bytes`, `app/test_override_target::next_step_waits_for_the_report_then_fails_after_timeout` (feed and spindle, as GTK; one step per `Ov:` confirmation) |

## Console

| Feature | Source doc | egui status | Evidence |
| --- | --- | --- | --- |
| Device messages, sent lines echoed | 15-console | Ported | `test_console_pane::console_shows_received_lines_in_app`, `test_console_pane::console_entry_sends_line_and_echoes_it` |
| Colour by level (info, error, ...) | 15-console | Manual check needed | `received_level` colours lines; no test checks the colour |
| Hide status reports | 15-console | Ported | `test_console_pane::console_filters_status_reports` |
| Per-level filter buttons, timestamps | 15-console | Not applicable | Not in the GTK source |
| Command history (Up/Down) | 15-console | Ported | `app/test_command_history::navigate_up`, `app/test_command_history::navigate_down` |
| Command history kept across sessions | *(GTK)* | Ported | `test_console_pane::test_console_history_kept_across_sessions`, `app/test_command_history::test_history_file_round_trip`, `app/test_command_history::test_history_file_missing_damaged_or_long` (GTK's `command_history.json` in the legacy settings folder, 250 commands, written atomically) |
| Bounded buffer | 15-console | Ported | `test_console_pane::console_buffer_is_bounded` |
| Auto-scroll | 15-console | Manual check needed | egui scroll area sticks to the bottom; no test |
| Clear | 15-console | Manual check needed | Button exists; no test |

## Streaming

| Feature | Source doc | egui status | Evidence |
| --- | --- | --- | --- |
| Send the editor's program | 80-streaming | Ported | `test_editor::stream_start_not_routed_when_editor_sync_cannot_run`, `app/test_machine_controller::stream_does_not_block_caller` |
| Pause / resume (feed hold / cycle start) | 80-streaming | Ported | `app/test_machine_controller::pause_stops_feeding_and_resume_restarts`, `test_machine_pane::safety_chords_work_in_text_field` |
| Stop (soft reset, clear queue) | 80-streaming | Ported | `app/test_machine_controller::stop_soft_resets_and_clears_buffers`, `app/test_machine_busy_guard::stop_resets_only_once_the_machine_has_stopped` |
| Progress in status bar and editor | 80-streaming | Ported | `test_machine_pane::stream_progress_updates_while_dialog_open`, `test_editor::current_line_follows_stream` |
| Alarm ends the stream | 80-streaming | Ported | `app/test_machine_controller::alarm_ends_stream` |
| Elapsed / remaining time | 80-streaming | Ported | `test_status_bar::test_status_bar_time_format`, `test_status_bar::test_status_bar_job_times_estimate`, `test_status_bar::test_status_bar_summary_times_while_streaming`, `test_status_bar::test_status_bar_stream_start_time_follows_stream` (GTK's `HH:MM:SS` and its per-line estimate) |
| Feed / spindle readout in the status bar | *(GTK)* | Ported | `test_status_bar::test_status_bar_feed_spindle_text`, `test_status_bar::test_status_bar_shows_feed_spindle_in_app` (reported feed with two decimals and its unit, as the DRO; GTK showed the commanded feed) |
| File statistics and estimated run time | 20-gcode-editor, 80-streaming | Not applicable | Not in the GTK source |
| Comments stripped, line length checked | *(GTK)* | Ported | `app/test_job_preferences::comments_are_removed_unless_preserved`, `app/test_job_preferences::line_longer_than_maximum_is_refused` |

## G-code editor

| Feature | Source doc | egui status | Evidence |
| --- | --- | --- | --- |
| Open, edit, save, Save As (atomic) | 20-gcode-editor | Ported | `test_editor::open_save_round_trip`, `test_editor::save_as_writes_atomically_and_leaves_no_temp_file` |
| Refuses read-only files | 20-gcode-editor | Ported | `test_editor::save_refuses_read_only_file_and_keeps_content` |
| Syntax highlighting | 20-gcode-editor | Ported | `test_editor::highlight_classes` |
| Find next / previous | 20-gcode-editor | Ported | `test_editor::find_next` |
| Match count | 20-gcode-editor | Ported | `test_editor::test_editor_replace_all_is_one_undo_step` (`Matches: N` follows edits); GTK's "2 of 5" index is not (editor module docs) |
| Replace and Replace All | 20-gcode-editor | Ported | `test_editor::test_editor_replace_all_is_one_undo_step`, `test_editor::test_editor_replace_current_match`, `test_editor::test_find_replace_all_text`, `test_editor::test_editor_replace_refused_while_job_runs`, `test_editor::test_editor_replace_row_fits_at_1280` (GTK's replace row under the find row; one undo step; refused during a job) |
| Case-sensitive toggle | 20-gcode-editor | Not applicable | Not in the GTK source |
| Large files | 20-gcode-editor | Ported | `test_editor::large_file_is_read_only_until_edit_anyway`, `test_editor::large_file_frame_time` |
| Current line follows the stream | 20-gcode-editor | Ported | `test_editor::current_line_follows_stream` |
| Executed-line tick marks | 20-gcode-editor | Not applicable | Not in the GTK source |
| Undo / redo, clipboard, select all | 90-shortcuts | Manual check needed | Text field's own undo and clipboard; no test |
| Read-only while a job runs | *(GTK)* | Ported | `test_editor::editor_read_only_while_job_runs` |
| Clear editor | *(GTK)* | Ported | `test_editor::clear_editor_asks_then_clears`, `test_editor::clear_editor_without_text_asks_nothing`, `test_editor::clear_editor_refused_while_job_runs` (asks whenever the editor holds G-code, as GTK did) |

## Visualizer

| Feature | Source doc | egui status | Evidence |
| --- | --- | --- | --- |
| Rapid and cut moves in their own colours, arcs | 30-visualizer | Ported | `test_visualizer::toolpath_tags_each_segment_with_its_line`, `test_visualizer::snapshot_iso_sample` |
| Orbit, pan, zoom, fit | 30-visualizer | Ported | `test_visualizer::orbit_pan_zoom`, `test_visualizer::fit_contains_bounds` |
| Nav cube and standard views | 30-visualizer | Ported | `test_visualizer::nav_cube_click_front_sets_view`, `test_hidpi::nav_cube_faces_hit_at_every_scale` |
| Keyboard zoom and fit | 30-visualizer | Ported | `test_visualizer::view_commands_move_the_camera` |
| Executed part of the path highlighted | 30-visualizer | Ported | `test_visualizer::progress_colours_acknowledged_lines_only` |
| Tool marker | 30-visualizer | Manual check needed | Follows the work position (module docs); no test |
| Large toolpaths | 30-visualizer | Ported | `test_visualizer::toolpath_fits_buffer_limit`, `test_visualizer::upload_frees_cpu_vertices` |
| 2D mode | SPEC (2D/3D toggle) | Ported | `test_visualizer_2d::2d_button_locks_the_top_view`, `test_visualizer_2d::2d_drag_pans_instead_of_orbiting`, `test_visualizer_2d::program_loaded_in_2d_is_fitted_from_the_top` |
| Show/hide rapids, cuts, grid, tool marker; grid spacing | 30-visualizer | Ported | `test_visualizer_display::sidebar_toggles_change_the_display`, `test_visualizer_display::hidden_rapids_and_cuts_are_not_drawn`, `test_visualizer_display::grid_spacing_and_hidden_grid`, `test_visualizer_display::toolpath_marks_travel` |
| Machine bounds, fit to device | *(GTK)* | Ported | `test_visualizer_bounds::work_area_comes_from_the_profile`, `test_visualizer_bounds::fit_to_device_buttons_fit_the_work_area`, `test_visualizer_bounds::fit_to_device_without_a_profile_uses_the_default_area`, `test_visualizer_bounds::machine_bounds_drawn_when_shown` |
| Stock removal simulation | 30-visualizer, SPEC | Ported | `test_visualizer_stock::toggle_follows_the_setting`, `test_visualizer_stock::simulation_restarts_on_program_change`, `test_visualizer_stock::frame_returns_while_simulating`, `test_visualizer_stock::rendered_cut_lies_in_its_groove`, `test_visualizer_stock::designer_generate_seeds_the_stock`, `test_visualizer_stock::designer_without_stock_keeps_the_stock`, `test_visualizer_stock::dragging_a_field_runs_one_simulation`, `test_visualizer_stock::snapshot_stock_groove` |
| Run preview (play/pause/speed) | *(GTK)* | Ported | `test_visualizer_preview::preview_steps_by_speed_per_tick`, `test_visualizer_preview::preview_bar_buttons_drive_the_preview`, `test_visualizer_preview::preview_plays_in_frames_and_asks_nothing`, `test_visualizer_preview::file_run_in_the_app_shows_the_visualizer_and_sends_nothing`, `test_visualizer_preview::preview_marker_drawn_unless_hidden`, `test_visualizer_preview::preview_path_spacing_as_gtk` (File > Run starts it, as in GTK) |
| Copy cursor coordinates (right-click menu) | *(GTK)* | Ported | `test_visualizer_menu::copy_cursor_coordinates_copies_the_clicked_point`, `test_visualizer_menu::cursor_on_plane_inverts_the_projection`, `test_visualizer_menu::coordinates_text_is_gtk_format`, `test_visualizer_menu::menu_toggles_and_fits` (GTK's menu: fits, copy, Toggle Grid/Machine Bounds/Rapid Moves/Cutting Moves) |
| Intensity statistics | *(GTK)* | Gap | Visualizer module docs |

## Designer

| Feature | Source doc | egui status | Evidence |
| --- | --- | --- | --- |
| Drawing tools (rectangle, circle, ellipse, line, polyline, triangle, polygon) | 40-designer | Ported | `test_designer_canvas::each_drawing_tool_draws_its_shape`, `test_designer_canvas::polyline_double_click_closes_and_escape_cancels` |
| Text tool | 40-designer | Ported | `test_designer_text_tool::*` (7 tests) |
| Gear, sprocket, timing pulley | 40-designer | Ported | `test_designer_properties_shapes::gear_parameters`, `test_designer_properties_shapes::sprocket_pitch_sets_the_standard_roller` |
| Fast shape gallery | 40-designer | Ported | `test_designer_gallery::*` (template icons not ported: text cards) |
| Click, Ctrl+Click, marquee selection | 40-designer | Ported | `test_designer_canvas::click_selects_shape_and_empty_click_deselects`, `test_designer_editing::ctrl_click_toggles_selection`, `test_designer_canvas::marquee_selects_shapes_it_touches` |
| Select all (Ctrl+A) | 40-designer | Manual check needed | No test |
| Move, resize, rotate | 40-designer | Ported | `test_designer_editing::drag_moves_selection_and_undo_restores`, `test_designer_editing::resize_by_corner_handle_keeps_aspect`, `test_designer_editing::rotate_from_context_menu` |
| Exact size, position, rotation | 40-designer | Ported | `test_designer_properties::property_edit_updates_shape`, `test_designer_properties::rotation_is_absolute_for_one_shape_and_locks_its_size` |
| Mirror X / Y | 40-designer | Gap | GTK context menu; egui context menu has Delete, Rotate, Array only |
| Align (left, right, top, bottom, centre) | 40-designer | Gap | GTK "Align" submenu; not in egui |
| Cut / copy / paste shapes | 90-shortcuts | Gap | GTK context menu and shortcuts; egui designer ignores them |
| Boolean union / difference / intersection | *(GTK)* | Gap | GTK context menu; not in egui |
| Linear, circular, grid arrays | 40-designer | Ported | `test_designer_array::array_dialog_creates_n_copies_as_one_undo_step`, `test_designer_array::array_dialog_grid_and_circular` |
| Group / ungroup | 40-designer | Ported | `test_designer_layers::group_and_ungroup_are_one_undo_step_each` |
| Ctrl+G / Ctrl+Shift+G | 40-designer, 90-shortcuts | Gap | Grouping is from the Layers pane only |
| Offset, fillet, chamfer | 40-designer | Ported | `test_designer_properties_shapes::offset_previews_while_typing_and_applies_on_commit`, `test_designer_properties_shapes::path_fillet_and_closed` |
| Layers: hide, lock, rename, reorder | 40-designer | Ported | `test_designer_layers::layer_visibility_hides_shape`, `test_designer_layers::locked_shape_is_drawn_but_not_pickable`, `test_designer_layers::move_buttons_reorder_one_undo_step_each` |
| Import SVG and DXF | 40-designer | Ported | `test_designer_files::import_svg_adds_its_shapes`, `test_designer_files::import_dxf_adds_its_shapes` (still the gcodekit5 readers, not craft-libs: CHANGELOG known gap) |
| Import STL | 40-designer | Gap | `file_ops.rs` module docs |
| Import images, engraving settings, Frame | 40-designer | Ported | `test_designer_files::import_image_in_laser_mode_only`, `test_designer_properties_cam::raster_image_engraving_settings`, `test_designer_files::generate_and_frame_hand_gcode_to_the_editor` |
| Profile and pocket operations, per-shape CAM | 40-designer | Ported | `test_designer_properties_cam::cam_fields_follow_use_global_and_pocket`, `test_designer_toolpath::exported_gcode_cuts_what_the_preview_shows` |
| Adaptive pocket strategy | 40-designer | Manual check needed | "Adaptive" strategy listed; no test selects it |
| Multi-pass depth stepping | 40-designer | Ported | `test_designer_properties_cam::cam_rejects_unsafe_step_and_ramp_values` |
| V-carve, drilling operations | 40-designer | Not applicable | Not in the GTK designer |
| Toolpath preview | 40-designer | Ported | `test_designer_toolpath::show_toolpaths_previews_the_design` |
| Undo / redo | 40-designer | Ported | `test_designer_editing::undo_redo_commands`, `test_designer_app::app_routes_undo_to_active_designer_and_tracks_dirty` |
| Save / open design files | 40-designer | Ported | `test_designer_files::save_then_open_round_trips_the_design` |
| Export G-code and SVG | 40-designer | Ported | `test_designer_files::export_gcode_writes_only_visible_shapes`, `test_designer_files::export_svg_writes_visible_shapes` |
| Device work area on the canvas | *(GTK)* | Gap | egui shows the default 250 x 250 mm (designer module docs) |

## CAM tools

| Feature | Source doc | egui status | Evidence |
| --- | --- | --- | --- |
| Tool picker with categories and search | 50-camtools | Ported | `test_cam_tools::cam_picker_filters_by_category_and_search` |
| Tabbed Box Maker | 50-camtools | Ported | `test_cam_tabbed_box::cam_tabbed_box_generate_matches_gtk` |
| Jigsaw Puzzle | 50-camtools | Ported | `test_cam_jigsaw::cam_jigsaw_generate_matches_gtk` |
| Drill Press | 50-camtools | Ported | `test_cam_drill_press::cam_drill_press_generate_matches_gtk_defaults`, `test_cam_drill_press::cam_drill_press_generate_matches_gtk_custom` |
| Laser Bitmap Engraver | 50-camtools | Ported | `test_cam_bitmap_engraving::cam_bitmap_engraving_matches_gtk` |
| Laser Vector Engraver | 50-camtools | Not applicable | Commented out in the GTK CAM page list |
| Spoilboard Surfacing | 50-camtools | Ported | `test_cam_spoilboard_surfacing::cam_surfacing_generate_matches_gtk_defaults` |
| Spoilboard Grid | 50-camtools | Ported | `test_cam_spoilboard_grid::cam_grid_generate_matches_gtk_defaults` |
| Gerber converter | 50-camtools | Ported | `test_cam_gerber::cam_gerber_generate_matches_gtk` |
| Speeds and Feeds | 50-camtools | Ported | `test_cam_speeds_feeds::cam_speeds_feeds_matches_gtk_defaults` |
| Presets: save, load, GTK files | 50-camtools | Ported | `test_cam_presets::cam_preset_loads_the_gtk_files`, `test_cam_presets::cam_preset_save_and_load_through_the_dialogs` |
| Send to editor, preview | 50-camtools | Ported | `test_cam_tools::send_to_editor_replaces_the_program_and_forgets_the_file`, `test_cam_tools::preview_brings_the_visualizer_to_the_front` |
| Help button per tool | 50-camtools | Ported | `test_help::help_console_and_cam_pages_open_their_topic` |
| Units follow the measurement system | 50-camtools | Ported | `test_cam_jigsaw::cam_jigsaw_follows_the_measurement_system` and the matching test of each page |

## Probing

| Feature | Source doc | egui status | Evidence |
| --- | --- | --- | --- |
| Z-touch, edge, corner, bore, boss, tool length routines | PROBING | Ported | `test_cam_probe_routines::cam_probe_generate_matches_gtk`, `test_cam_probe_routines::cam_probe_run_lines_match_gtk` |
| Run and apply the result to a WCS | PROBING | Ported | `test_cam_probe_run::cam_probe_z_touch_reports_and_applies_in_mm`, `test_cam_probe_run::cam_probe_apply_waits_for_a_successful_run` |
| Stop on error, alarm or disconnect | PROBING | Ported | `test_cam_probe_run::cam_probe_error_reply_stops_the_routine`, `test_cam_probe_run::cam_probe_disconnect_aborts_the_run` |
| Probe defaults and setter plate settings | PROBING | Manual check needed | Routine settings fields exist; no test of saved defaults |

## Tool library, materials and settings

| Feature | Source doc | egui status | Evidence |
| --- | --- | --- | --- |
| Tools: add, edit, delete, persist | 60-tool-library | Ported | `test_tools_manager::tools_manager_create_edit_delete`, `test_tools_manager::tools_manager_gtk_file_loads_and_saves_back` |
| Tool catalog import/export (GTC) | *(GTK)* | Ported | `test_tools_manager::tools_manager_gtc_import_and_export` |
| Materials: add, edit, delete, built-ins | 60-tool-library | Ported | `test_materials_manager::materials_manager_create_edit_delete`, `test_materials_manager::materials_manager_standard_material_edits_as_override` |
| Tool change modes | 60-tool-library | Not applicable | Not in the GTK source |
| Units, theme, language, default directory | 70-settings | Ported | `test_settings_window::units_toggle_persists`, `test_settings_window::every_edited_field_persists` |
| Device type CNC / Laser ($32) | 70-settings | Ported | `test_device_info::device_info_laser_type_writes_32_and_refresh_asks_again` |
| DRO decimals | 70-settings | Ported | Fixed: 2 for mm, 3 for inches, as GTK (`test_machine_pane::dro_shows_two_decimals`) |
| Auto-connect on startup | 70-settings | Not applicable | Not in the GTK source |
| Preferences "Controller" page | *(GTK)* | Gap | egui Preferences has General, Machine, User Interface, File Processing, Advanced |
| Keyboard shortcut editor | 70-settings | Gap | GTK Preferences has a Shortcuts page; egui has no shortcut editor |
| Firmware settings: retrieve, filter, write | 70-settings | Ported | `test_firmware_settings::firmware_settings_retrieve_sends_dump_and_identity_queries`, `test_firmware_settings::firmware_settings_write_sends_exactly_the_line` |
| Firmware settings: export / import to file | 70-settings | Manual check needed | Save/Load buttons exist; no test |

## Safety, diagnostics, help

| Feature | Source doc | egui status | Evidence |
| --- | --- | --- | --- |
| Feed hold | 85-safety-diagnostics | Ported | `test_commands::safety_chords_bound_everywhere` |
| E-Stop state machine, motion interlock | 85-safety-diagnostics | Not applicable | Not in the GTK source |
| Communication, buffer and performance diagnostics panels | 85-safety-diagnostics | Not applicable | Not in the GTK source |
| Help browser with topics, links, images | 87-help-system | Ported | `test_help::help_opens_topic_by_id_and_renders_headings_and_images`, `test_help::help_internal_link_navigates_and_back_forward_return` |
| F1 opens help | 87-help-system | Ported | `test_help::help_f1_command_opens_index` |

## SPEC.md feature list (2.2) not covered above

| Feature | Source doc | egui status | Evidence |
| --- | --- | --- | --- |
| Arc expansion, mesh leveling, transformations | SPEC 2.2 | Not applicable | No UI in either front end |
| Macros and custom buttons | SPEC 2.2, 5.2.10 | Not applicable | Not in the GTK source |
| File validation dialog | SPEC 5.3.3 | Not applicable | Not in the GTK source |
| Plugin system | SPEC 2.2 | Not applicable | Not in either front end |

## Gaps

Ranked by user impact. Sizes are rough estimates for one developer who knows the codebase.

| Rank | Gap | User impact | Rough size |
| --- | --- | --- | --- |
| 4 | Visualizer 2D mode, display toggles, grid spacing, machine bounds, fit to device | Medium | 2-3 days |
| 6 | Designer mirror, align, cut/copy/paste, boolean ops, Ctrl+G | Medium | 2-3 days (designer commands exist in `gcodekit5-designer`) |
| 7 | Visualizer run preview (play/pause/speed) | Low | 1-2 days |
| 8 | Import STL | Low: behind a Settings switch in GTK | 1 day |
| 11 | Preferences Controller page and shortcut editor | Low | 1-2 days |
| 12 | Designer device work area, gallery icons | Low | 1-2 days together |

Manual checks still to run before GTK is deleted (no automated evidence): TCP and
WebSocket connections, auto-reconnect, non-GRBL firmwares and the grblHAL simulator, Windows
and macOS builds, Unlock, spindle and rapid override buttons, console colours and
auto-scroll, editor undo/clipboard, tool marker, designer Select All and adaptive pocketing,
firmware settings export/import, probe defaults.

## Visual

Comparison of the egui pages with the GTK build, after the visual restore (tasks V1 to V7,
2026-11-26). Renders: 1440x900 dark, from the page snapshots in
`crates/gcodekit5-ui-egui/tests/snapshots/*_page_dark.png` (copies in
`target/temp/v7-capture/`, untracked). Pages with a help image (`resources/help_images/`)
were compared with it. The other pages were compared with the GTK source layout, because
there is no screenshot of them. The lists below are what still differs.

**All pages:**

- GTK's page tabs have bold labels and a grey active tab; egui fills the active tab with
  the accent blue.
- Headings use a faux bold (the text is drawn twice), because no bold font is bundled.

**Machine Control** (`machine_control.png`):

- GTK centres the centre column vertically; egui starts it at the top.
- Override rows show the percentage only (GTK: "0.0 mm/min (100%)"), and egui adds a Rapid
  row.
- The readout shows "-" with no report, with two decimals (GTK: "0.000").
- Machine State has no Buffer value. The editor has no "G-Code Editor" header, and no
  clear or save buttons.
- The large state text is not bold. GTK's keyboard-jog hint and its Stop hint are left out.
- In a window narrower than about 1000 pt, egui shrinks the editor column first, then the
  sidebar (down to 140 pt). Then the override rows and the readout stack, and E-STOP
  moves to the top of the centre column. GTK let E-STOP slide under the editor.

**Designer** (`designer.png`):

- Tool and Stock Settings are collapsible sections in egui; GTK uses two dialog buttons.
  The toolbox is therefore taller: View Controls scroll at 900 pt.
- The inspector takes 30% of the width (GTK: 20%), and the object rows carry hide and lock
  toggles that GTK lacks.
- The canvas toolbar has no scrollbar toggle and no "?" shortcut popover.
- The lock toggle shows the open-lock icon in both states.
- Labels differ: "Snap to grid" (GTK: "Snap"), and the "Active tool:" line is shown.

**Visualizer** (`visualizer.png`; V6 has the full list):

- The sidebar has no Toolpath, Guides or Simulation toggles, no 2D/3D switch and no Fit
  to Device. These are features egui lacks (Gaps rank 4).
- There is no Zoom/Center/Cursor status line, no run preview row, and no Toggle
  Scrollbars.
- Cuts are drawn yellow by power (GTK: green).

**CAM Tools** (no help image; compared with `ui/gtk/cam_tools/`):

- The tool list matches GTK: a boxed list with GTK's icons, a title and a caption that
  wraps (the row grows with it), and the details and Open beside it.
- A tool page matches GTK's layout:
  - GTK's header (back, title, help).
  - The description sidebar (40%; Speeds and Feeds: the results, 35%).
  - The form in its own scroll area, in framed groups with bold titles.
  - The action buttons pinned at the bottom right: Load, Save, then the page's actions,
    with Generate (primary) at the right end.
- The sidebar has no second title-3 heading: it would repeat the header's title, and two
  labels with the tool's name would make it ambiguous for screen readers.
- The generated program (an egui-only view) is shown in the sidebar under the description.
- Fields are text fields with the unit beside them, not Adwaita action rows with suffix
  widgets.

**Device Manager** (no help image; compared with `ui/gtk/device_manager/`):

- The layout matches GTK: a sidebar with a header, help, search, a boxed list with the
  Active badge and Add Device. The form has the action bar, a centred tab switcher and a
  framed form.
- GTK's emoji button labels are replaced by icons.
- The "Connected" badge is not shown: the egui list does not track the connected port.
- The sidebar is a fixed 270 pt. GTK's paned starts at 20% and can be dragged.

**Device Config** (no help image; compared with `ui/gtk/config_settings/`):

- The toolbar, filter bar, list header (GTK's column widths), boxed list and status line
  match GTK.
- The Device Info panel is the page's left column (320 pt), as in GTK. Machine > Device
  Info switches to this page.
- The panel lists Build and Options as well as GTK's Name, Firmware and Version. The
  connection icon has no text label: the page's status line states the connection.
- Long units are truncated in the 80 pt Unit column; GTK's labels widen the column.

**CNC Tools** (no help image; compared with `ui/gtk/tools_manager/`):

- The layout matches GTK's three columns: the list sidebar (header, search, type and
  material filters, framed Filters, boxed list), the form (action bar, tab switcher,
  framed form), and the Library sidebar.
- The diameter filters' hints read "Min diameter (mm)" (GTK: "Min Ø (mm)").

**Materials** (no help image; compared with `ui/gtk/materials_manager.rs`):

- The layout matches GTK: a sidebar with the boxed list and New Material under it, and
  the form with its action bar and tab switcher.
- No other difference was found in the source comparison.
