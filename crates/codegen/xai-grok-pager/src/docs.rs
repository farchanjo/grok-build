//! In-app how-to documentation data.
//!
//! Two content shapes coexist by design:
//!
//! - [`Guide`] (defined in this file) describes short reference guides as
//!   structured blocks and renders them without any markup parser.
//! - [`Doc`] holds the remaining long-form guides as `include_str!` markup,
//!   which the scrollback markdown renderer already handles well.
//!
//! All lookups are zero-allocation over static slices; `DocEntry` exists only for
//! backward compatibility with the TUI doc picker.

/// One row in a guide table. Column count is fixed per table by [`GuideTable::headers`].
#[derive(Debug, Clone, Copy)]
pub struct GuideRow {
    pub cells: &'static [&'static str],
}

/// A table inside a guide section.
#[derive(Debug, Clone, Copy)]
pub struct GuideTable {
    pub headers: &'static [&'static str],
    pub rows: &'static [GuideRow],
}

/// One block of a guide. Ordered; the renderer emits blocks in slice order.
#[derive(Debug, Clone, Copy)]
pub enum GuideBlock {
    /// A short paragraph of prose.
    Text(&'static str),
    /// A two-or-more-column table.
    Table(GuideTable),
    /// A bullet list.
    Bullets(&'static [&'static str]),
    /// A literal code or config snippet, rendered without wrapping the interior.
    Code(&'static str),
}

/// A structurally described guide, rendered without a markup parser.
#[derive(Debug, Clone, Copy)]
pub struct Guide {
    pub filename: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    /// One-line summary used as the `Doc::content` fallback for non-rendering
    /// consumers. It starts with a `#` heading so it stays a valid standalone doc.
    pub summary: &'static str,
    pub blocks: &'static [GuideBlock],
}

/// Convert a [`Guide`] into the [`Doc`] shape the picker and viewer already use.
///
/// The [`Doc::content`] field carries the one-line summary so every existing
/// consumer that reads a `&'static str` — the `/docs` slash command and tests
/// asserting on content shape — keeps working. The interactive viewer prefers
/// [`structured_body`] and re-flattens through [`plain_text`] for disk extraction.
#[must_use]
pub const fn as_doc(g: Guide) -> Doc {
    Doc {
        filename: g.filename,
        title: g.title,
        description: g.description,
        content: g.summary,
    }
}

/// Reference form used by [`find_doc`], which returns `&'static Doc`.
#[must_use]
pub fn as_doc_ref(g: &'static Guide) -> &'static Doc {
    STRUCTURED_DOCS
        .iter()
        .find(|d| d.title.eq_ignore_ascii_case(g.title))
        .unwrap_or_else(|| {
            // Every structured guide has a matching entry; fall back to the
            // markup table so the lookup still answers.
            USER_GUIDE
                .iter()
                .find(|d| d.title.eq_ignore_ascii_case(g.title))
                .expect("structured guide has a markup entry")
        })
}

/// Static mirror of [`STRUCTURED_GUIDES`] in [`Doc`] form, built once.
static STRUCTURED_DOCS: &[Doc] = &[as_doc(KEYBOARD_SHORTCUTS)];

/// Flatten a [`Guide`] into readable plain text for disk extraction.
#[must_use]
pub fn plain_text(g: &Guide) -> String {
    let mut out = String::from(g.summary);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    for block in g.blocks {
        match block {
            GuideBlock::Text(t) => {
                out.push('\n');
                out.push_str(t);
                out.push('\n');
            }
            GuideBlock::Bullets(items) => {
                out.push('\n');
                for item in items.iter() {
                    out.push_str("  - ");
                    out.push_str(item);
                    out.push('\n');
                }
            }
            GuideBlock::Code(code) => {
                out.push('\n');
                for line in code.lines() {
                    out.push_str("    ");
                    out.push_str(line.trim_end());
                    out.push('\n');
                }
            }
            GuideBlock::Table(table) => {
                out.push('\n');
                for row in table.rows {
                    let Some(key) = row.cells.first() else {
                        continue;
                    };
                    let rest = row.cells[1..].join(" / ");
                    if rest.is_empty() {
                        out.push_str(key);
                    } else {
                        out.push_str(key);
                        out.push_str(": ");
                        out.push_str(&rest);
                    }
                    out.push('\n');
                }
            }
        }
    }
    while out.ends_with("\n\n") {
        out.pop();
    }
    out
}

// ── Pilot: keyboard shortcuts ────────────────────────────────────────────────

pub static KEYBOARD_SHORTCUTS: Guide = Guide {
    filename: "03-keyboard-shortcuts.md",
    title: "Keyboard Shortcuts",
    description: "Complete reference for all TUI key bindings",
    summary: "# Keyboard Shortcuts\n\nBuilt-in bindings, not remappable. See the \
              shortcuts help (Ctrl+. or Ctrl+X) for the full per-context table.",
    blocks: &[
        GuideBlock::Text(
            "Bindings are built in and cannot currently be remapped. The \
             \u{21e7} column shows the simple-mode equivalent of a Vim binding.",
        ),
        GuideBlock::Text(
            "Two input modes control scrollback navigation. Simple mode is the \
             default: arrows navigate, Shift+Arrow jumps turns, Space focuses the \
             prompt, any letter auto-focuses it. Vim mode is opt-in with \
             `vim_mode = true` under `[ui]` in config.toml, or `/vim-mode` at runtime.",
        ),
        GuideBlock::Text(
            "Single-letter and Shift+letter scrollback bindings (`j/k`, `h/l`, \
             `g/G`, `L/H`, `y/Y`, `o/O`, `r`, `x`, `e/E`) require Vim mode. Arrow \
             keys, Tab, Esc, Space, PageUp/PageDown, and every Ctrl+letter work in \
             both modes.",
        ),
        GuideBlock::Text("Navigation, with the scrollback focused:"),
        GuideBlock::Table(GuideTable {
            headers: &["Key", "\u{21e7}", "Action"],
            rows: &[
                GuideRow {
                    cells: &["j", "Down", "Select next entry"],
                },
                GuideRow {
                    cells: &["k", "Up", "Select previous entry"],
                },
                GuideRow {
                    cells: &["\u{21e7}L", "Shift+Right", "Jump to next turn"],
                },
                GuideRow {
                    cells: &["\u{21e7}H", "Shift+Left", "Jump to previous turn"],
                },
                GuideRow {
                    cells: &["\u{21e7}J", "", "Jump to next assistant response"],
                },
                GuideRow {
                    cells: &["\u{21e7}K", "", "Jump to previous assistant response"],
                },
                GuideRow {
                    cells: &["g", "", "Go to top of scrollback"],
                },
                GuideRow {
                    cells: &["\u{21e7}G", "", "Go to bottom of scrollback"],
                },
                GuideRow {
                    cells: &["Ctrl+K", "", "Scroll up one line"],
                },
                GuideRow {
                    cells: &["Ctrl+J", "", "Scroll down one line"],
                },
                GuideRow {
                    cells: &["PageUp", "", "Scroll up one page"],
                },
                GuideRow {
                    cells: &["PageDown", "", "Scroll down one page"],
                },
                GuideRow {
                    cells: &["Ctrl+U", "", "Scroll up half page"],
                },
                GuideRow {
                    cells: &["Ctrl+D", "Shift+D in VS Code", "Scroll down half page"],
                },
            ],
        }),
        GuideBlock::Text(
            "PageUp and PageDown also scroll while the prompt is focused, without \
             moving focus or changing the draft. An active prompt history, `@` file \
             search, slash menu, or completion dropdown keeps those keys itself.",
        ),
        GuideBlock::Text("View, with the scrollback focused:"),
        GuideBlock::Table(GuideTable {
            headers: &["Key", "\u{21e7}", "Action"],
            rows: &[
                GuideRow {
                    cells: &["h", "Left", "Collapse selected entry"],
                },
                GuideRow {
                    cells: &["l", "Right", "Expand selected entry"],
                },
                GuideRow {
                    cells: &["e", "", "Toggle fold on selected entry"],
                },
                GuideRow {
                    cells: &["\u{21e7}E", "", "Expand all / collapse all entries"],
                },
                GuideRow {
                    cells: &["Ctrl+E", "", "Expand/collapse all thinking blocks"],
                },
                GuideRow {
                    cells: &["r", "", "Toggle raw markdown on selected entry"],
                },
            ],
        }),
        GuideBlock::Text(
            "`respect_manual_folds = true` under `[scrollback.scroll]` in pager.toml \
             pins a hand-folded block: streaming updates leave it alone, and expanding \
             a block while auto-scroll follows the tail stops following. Resume with \
             \u{21e7}G, `j` on the last entry, scrolling past the bottom, or sending a \
             new prompt. \u{21e7}E clears all pins; Ctrl+E clears pins on thinking blocks.",
        ),
        GuideBlock::Text("Block content, with the scrollback focused:"),
        GuideBlock::Table(GuideTable {
            headers: &["Key", "Action"],
            rows: &[
                GuideRow {
                    cells: &["y", "Copy block content to clipboard"],
                },
                GuideRow {
                    cells: &["\u{21e7}Y", "Copy block metadata (e.g. the shell command)"],
                },
                GuideRow {
                    cells: &["Enter", "Open block content in fullscreen viewer"],
                },
                GuideRow {
                    cells: &["Ctrl+F", "Fullscreen viewer (alt binding)"],
                },
            ],
        }),
        GuideBlock::Text("Focus:"),
        GuideBlock::Table(GuideTable {
            headers: &["Key", "\u{21e7}", "Context", "Action"],
            rows: &[
                GuideRow {
                    cells: &[
                        "Tab",
                        "Space, or i in Vim mode",
                        "Scrollback focused",
                        "Focus the prompt",
                    ],
                },
                GuideRow {
                    cells: &["Tab", "", "Prompt focused", "Focus the scrollback"],
                },
                GuideRow {
                    cells: &["Enter", "", "Prompt focused", "Send the current prompt"],
                },
            ],
        }),
        GuideBlock::Text(
            "Esc is not a focus key. It follows the cancel, clear, and rewind rules \
             below. Overlays, modals, dropdowns, voice, search, and selection steal \
             Esc first.",
        ),
        GuideBlock::Text("Escape:"),
        GuideBlock::Table(GuideTable {
            headers: &["State", "Gesture", "Effect"],
            rows: &[
                GuideRow {
                    cells: &[
                        "Turn running, minimal mode or Vim scrollback off (default)",
                        "Esc",
                        "Cancel immediately; a draft is preserved, unlike Ctrl+C",
                    ],
                },
                GuideRow {
                    cells: &[
                        "Turn running, fullscreen Vim mode",
                        "Esc",
                        "Swallowed no-op; use Ctrl+C",
                    ],
                },
                GuideRow {
                    cells: &["Turn cancelling", "Esc", "Re-sends cancel in every mode"],
                },
                GuideRow {
                    cells: &[
                        "Idle, non-empty prompt, prompt focused",
                        "Esc twice within 800ms",
                        "Clear the prompt; text is saved to history",
                    ],
                },
                GuideRow {
                    cells: &[
                        "Idle, empty prompt, messages present",
                        "Esc twice within 800ms",
                        "Open the rewind picker",
                    ],
                },
                GuideRow {
                    cells: &[
                        "Idle and empty, or scrollback focused with a draft, mode, overlay, or search",
                        "Esc",
                        "Swallowed no-op",
                    ],
                },
            ],
        }),
        GuideBlock::Text(
            "Ctrl+C vs Esc: with a non-empty draft during a turn, Ctrl+C clears the \
             draft and keeps the turn; a second Ctrl+C on an empty prompt cancels. Esc \
             cancels immediately and preserves the draft. Idle non-empty Ctrl+C clears \
             in one press; Esc needs two presses within 800ms.",
        ),
        GuideBlock::Text("Agent-level, from the agent screen:"),
        GuideBlock::Table(GuideTable {
            headers: &["Key", "Context", "Action"],
            rows: &[
                GuideRow {
                    cells: &["Ctrl+P", "Agent screen", "Open the command palette"],
                },
                GuideRow {
                    cells: &["?", "Agent screen", "Command palette (alt binding)"],
                },
                GuideRow {
                    cells: &["Ctrl+M", "Agent screen", "Open the model picker"],
                },
                GuideRow {
                    cells: &["Ctrl+M", "Prompt focused", "Toggle multiline input"],
                },
                GuideRow {
                    cells: &[
                        "Ctrl+C",
                        "Agent screen",
                        "Cancel the turn, or clear a draft first",
                    ],
                },
                GuideRow {
                    cells: &["Ctrl+O", "Agent screen", "Toggle always-approve (YOLO)"],
                },
                GuideRow {
                    cells: &["Ctrl+S", "Agent screen", "Open the session picker"],
                },
                GuideRow {
                    cells: &["Ctrl+;", "Agent screen", "Toggle the prompt queue pane"],
                },
                GuideRow {
                    cells: &[
                        "Ctrl+'",
                        "Windows",
                        "Queue pane alt; some consoles drop Ctrl",
                    ],
                },
                GuideRow {
                    cells: &["Ctrl+4", "Local macOS VS Code family", "Queue pane primary"],
                },
                GuideRow {
                    cells: &[
                        "Shift+Tab",
                        "Prompt focused",
                        "Cycle Normal \u{2192} Plan \u{2192} Always-approve",
                    ],
                },
                GuideRow {
                    cells: &[
                        "Ctrl+B",
                        "Agent screen",
                        "Send the running command to background",
                    ],
                },
                GuideRow {
                    cells: &["Ctrl+T", "Agent screen", "Toggle the todos pane"],
                },
                GuideRow {
                    cells: &["Ctrl+G", "Full TUI", "Toggle the tasks pane"],
                },
                GuideRow {
                    cells: &[
                        "Ctrl+G",
                        "Minimal composer",
                        "Edit the draft in an external editor",
                    ],
                },
                GuideRow {
                    cells: &["Ctrl+L", "Non VS Code family", "Open the extensions modal"],
                },
                GuideRow {
                    cells: &[
                        "Ctrl+L",
                        "VS Code family",
                        "Mid-turn interject; plugins via /plugins",
                    ],
                },
                GuideRow {
                    cells: &[
                        "\u{2191}",
                        "Empty prompt",
                        "Open history with your last prompt",
                    ],
                },
                GuideRow {
                    cells: &["!", "Prompt focused", "Enter shell mode"],
                },
                GuideRow {
                    cells: &[
                        "Ctrl+.",
                        "Agent screen",
                        "Open shortcuts help (needs Kitty protocol)",
                    ],
                },
                GuideRow {
                    cells: &[
                        "Ctrl+X",
                        "Always",
                        "Shortcuts help; works without Kitty protocol",
                    ],
                },
                GuideRow {
                    cells: &["F2", "Agent screen", "Open the settings modal"],
                },
            ],
        }),
        GuideBlock::Text(
            "Minimal-mode external editing resolves $VISUAL, then $EDITOR, then vi. \
             Saving replaces only the draft; an empty file clears it. Drafts with \
             pasted, file, or image chips must be edited in the composer so \
             attachments are not flattened. Run `/doctor` if modified keys misbehave \
             in tmux.",
        ),
        GuideBlock::Text("During an active turn:"),
        GuideBlock::Bullets(&[
            "Plain Enter with text queues a follow-up for after the current turn.",
            "Enter again on the emptied composer sends the top queued follow-up now.",
            "The send-now chord is cancel-and-send: it stops the turn and runs your message next.",
            "With an empty composer and something queued, send-now sends the top row.",
            "While the agent is blocked on a task or subagent, plain Enter delivers immediately.",
        ]),
        GuideBlock::Table(GuideTable {
            headers: &["Terminal", "Primary", "Alternates"],
            rows: &[
                GuideRow {
                    cells: &["Default", "Ctrl+Enter", "Ctrl+I"],
                },
                GuideRow {
                    cells: &["Apple Terminal", "Ctrl+O", "Ctrl+Enter, Ctrl+I"],
                },
                GuideRow {
                    cells: &["VS Code family", "Ctrl+L", "none"],
                },
            ],
        }),
        GuideBlock::Text(
            "In `/multiline` mode, Shift+Enter or Alt+Enter sends while plain Enter \
             inserts a newline \u{2014} except on an empty composer mid-turn with a queued \
             follow-up, where plain Enter still sends the top row.",
        ),
        GuideBlock::Text("Global:"),
        GuideBlock::Table(GuideTable {
            headers: &["Key", "\u{21e7}", "Action", "Confirm"],
            rows: &[
                GuideRow {
                    cells: &[
                        "Ctrl+N",
                        "",
                        "New session (optionally in a worktree)",
                        "Yes",
                    ],
                },
                GuideRow {
                    cells: &["Ctrl+Q", "Ctrl+D", "Quit the application", "Yes"],
                },
            ],
        }),
        GuideBlock::Text(
            "Confirm means press twice within 1000ms. On VS Code family terminals \
             Ctrl+Q is captured by the host, so Ctrl+D is the sole quit key and \
             half-page-down becomes bare Shift+D.",
        ),
        GuideBlock::Text("Welcome screen only:"),
        GuideBlock::Table(GuideTable {
            headers: &["Key", "Action"],
            rows: &[
                GuideRow {
                    cells: &["Ctrl+S", "Resume session (also works in a session)"],
                },
                GuideRow {
                    cells: &["Ctrl+W", "New Worktree dialog, inside a git repository"],
                },
                GuideRow {
                    cells: &["Ctrl+I", "Import Claude settings, when available"],
                },
                GuideRow {
                    cells: &["Ctrl+Shift+I", "Dismiss the Claude import row"],
                },
            ],
        }),
        GuideBlock::Text(
            "Returning to the welcome screen has no key binding; use `/home` \
             (alias `/welcome`) from inside a session.",
        ),
        GuideBlock::Text("Image paste and drag-and-drop:"),
        GuideBlock::Table(GuideTable {
            headers: &["Action", "macOS", "Linux", "Windows"],
            rows: &[
                GuideRow {
                    cells: &[
                        "Drag from file manager",
                        "Finder",
                        "Files / Dolphin",
                        "Explorer",
                    ],
                },
                GuideRow {
                    cells: &["Copy then paste", "Cmd+V", "Ctrl+V", "Ctrl+V"],
                },
                GuideRow {
                    cells: &["Paste clipboard image", "Cmd+V", "Ctrl+V", "Alt+V"],
                },
            ],
        }),
        GuideBlock::Text(
            "Non-image files insert their absolute path as text instead of a chip. \
             Alt+V on Windows bypasses the Windows Terminal interceptor, whose default \
             Ctrl+V drops image clipboards.",
        ),
        GuideBlock::Bullets(&[
            "Ctrl+V reads the CLIPBOARD selection and never falls back to PRIMARY.",
            "An unmodified middle click reads PRIMARY on Linux X11 when DISPLAY is set.",
            "Shift+Insert is the terminal-native way to paste selected text.",
            "Over SSH the remote cannot read the local X11 selection; use Shift+Insert.",
        ]),
        GuideBlock::Text("Quick reference, Vim mode:"),
        GuideBlock::Code(
            "Navigation:  j/k up/down  H/L turns  K/J responses  g/G top/bottom\n\
             Scrolling:   Ctrl+J/K line  Ctrl+U/D half page  PgUp/PgDn page\n\
             Folding:     h/l collapse-expand  e toggle  E all\n\
             Content:     y copy  Y copy cmd  Enter fullscreen\n\
             View:        r raw markdown  Ctrl+E thinking\n\
             Focus:       i, Tab, or Space",
        ),
        GuideBlock::Text("Quick reference, prompt focused:"),
        GuideBlock::Code(
            "Send:           Enter\n\
             Newline:        Shift+Enter or Alt+Enter\n\
             Multiline:      Ctrl+M\n\
             Paste:          Ctrl+V\n\
             Selected text:  Middle click or Shift+Insert (Linux)\n\
             Paste image:    Alt+V (Windows)\n\
             Select all:     Cmd+A (Ghostty only)\n\
             Leave:          Tab\n\
             Cancel:         Ctrl+C\n\
             Clear:          Esc Esc within 800ms\n\
             Rewind:         Esc Esc within 800ms",
        ),
        GuideBlock::Text(
            "Cmd+A is wired up only when the detected terminal is Ghostty; elsewhere \
             the terminal's native select-all applies. On Ghostty, add \
             `keybind = cmd+a=unbind` to its config so the keystroke reaches the TUI.",
        ),
        GuideBlock::Text("Always available:"),
        GuideBlock::Code(
            "Palette:        Ctrl+P or ?\n\
             Model picker:   Ctrl+M\n\
             Cancel:         Ctrl+C\n\
             Always-approve: Ctrl+O\n\
             New session:    Ctrl+N\n\
             Quit:           Ctrl+Q or Ctrl+D",
        ),
    ],
};

/// Docs defined as structures rather than parsed markup.
static STRUCTURED_GUIDES: &[&Guide] = &[&KEYBOARD_SHORTCUTS];

/// Render a structured guide into plain text for the on-disk extraction path.
#[must_use]
pub fn structured_plain_text(title: &str) -> Option<String> {
    STRUCTURED_GUIDES
        .iter()
        .find(|g| g.title.eq_ignore_ascii_case(title))
        .map(|g| plain_text(g))
}

/// Resolve the guide defined as blocks for a title, if one exists.
#[must_use]
pub fn find_structured(title: &str) -> Option<&'static Guide> {
    STRUCTURED_GUIDES
        .iter()
        .find(|g| g.title.eq_ignore_ascii_case(title))
        .copied()
}

/// Content shown by the in-app viewer: heading, prose, tables, lists, snippets.
///
/// Rows of a table are separated by a blank line when the terminal is narrow, so
/// wrapped columns stay readable without a border-heavy layout.
#[must_use]
pub fn structured_body(g: &Guide, width: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let narrow = width < 72;
    out.push(g.title.to_string());
    out.push(String::new());
    for block in g.blocks {
        match block {
            GuideBlock::Text(t) => {
                for line in wrap_text(t, width) {
                    out.push(line);
                }
                out.push(String::new());
            }
            GuideBlock::Bullets(items) => {
                for item in items.iter() {
                    let mut iter = wrap_text(item, width.saturating_sub(4)).into_iter();
                    if let Some(first) = iter.next() {
                        out.push(format!("  \u{b7} {first}"));
                    }
                    for rest in iter {
                        out.push(format!("    {rest}"));
                    }
                }
                out.push(String::new());
            }
            GuideBlock::Code(code) => {
                for line in code.lines() {
                    out.push(format!("    {}", line.trim_end()));
                }
                out.push(String::new());
            }
            GuideBlock::Table(table) => {
                for line in render_table(table, width) {
                    out.push(line);
                    if narrow {
                        out.push(String::new());
                    }
                }
                if !out.is_empty() {
                    out.push(String::new());
                }
            }
        }
    }
    while out.last().is_some_and(|l| l.is_empty()) {
        out.pop();
    }
    out
}

fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    let wrapped = xai_grok_pager_render::render::wrapping::word_wrap_lines_with_joiners(
        std::iter::once(text),
        std::cmp::max(width, 20),
    );
    wrapped
        .0
        .into_iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<Vec<_>>()
                .join("")
        })
        .collect()
}

