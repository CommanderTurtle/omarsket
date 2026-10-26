# Tasket / Omarsket feature parity

Source audit: 2026-10-18. Baseline: Tasket's locally available source snapshot
[`bbc70d5`](https://github.com/AmirHammouteneEI/ScheduledPasteAndKeys/tree/bbc70d5304786ab337218fbf803f3c8da11fb92b),
whose UI/readme identify version 2.3. This is a feature comparison, not a claim to
have built or run Tasket or Omarchy here. **Present** means source-implemented;
**Partial**, **Absent**, **Different**, and **Additional** keep gaps visible.

Omarsket evidence: [panel/menus](../qml/Panel.qml),
[action editor](../qml/ActionEditor.qml), [task contract](../src/model.rs),
[storage](../src/store.rs), [scheduler](../src/engine.rs),
[controller methods](../src/main.rs), [Wayland worker](../backend/worker.py),
[keyboard layers](../qml/InputLayers.qml). Macrohelp's independent HTTP/overlay
comparison is in its [parity document](https://github.com/CommanderTurtle/macrohelp/blob/main/docs/feature-parity.md).

<details>
<summary>Tasket menus, schedules, actions, data and runtime</summary>

| Feature / menu control | Tasket baseline | Omarsket / Omarchy counterpart |
| --- | --- | --- |
| T01 · Main window | Qt task tabs, toolbar, system tray | Different: native bar popup with task-document tabs; same editing/execution purpose, not identical placement |
| T02 · Open/hide | Tray icon, close-to-tray | Present: bar icon toggles panel; Hide panel / Escape; shell owns placement |
| T03 · Exit / Stop | Quit tray item, per-task stop, global stop | Present: per-job Stop, Stop all, bar right-click; plugin disable stops engine; no Quit menu that exits the whole shell |
| T04 · New task | Add a new task | Present: File → New schedule / + |
| T05 · Open task | Load task dialog | Present: File → Open / manage schedules; Schedules → Open |
| T06 · Rename tab/task | Tab context menu Rename | Present: File → Rename / Schedules → Rename |
| T07 · Close task tab | Tab context menu Close | Present: File → Close tab; save/discard/cancel for unsaved draft |
| T08 · Save | Per-task save button | Present: File → Save and Tasks → Save |
| T09 · Unsaved changes | Save prompt before close/exit | Present for closing tabs; Partial: shell/plugin shutdown cannot present a blocking save-all dialog |
| T10 · Duplicate task | Save/copy/import workflow | Additional explicit File / Schedules → Duplicate |
| T11 · Import schedule | Tasket `.scht` | Present: File → Import .omscht / .scht; native action validation, original retained |
| T12 · Import folder | Load saved tasks | Present: File → Import folder; valid `.scht` / `.omscht` files |
| T13 · First-open migration | Existing `.scht` directory | Present: converts existing schedules to individual `.omscht`, keeps originals |
| T14 · Export native schedule | `.scht` task file | Present: File → Export .omscht |
| T15 · Export Windows schedule | Native `.scht` | Present for the six shared action types; refuses Omarsket-only convenience actions rather than exporting broken Tasket JSON |
| T16 · Delete schedule | Saved-file/task management | Present: Schedules / File → Delete, explicit confirmation; queued plan snapshots survive deletion |
| T17 · Reload outside edits | File loading | Present: File → Reload files; dirty open drafts retained |
| T18 · Collection import/export | Program data + schedules handled separately | Additional: File → Import / Export collection JSON; conflicts rejected, imported permissions/startup disabled |
| T19 · Task name / description | Editable fields | Present |
| T20 · Task delay | Delay/countdown before execution | Present: 0–86400 seconds; History shows waiting/running progress |
| T21 · Execute N times | Count field | Present: Loops field |
| T22 · Infinite loop | Infinite loop ON/OFF button | Present: Loops -1; zero never schedules an empty loop count |
| T23 · Save and schedule | Schedule the Task | Present: Task → Save and schedule / Schedule |
| T24 · Run unsaved copy | Not a dedicated baseline control | Additional: Task → Run temporary copy, no schedule file required |
| T25 · Stop selected task | Task stop button | Present: Task → Stop this schedule; individual job Stop |
| T26 · Action progress | Current action highlight, loop counter | Present: action highlight, job action/loop/countdown text |
| T27 · Follow running action | Toolbar autoscroll | Present: View → Follow running action |
| T28 · Pin window | Stay on top | Different: View → Pin panel prevents normal dismissal; shell layer controls stacking |
| T29 · Light/dark toolbar | Two local theme actions | Native replacement: follows current Omarchy Color/Style, fonts, spacing and scaling; no competing theme switch |
| T30 · Window geometry | Saves size/position | Native replacement: fitted panel bounds and per-output shell ownership; does not overwrite WM geometry/refresh settings |
| T31 · Action insert | Add Paste / Keys / Cursor / System / Other Task / Wait | Present: Add action dropdown includes all six and native conveniences |
| T32 · Action edit | Per-action widgets | Present: expandable ActionCard + ActionEditor |
| T33 · Action reorder | Move actions | Present: action up/down controls |
| T34 · Action duplicate | Duplicate action | Present: card copy control |
| T35 · Action delete | Remove action | Present: card delete control |
| T36 · Duplicate action list | Task tools → Duplicate actions list | Present: Task → Duplicate action list, bounded additional copies |
| T37 · Rescale cursor positions | Task tools → new screen resolution | Present: Task → Rescale cursor positions / Tools; scales embedded/resolved positions |
| T38 · Reusable paste data | `contentId` references program data | Present: Data page + Reusable data selector; `data.json` storage |
| T39 · Reusable key data | `keysSeqId` | Present |
| T40 · Reusable cursor data | `cursorMovsId` | Present |
| T41 · Data editor | Sentences, keys sequences, cursor movements | Present: Data → Kind + same three data editors; create/save/delete |
| T42 · Data export/import format | `programData.ini`, merge with `_other` identities | Partial: JSON import/export with conflict rejection; no direct Qt INI import or automatic `_other` renaming |
| T43 · Startup schedules | Toolbar startup task editor; Windows startup registration | Different: Startup page runs enabled tasks when Omarsket starts; plugin/session enablement belongs to Omarchy, not Windows Run registry |
| T44 · Startup delay/loops | Per-startup-task parameters | Present through each native task's delay/loops; Partial: no separate startup-only overrides independent of the same task |
| T45 · Autorun shortcut | Desktop shortcut dialog | Present: Task → Create launcher; `.desktop` invoking existing runner; no auto-install/autostart |
| T46 · Saved-task folder | Application `saved_tasks/*.scht` | Present: individual `$XDG_DATA_HOME/omarsket/schedules/*.omscht`, configurable directory |
| T47 · About/welcome | Welcome text/version; Qt What's This help | Partial: View → About plus quick start; no exact Qt contextual What's This help |
| T48 · Key-sequence creation | Manual keys selector and recorded sequence tooling | Partial: row-based key names/timing + focused chord capture; not Tasket's full categorized selector UI |
| T49 · Key timing | Timed presses/releases, combo rows, repeats | Present: millisecond start/hold fields and repeated sequence |
| T50 · Key identity | Windows Tasket names / mouse wheel strokes | Present for supported canonical and native keys; firmware Fn is not a synthetic observable key |
| T51 · Cursor movement | Delayed/interpolated positions + optional held keys | Present: path rows, timed capture, keyboard/mouse holds through worker |
| T52 · Paste | Clipboard-backed content and repetition | Present: `wl-copy`, acquisition wait, Shift+Insert; respects terminal paste convention |
| T53 · Wait | Duration in seconds | Present |
| T54 · Run another task | Task name, delay, loops | Present: validated nested task plan; cycles/depth over 16 rejected |
| T55 · Scheduling multiple tasks | Independent TaskThread executors | Different: due jobs share serialized input execution; prevents macros fighting over keyboard/mouse |
| T56 · Stop during input | TaskThread stop | Present: cancellation kills worker process group and rebuilds backend; actual compositor key-release behavior still needs native validation |
| T57 · Scheduled plan snapshot | Tasket loads task for execution | Different: native plan/data/children captured at queue time, later edits do not mutate queued work |
| T58 · History | Task-local execution status | Additional: History page, session-qualified job numbers, errors and completed loops |
| T59 · Backend diagnostics | Windows APIs/Tasket implementation | Present: Tools → Check Wayland backend; capabilities/errors from installed tools and WA |
| T60 · Native input conveniences | Use shared six action forms | Additional: move, click, scroll, hotkey, type; intentionally not exported as Windows `.scht` |
| T61 · Read-only simulation | No equivalent advertised in UI | Additional: engine `--dry-run`, used by process/contract tests; sends no keyboard/mouse events |
| T62 · Local external control | No owner socket in Tasket baseline | Additional: owner-only Unix control socket; can be disabled; no HTTP listener in plugin |
| T63 · HTTP API | Separate Macrohelp Qt daemon | Additional integration: separate Macrohelp Rust daemon talks to owner socket; never bundled into overlay |
| T64 · Backend installation | Native Tasket build | Different: Rust engine + pinned WA fork installed with uv into private runtime; no global Python packages |
| T65 · Network / telemetry | No data collection stated | Present: no telemetry; normal playback is local; explicit open-url/launch actions are user-controlled |

</details>

<details>
<summary>Every system-command action</summary>

These belong to the macro engine, not Macrohelp's overlay. System/file actions need
explicit schedule permission; imported schedules do not grant it automatically.

| Action | Tasket Windows implementation | Omarsket Linux implementation |
| --- | --- | --- |
| S01 · `executeprogram` | QProcess detached executable + argument string | Present: detached executable + `shlex` arguments; no implicit shell interpolation |
| S02 · `shutdown` | Windows shutdown with force option | Present: systemctl poweroff; session authorization remains native |
| S03 · `restart` | Windows shutdown/restart | Present: systemctl reboot |
| S04 · `logoff` | Windows logoff | Present: loginctl terminate current user; no claim that Windows Force semantics are identical |
| S05 · `changevolume` | Windows endpoint volume | Present: wpctl default sink, 0–100 |
| S06 · `changeaudiodevice` | Windows audio endpoint selector | Present: PipeWire numeric ID; not a Windows endpoint ID/name |
| S07 · `killprocess` | taskkill image name, forced | Present: pkill exact name, current UID only; Windows `.exe` names need adjustment |
| S08 · `quitselfprogram` | Closes Tasket; Qt HTTP stub ignores quit | Different: worker asks native engine to shut down; does not exit Omarchy shell |
| S09 · `createfolder` | Recursive mkdir | Present |
| S10 · `deletefolder` | Recursive deletion | Present: refuses home/root ancestry and directory symlink recursion |
| S11 · `createfile` | Create empty file, optional auto-rename | Present: same selectable filename policy |
| S12 · `deletefile` | Remove file | Present with broad-target refusal |
| S13 · `copyfile` | Replace destination file | Present: shutil copy2; Linux paths required |
| S14 · `screenshot` | Save composite screen PNG; optional auto-rename | Present: native grim capture/file output; only used when this backend action is selected |
| S15 · `printscreen` | Windows screen-copy action | Present: grim output → image clipboard |
| S16 · `openfile` | DesktopServices file open | Present: xdg-open |
| S17 · `openfolder` | DesktopServices folder open | Present: xdg-open |
| S18 · `openurl` | DesktopServices URL | Present: xdg-open; allowed schemes restricted |
| S19 · Unknown system action | Undefined action | Different: validates/rejects unsupported names before execution |

</details>

<details>
<summary>Keyboard layers — theme, compatibility, conflicts and authorization</summary>

| Feature / layer | Baseline / native owner | Omarsket implementation |
| --- | --- | --- |
| K01 · Layer navigation | No Tasket keyboard-layer UI | Additional: Heatmap, Macros, Software, Omarchy, Virtual input, Onboard |
| K02 · Shared keyboard visual | Device-dependent physical keys | Present: full-size keyboard/numpad/media/Fn/Copilot legends, optional mouse/side buttons; not a guarantee of emitted firmware events |
| K03 · Theme across all layers | Omarchy shell owns colors/font/scale | Present: shared KeyboardMap, native controls and Color/Style tokens; no local fixed palette |
| K04 · Heatmap connection | Future opt-in Macrohelp session | Deferred: inert placeholder; no shell IPC, connection probe or polling. Planned consumer of a temporary local Macrohelp log |
| K05 · Heatmap values | Not in Tasket | Deferred: key/word visualization from explicitly enabled sessions; no new logging in this patch |
| K06 · Heatmap timers | Not in Tasket | Deferred: explicit start on Macrohelp's side, user-directed log deletion through Omarsket |
| K07 · Heatmap binding conflict | No bind operation in this layer | Not applicable: read-only key visualization; cannot rebind a key |
| K08 · Macros layer | Macro schedules / Macrohelp commands | Present: right-click/Assign a selected modifier chord, choose an existing schedule or import a uniquely named .omscht; native templates and schedule shortcuts |
| K09 · Software layer | Native compositor symbol bindings | Present: command/text templates in the native file; media/Copilot require an actual emitted key/chord |
| K10 · Omarchy layer | Native user `hypr/bindings.lua`, `input.lua` | Present: edits those files directly; no alternate binding registry or generated include |
| K11 · Default restore/read | Packaged defaults, personal overrides | Present: read-only defaults/source plus an inactive-literal-default inventory; no evaluation of dynamic Lua or automatic rewrite of packaged defaults |
| K12 · User named OS profiles / switch OSD | Requested higher-level UI | Absent: native file editor does not yet provide named OS binding profile save/switch or profile OSD |
| K13 · Chord entry | Native keysyms / `code:N` / mouse codes | Present: manual text, physical keyboard/mouse selection and focused recorder |
| K14 · Layout compatibility | Native XKB rules/model/layout/options | Present: existing libxkbcommon resolves physical/symbol collisions across configured layouts; actual device/Fn/IME behavior remains native |
| K15 · Active binding lookup | Current compositor bindings | Present: formatted chord/description/action inventory, command-based layer filters, submap scope labels, free-global-key inventory, modifier-aware board colors and Find in source |
| K16 · Cross-layer macro/software/OS collisions | Same native binding namespace | Present: all three use the same active native list, including physical/symbol overlaps and draft literal bindings |
| K17 · Conflict names | Browser/file-manager/etc. native descriptions | Present: warning lists exact chord and current action description, not a generic duplicate error |
| K18 · Replace occupied chord | Native unbind then bind | Present: Review replacement → explicit Replace bindings / Keep existing; staged native `hl.unbind` lines apply only on Save |
| K19 · Exact unbind identity | Native display chord preserves modifier order | Present for literal chords in inspected native sources: retains original spelling/order, not reconstructed Lua callbacks |
| K20 · Complex native bindings | Functions, hold/release flags, submaps, generated strings | Partial: raw native editor preserves arbitrary Lua; automatic replacement refuses unresolved exact chord. Confirmed unbind removes all variants of the listed exact native chord; UI warns explicitly |
| K21 · Unsaved / outside changes | Native file is authority | Present: draft confirmation guard, expected-content save check, syntax preflight, native reload/configerrors and guarded rollback |
| K22 · Pencil / save / authorization | File ownership and native admin editor | Present: user-owned files in normal editor; privileged files through native terminal + sudoedit; no privileged contents returned to QML |
| K23 · Virtual layer default | tslib ts_uinput touch adapter | Present: immutable ts_uinput touch/pass-through default; never repurposed as keyboard remapper |
| K24 · Virtual keyboard/mouse opt-in | Existing libevdev + kernel uinput | Present: off by default; optional root-owned helper, selected physical device IDs and saved private profiles |
| K25 · Virtual mappings | Source key → output key/chord/disabled | Present: same keyboard/mouse visual; numerical evdev editing; driver capability validation before grab |
| K26 · Virtual/native binding compatibility | Device remap changes compositor input | Present: warns about source/target keys and modifiers used by native bindings; activation confirmation. Does not silently delete OS bindings to hide a hardware remap |
| K27 · Reverse compatibility warning | Native bindings affected by active virtual mapping | Present: Macros/Software/Omarchy editor shows active physical routing warnings |
| K28 · Virtual safety | Native permissions and explicit activation | Present: authorize Start, drop root after opening selected devices, no-new-privileges, private controller lease, Stop/unplug/exit/emergency release |
| K29 · Virtual limitations | Physical hardware capability | Explicit: rejects touch/absolute/force-feedback/virtual-recursive devices; no promise of zero latency; real device validation outstanding |
| K30 · Onboard default | Vendor-specific firmware capabilities | Present: off by default; independently selected Razer/SteelSeries/Logitech providers |
| K31 · Onboard Infer | Existing OpenRazer / ratbagd / OmasteelRGB | Present: read-only discovery of already-running providers; no service activation, package install or network request |
| K32 · Onboard memory write | Device-specific supported firmware operation | Absent: generic firmware save/rebind is not implemented; software macro support is not evidence of onboard memory support |
| K33 · Onboard conflicts | Requires actual firmware binding readback | Partial: reported provider state shown, but no universal hardware keymap or conflict analysis; there are no writable binding controls to falsely confirm |
| K34 · Heatmap/HUD remap boundary | Compositor receives mapped output | Explicit: Macrohelp observes compositor-level output, not an additional hidden raw-device logger |
| K35 · Dependency behavior | User controls installed packages | Present: no automatic system installs/version gates; optional helper/providers must be initialized deliberately |
| K36 · Conflict-analysis limits | Native flags/device scopes and arbitrary Lua | Partial: guided edits check the global/universal active key namespace and literal draft chords; not a full Lua interpreter or proof against every device-scoped, ignore-modifier or generated binding. Raw Lua edits use native syntax/reload checks, not automatic conflict rewriting |

</details>

## Remaining implementation / validation

- [ ] Close unsaved drafts safely across plugin shutdown; match the baseline's
  full contextual help and manual key-picker UI (T09, T47–48).
- [ ] Direct Qt INI data migration and separate startup-only parameter overrides
  if preserving those formats/controls verbatim (T42, T44).
- [ ] Named native-config profiles / switching OSD, without introducing a second
  authoritative OS binding store (K12).
- [ ] Device-specific onboard writes/readback/conflict checks only after selecting
  supported hardware and explicit optional providers (K32–33).
- [ ] Native Omarchy GUI, authorization, IME/layout, input-grab/release and actual
  hardware checks. Host tests and Linux-target compilation are not those tests.

Native binding replacement follows Omarchy's
[personal configuration](https://github.com/omacom/omarchy/blob/quattro/config/hypr/bindings.lua)
and [binding helpers](https://github.com/omacom/omarchy/blob/quattro/default/hypr/helpers.lua).
Exact original chord ordering matters in Hyprland's
[binding registry](https://github.com/hyprwm/Hyprland/blob/main/src/keybinds/Registry.cpp);
the editor never replaces an unknown function with a guessed command string.