/// Render a table: one line per row, columns padded, every column wrapped inside
/// its own width so a long value never reflows the rest of the row.
fn render_table(table: &GuideTable, width: usize) -> Vec<String> {
    let cols = table.headers.len();
    if cols == 0 {
        return Vec::new();
    }
    let gap = if cols == 1 { 0 } else { 2 };
    let mut widths = vec![0usize; cols];
    let all_rows: Vec<&'static [&'static str]> = std::iter::once(table.headers)
        .chain(table.rows.iter().map(|r| r.cells))
        .collect();
    for row in all_rows.iter() {
        for (i, c) in row.iter().enumerate().take(cols) {
            widths[i] = widths[i].max(str_width(c));
        }
    }
    // Cap the widest column so the table fits the content area; narrower
    // columns keep their natural width.
    let natural_total: usize = widths.iter().sum::<usize>() + gap * cols.saturating_sub(1);
    let avail = width.max(40);
    if natural_total > avail {
        let widest = widths.iter().copied().max().unwrap_or(0);
        let shrink = natural_total
            .saturating_sub(avail)
            .min(widest.saturating_sub(16));
        if shrink > 0 {
            for w in widths.iter_mut() {
                if *w == widest {
                    *w = widest - shrink;
                }
            }
        }
    }

    let mut out: Vec<String> = Vec::new();
    for row in all_rows.iter() {
        let mut rendered: Vec<Vec<String>> = Vec::with_capacity(cols);
        for col in 0..cols {
            let cell = row.get(col).copied().unwrap_or("");
            rendered.push(wrap_text(cell, widths[col]));
        }
        let height = rendered.iter().map(|c| c.len()).max().unwrap_or(1).max(1);
        for line_idx in 0..height {
            let mut s = String::new();
            for col in 0..cols {
                let text = rendered[col]
                    .get(line_idx)
                    .map(String::as_str)
                    .unwrap_or("");
                s.push_str(text);
                if col + 1 < cols {
                    let pad = widths[col].saturating_sub(str_width(text));
                    for _ in 0..pad {
                        s.push(' ');
                    }
                    for _ in 0..gap {
                        s.push(' ');
                    }
                }
            }
            out.push(s.trim_end().to_string());
        }
    }
    out
}

fn str_width(s: &str) -> usize {
    unicode_width::UnicodeWidthStr::width(s)
}

/// A compile-time document entry. All fields are `&'static str`.
#[derive(Debug, Clone, Copy)]
pub struct Doc {
    pub filename: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub content: &'static str,
}

/// Owned variant for the TUI doc picker (backward compat).
#[derive(Debug, Clone)]
pub struct DocEntry {
    pub title: String,
    pub description: String,
    /// Embedded markdown content.
    pub content: &'static str,
}

impl From<&Doc> for DocEntry {
    fn from(d: &Doc) -> Self {
        Self {
            title: d.title.into(),
            description: d.description.into(),
            content: d.content,
        }
    }
}

// ── Static doc tables ────────────────────────────────────────────────────────

macro_rules! guide {
    ($file:literal, $title:literal, $desc:literal) => {
        Doc {
            filename: $file,
            title: $title,
            description: $desc,
            content: include_str!(concat!("../docs/user-guide/", $file)),
        }
    };
}

pub static USER_GUIDE: &[Doc] = &[
    guide!(
        "01-getting-started.md",
        "Getting Started",
        "Installation, first launch, and basic interaction"
    ),
    guide!(
        "02-authentication.md",
        "Authentication",
        "Browser login, API keys, OIDC, external auth providers"
    ),
    guide!(
        "03-keyboard-shortcuts.md",
        "Keyboard Shortcuts",
        "Complete reference for all TUI key bindings"
    ),
    guide!(
        "04-slash-commands.md",
        "Slash Commands",
        "All / commands, including goals, research, and workflow management"
    ),
    guide!(
        "05-configuration.md",
        "Configuration",
        "config.toml, pager.toml, environment variables, file locations"
    ),
    guide!(
        "06-theming.md",
        "Theming and Appearance",
        "Themes, color support, pager.toml customization"
    ),
    guide!(
        "07-mcp-servers.md",
        "MCP Servers",
        "Setting up external tool integrations via MCP"
    ),
    guide!(
        "08-skills.md",
        "Skills",
        "Creating and using reusable prompt packages"
    ),
    guide!(
        "09-plugins.md",
        "Plugins and Marketplace",
        "Installing, managing, and creating plugin packages"
    ),
    guide!(
        "10-hooks.md",
        "Hooks",
        "Project lifecycle scripts for pre/post tool-use events"
    ),
    guide!(
        "11-custom-models.md",
        "Custom Models",
        "BYOK, Ollama, OpenAI-compatible endpoints"
    ),
    guide!(
        "12-project-rules.md",
        "Project Rules (AGENTS.md)",
        "Per-directory instructions and precedence rules"
    ),
    guide!(
        "13-memory.md",
        "Memory",
        "Cross-session knowledge persistence and search"
    ),
    guide!(
        "14-headless-mode.md",
        "Headless Mode and Scripting",
        "Non-interactive CLI for automation and CI/CD"
    ),
    guide!(
        "15-agent-mode.md",
        "Agent Mode and IDE Integration",
        "ACP stdio transport, WebSocket relay, SDK integration"
    ),
    guide!(
        "16-subagents.md",
        "Subagents and Personas",
        "Spawning parallel child agents with specialized roles"
    ),
    guide!(
        "17-sessions.md",
        "Session Management",
        "Save, load, resume, rewind, and compact sessions"
    ),
    guide!(
        "18-sandbox.md",
        "Sandbox Mode",
        "OS-level filesystem and network isolation"
    ),
    guide!(
        "19-plan-mode.md",
        "Plan Mode",
        "Structured planning with approval dialogs"
    ),
    guide!(
        "20-background-tasks.md",
        "Background Tasks and Monitoring",
        "Background commands, /loop, monitor, scheduler"
    ),
    guide!(
        "21-terminal-support.md",
        "Terminal Support and Troubleshooting",
        "tmux, Byobu, Zellij, SSH, truecolor, clipboard, and diagnostics"
    ),
    guide!(
        "22-permissions-and-safety.md",
        "Permissions and Safety",
        "Tool approval, sandbox, security"
    ),
    guide!(
        "23-dashboard.md",
        "Agent Dashboard",
        "Central overview of local sessions and forks"
    ),
    guide!(
        "24-monitoring-usage.md",
        "Monitoring Usage (External OpenTelemetry)",
        "Customer OTEL export"
    ),
    guide!(
        "25-compaction.md",
        "Compaction Settings",
        "Strategy, trigger policy, band count, and model selection for history summarization"
    ),
    guide!(
        "26-anthropic-provider.md",
        "Anthropic Provider",
        "Anthropic API peer, native client, Files, experimental Claude CLI"
    ),
    guide!(
        "27-anthropic-migration.md",
        "Migrating to Anthropic Peer",
        "Non-destructive migration from custom Messages and env keys"
    ),
    guide!(
        "28-media-understanding.md",
        "Media Understanding",
        "Capability-aware image, audio, and video routing"
    ),
    guide!(
        "29-multi-account-providers.md",
        "Multi-Account Providers",
        "Provider instances, account-qualified models, and lifecycle operations"
    ),
    guide!(
        "30-retrieval-and-prime.md",
        "Retrieval and Prime",
        "Retrieval profiles, prime selection, degradation, and memory boundaries"
    ),
    guide!(
        "31-strict-skills-migration.md",
        "Strict Skills Migration",
        "metadata.grok.* moves, quarantine repair, evals, indexes, and rollback"
    ),
];

/// Non-user-guide reference docs. Separate from USER_GUIDE because they
/// live under `docs/` (not `docs/user-guide/`), are not extracted to disk,
/// and do not follow the NN-*.md managed naming pattern. Bundled via
/// `include_str!` so they are available at runtime without a docs path.
static REFERENCE_DOCS: &[Doc] = &[
    Doc {
        filename: "hooks-and-plugins.md",
        title: "Hooks & Plugins Guide",
        description: "Using hooks, plugins, and marketplace",
        content: include_str!("../docs/hooks-and-plugins.md"),
    },
    Doc {
        filename: "custom-hooks.md",
        title: "Creating Custom Hooks",
        description: "Writing your own hooks and matchers",
        content: include_str!("../docs/custom-hooks.md"),
    },
];

// ── Public API ───────────────────────────────────────────────────────────────

/// Find a doc by title (case-insensitive). Returns the static entry.
///
/// Structured guides are checked first so a guide defined as blocks wins over the
/// markup entry with the same title.
pub fn find_doc(title: &str) -> Option<&'static Doc> {
    if let Some(g) = find_structured(title) {
        return Some(as_doc_ref(g));
    }
    USER_GUIDE
        .iter()
        .chain(REFERENCE_DOCS.iter())
        .find(|d| d.title.eq_ignore_ascii_case(title))
}

/// All doc titles, zero allocation.
pub fn all_titles() -> impl Iterator<Item = &'static str> {
    USER_GUIDE
        .iter()
        .chain(REFERENCE_DOCS.iter())
        .map(|d| d.title)
}

/// One-line fallback content used when no render width is known yet.
///
/// Structured guides render their first paragraph so a width-blind consumer
/// (`ShowReleaseNotes` before the first layout pass) still gets real text.
#[must_use]
pub fn doc_content(doc: &Doc) -> &'static str {
    if doc.content.is_empty() {
        if let Some(g) = find_structured(doc.title) {
            if let Some(GuideBlock::Text(first)) = g.blocks.first() {
                return first;
            }
            return g.summary;
        }
    }
    doc.content
}

/// Returns the content of a how-to document by exact title match (case-insensitive).
///
/// Structured guides are flattened here because this entry point is consumed by
/// non-terminal callers that do not run the renderer.
pub fn get_howto_doc(title: &str) -> Option<&'static str> {
    find_doc(title).map(|d| d.content)
}

/// Returns a list of available how-to titles for the model to choose from.
pub fn list_howto_titles() -> Vec<String> {
    all_titles().map(String::from).collect()
}

/// Returns all docs as owned `DocEntry` values for the TUI doc picker.
pub fn default_howto_entries() -> Vec<DocEntry> {
    USER_GUIDE
        .iter()
        .chain(REFERENCE_DOCS.iter())
        .map(DocEntry::from)
        .collect()
}

/// Flatten one document into plain text for on-disk extraction.
///
/// The in-app viewer renders [`Doc`] content through the structured renderer and
/// never sees this form. Extraction exists for the model-facing path, which reads
/// files from disk and gets the cheapest possible thing: headings become bare
/// section lines, tables become `key: value` pairs, and fenced blocks become
/// indented text. No markup syntax is emitted.
#[must_use]
pub fn plain_text_content(doc: &Doc) -> String {
    if let Some(g) = find_structured(doc.title) {
        return plain_text(g);
    }
    let mut out = String::with_capacity(doc.content.len() + 32);
    let mut in_fence = false;
    for raw in doc.content.lines() {
        let line = raw.trim_end();
        let trimmed = line.trim();
        if trimmed == "```" || trimmed.starts_with("```") {
            in_fence = !in_fence;
            if trimmed.len() > 3 {
                // Opening fence with an info string: skip it, keep the body.
                continue;
            }
            continue;
        }
        if in_fence {
            out.push_str("    ");
            out.push_str(trimmed);
            out.push('\n');
            continue;
        }
        if trimmed.is_empty() || trimmed == "---" {
            if !out.ends_with('\n') {
                out.push('\n');
            }
            continue;
        }
        if let Some(head) = trimmed.strip_prefix("# ") {
            out.push_str(head.trim());
            out.push('\n');
            continue;
        }
        if let Some(head) = trimmed.strip_prefix("## ") {
            out.push('\n');
            out.push_str(head.trim());
            out.push('\n');
            continue;
        }
        if let Some(head) = trimmed
            .strip_prefix("### ")
            .or_else(|| trimmed.strip_prefix("#### "))
        {
            out.push('\n');
            out.push_str(head.trim());
            out.push('\n');
            continue;
        }
        if trimmed.starts_with("> ") {
            out.push_str(trimmed[2..].trim());
            out.push('\n');
            continue;
        }
        if trimmed.starts_with('|') {
            let cells: Vec<&str> = trimmed
                .trim_matches('|')
                .split('|')
                .map(str::trim)
                .filter(|c| !c.is_empty() && !c.chars().all(|ch| ch == '-'))
                .collect();
            if cells.len() >= 2 {
                out.push_str(cells[0]);
                out.push_str(": ");
                out.push_str(cells[1..].join(" / ").trim());
                out.push('\n');
            } else if let Some(one) = cells.first() {
                out.push_str(one);
                out.push('\n');
            }
            continue;
        }
        if let Some(item) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
        {
            out.push_str("  - ");
            out.push_str(item.trim());
            out.push('\n');
            continue;
        }
        out.push_str(trimmed);
        out.push('\n');
    }
    while out.ends_with("\n\n") {
        out.pop();
    }
    out
}

/// Extract user-guide docs to `<grok_home>/docs/user-guide/`.
///
/// Called from the pager binary startup so the model can read them from disk.
pub fn extract_user_guide_docs(grok_home: &std::path::Path) {
    let docs_dir = grok_home.join("docs").join("user-guide");
    if let Err(e) = std::fs::create_dir_all(&docs_dir) {
        tracing::warn!(error = %e, "Failed to create user-guide docs directory");
        return;
    }
    for doc in USER_GUIDE {
        if let Err(e) = std::fs::write(
            docs_dir.join(doc.filename),
            plain_text_content(doc).as_bytes(),
        ) {
            tracing::debug!(error = %e, filename = doc.filename, "Failed to extract user-guide doc");
        }
    }
    // Clean up stale managed docs (files removed from USER_GUIDE since last run).
    // Only remove files matching the managed naming pattern (NN-*.md).
    if let Ok(entries) = std::fs::read_dir(&docs_dir) {
        let valid: std::collections::HashSet<&str> =
            USER_GUIDE.iter().map(|d| d.filename).collect();
        for dir_entry in entries.flatten() {
            if let Some(name) = dir_entry.file_name().to_str() {
                let is_managed = name.len() > 3
                    && name.as_bytes()[0].is_ascii_digit()
                    && name.as_bytes()[1].is_ascii_digit()
                    && name.as_bytes()[2] == b'-'
                    && name.ends_with(".md");
                if is_managed
                    && !valid.contains(name)
                    && let Err(e) = std::fs::remove_file(dir_entry.path())
                {
                    tracing::debug!(error = %e, filename = name, "Failed to remove stale user-guide doc");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_guide_entries_are_valid() {
        for doc in USER_GUIDE {
            assert!(!doc.content.is_empty(), "Doc {} is empty", doc.filename);
            assert!(
                !doc.title.is_empty(),
                "Doc {} has empty title",
                doc.filename
            );
            assert!(
                !doc.description.is_empty(),
                "Doc {} has empty description",
                doc.filename
            );
            assert!(
                doc.content.starts_with('#'),
                "Doc {} should start with a markdown header",
                doc.filename
            );
        }
    }

    #[test]
    fn user_guide_entries_have_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for doc in USER_GUIDE {
            assert!(
                seen.insert(doc.filename),
                "Duplicate doc in list: {}",
                doc.filename
            );
        }
    }

    #[test]
    fn default_howto_entries_includes_all_user_guide_docs() {
        let entries = default_howto_entries();
        assert_eq!(entries.len(), USER_GUIDE.len() + REFERENCE_DOCS.len());
        for (i, doc) in USER_GUIDE.iter().enumerate() {
            assert_eq!(entries[i].title, doc.title, "Entry {} title mismatch", i);
        }
    }

    #[test]
    fn latest_user_guides_are_registered() {
        for filename in [
            "28-media-understanding.md",
            "29-multi-account-providers.md",
            "30-retrieval-and-prime.md",
            "31-strict-skills-migration.md",
        ] {
            assert!(
                USER_GUIDE.iter().any(|doc| doc.filename == filename),
                "{filename} must be available to the in-app browser and managed-doc extraction"
            );
        }
    }

    #[test]
    fn find_doc_is_case_insensitive() {
        let doc = find_doc("getting started").expect("should find Getting Started");
        assert_eq!(doc.title, "Getting Started");
        assert!(find_doc("nonexistent guide").is_none());
    }

    #[test]
    fn all_titles_covers_both_tables() {
        let titles: Vec<_> = all_titles().collect();
        assert_eq!(titles.len(), USER_GUIDE.len() + REFERENCE_DOCS.len());
        // The structured pilot keeps its markup entry so picker ordering, disk
        // extraction, and link auditing are unaffected by the new content path.
        assert!(titles.contains(&"Keyboard Shortcuts"));
    }

    #[test]
    fn structured_guide_wins_and_renders_without_markup() {
        let doc = find_doc("keyboard shortcuts").expect("structured guide is registered");
        assert_eq!(doc.filename, "03-keyboard-shortcuts.md");

        let guide = find_structured(doc.title).expect("structured guide is discoverable");
        let lines = structured_body(guide, 80);
        assert!(
            lines.first().is_some_and(|l| l == "Keyboard Shortcuts"),
            "structured body starts with the title, got {lines:?}",
        );
        let rendered = lines.join("\n");
        assert!(
            rendered.contains("Ctrl+P"),
            "agent-level bindings must be present"
        );
        assert!(
            rendered.contains("Esc twice within 800ms") || rendered.contains("800ms"),
            "escape timing rule must survive layout"
        );
        assert!(
            !rendered.contains("|---"),
            "structured rendering emits no markup table separators"
        );
    }

    #[test]
    fn structured_table_columns_stay_aligned_when_a_cell_wraps() {
        let guide = find_structured("Keyboard Shortcuts").expect("structured guide");
        let lines = structured_body(guide, 80);
        // The Key column of the navigation table must all start at column 0 and
        // a wrapped Action cell must not shift the next row's first column.
        let ctrl_rows: Vec<&String> = lines
            .iter()
            .filter(|l| l.starts_with("Ctrl+K") || l.starts_with("Ctrl+J"))
            .collect();
        assert_eq!(ctrl_rows.len(), 2, "both scroll rows render");
        for line in &ctrl_rows {
            assert!(
                line.contains("Scroll up") || line.contains("Scroll down"),
                "row keeps its action text: {line}"
            );
        }
    }

    #[test]
    fn structured_guide_is_briefer_than_the_markup_it_replaces() {
        let guide = find_structured("Keyboard Shortcuts").expect("structured guide");
        let structured = plain_text(guide);
        let markup = include_str!("../docs/user-guide/03-keyboard-shortcuts.md");
        assert!(
            structured.len() < markup.len(),
            "structured text {} bytes should be smaller than markup {} bytes",
            structured.len(),
            markup.len(),
        );
        assert!(structured.starts_with("# "), "disk form keeps a heading");
    }

    #[test]
    fn get_howto_doc_delegates_to_find_doc() {
        assert!(get_howto_doc("Getting Started").is_some());
        assert!(get_howto_doc("Hooks & Plugins Guide").is_some());
        assert!(get_howto_doc("no such doc").is_none());
    }

    #[test]
    fn list_howto_titles_returns_all() {
        let titles = list_howto_titles();
        assert_eq!(titles.len(), USER_GUIDE.len() + REFERENCE_DOCS.len());
    }

    #[test]
    fn anthropic_user_guide_registered_and_safe() {
        let anth = USER_GUIDE
            .iter()
            .find(|d| d.filename == "26-anthropic-provider.md")
            .expect("26-anthropic-provider.md must be in USER_GUIDE");
        assert!(anth.content.contains("ANTHROPIC_API_KEY"));
        assert!(anth.content.contains("GROK_CLAUDE_CLI_RUNTIME"));
        assert!(anth.content.contains("claude-cli-runtime"));
        assert!(
            anth.content.contains("never the global default"),
            "must document Anthropic is never the global default"
        );
        assert!(
            !anth.content.contains("sk-ant-"),
            "user guide must not embed Anthropic literal key samples"
        );
        assert!(
            anth.content.contains("Library only")
                || anth.content.contains("library only")
                || anth.content.contains("client library only"),
            "must qualify Files as client-library-only / product surface deferred"
        );
        assert!(
            anth.content.contains("product surface deferred")
                || anth.content.contains("Product integration remains"),
            "must state Files product surface is deferred"
        );
        let mig = USER_GUIDE
            .iter()
            .find(|d| d.filename == "27-anthropic-migration.md")
            .expect("27-anthropic-migration.md must be in USER_GUIDE");
        assert!(
            mig.content.contains("No destructive migration"),
            "migration guide must state no destructive migration"
        );
        assert!(!mig.content.contains("sk-ant-"));
    }

    #[test]
    fn user_guide_links_to_existing_files() {
        // Relative-link check for:
        // - same-directory managed NN-*.md
        // - `../providers/*.md` under the pager docs root
        // Ignores http(s), bare anchors, and other out-of-tree `../` paths.
        let names: std::collections::HashSet<&str> =
            USER_GUIDE.iter().map(|d| d.filename).collect();
        let docs_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs");
        let user_guide_dir = docs_root.join("user-guide");
        let providers_dir = docs_root.join("providers");
        let mut checked_provider_links = 0usize;
        for doc in USER_GUIDE {
            for cap in doc.content.split("](") {
                let Some(rest) = cap.split(')').next() else {
                    continue;
                };
                let target = rest.split('#').next().unwrap_or(rest).trim();
                if target.is_empty()
                    || target.starts_with("http://")
                    || target.starts_with("https://")
                    || target.starts_with('#')
                {
                    continue;
                }
                if let Some(name) = target.strip_prefix("../providers/") {
                    if name.ends_with(".md") && !name.contains("..") && !name.contains('/') {
                        let path = providers_dir.join(name);
                        assert!(
                            path.is_file(),
                            "broken providers link {:?} in {} (expected {:?})",
                            rest,
                            doc.filename,
                            path
                        );
                        checked_provider_links += 1;
                    }
                    continue;
                }
                if target.starts_with("../") || target.starts_with('/') {
                    // Out-of-guide paths are not part of this audit.
                    continue;
                }
                if target.ends_with(".md") && !target.contains('/') {
                    assert!(
                        names.contains(target) || user_guide_dir.join(target).is_file(),
                        "broken relative link {:?} in {}",
                        rest,
                        doc.filename
                    );
                }
            }
        }
        // New Anthropic guides must exercise the providers path check.
        assert!(
            checked_provider_links > 0,
            "expected at least one ../providers/*.md link in USER_GUIDE (Anthropic docs)"
        );
        let anth = USER_GUIDE
            .iter()
            .find(|d| d.filename == "26-anthropic-provider.md")
            .unwrap();
        assert!(
            anth.content.contains("../providers/anthropic.md"),
            "26-anthropic-provider.md should link to docs/providers/anthropic.md"
        );
        assert!(
            providers_dir.join("anthropic.md").is_file(),
            "docs/providers/anthropic.md must exist on disk"
        );
    }

    #[test]
    fn extract_writes_docs_and_cleans_stale() {
        let tmp = tempfile::tempdir().unwrap();
        let docs_dir = tmp.path().join("docs").join("user-guide");

        std::fs::create_dir_all(&docs_dir).unwrap();
        std::fs::write(docs_dir.join("99-removed.md"), "stale").unwrap();
        std::fs::write(docs_dir.join("notes.md"), "user notes").unwrap();

        extract_user_guide_docs(tmp.path());

        for doc in USER_GUIDE {
            let path = docs_dir.join(doc.filename);
            assert!(path.exists(), "Expected doc {} to exist", doc.filename);
            assert_eq!(
                std::fs::read_to_string(&path).unwrap(),
                plain_text_content(doc),
                "Content mismatch for {}",
                doc.filename,
            );
        }
        assert!(
            !docs_dir.join("99-removed.md").exists(),
            "Stale doc should be cleaned up"
        );
        assert!(
            docs_dir.join("notes.md").exists(),
            "User file should not be deleted"
        );
    }
}
