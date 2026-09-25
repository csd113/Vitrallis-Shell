//! Compact native PTY terminal; no external terminal application or GUI framework.
use vitrallis_native::theme::{BACKGROUND, TEXT};
mod command;
mod model;
mod pty;
use model::Terminal;
use sdl2::{keyboard::Keycode, pixels::Color, rect::Rect};
use vitrallis_native::ui::{self, Input, Ui};

/// # Errors
/// Reports SDL initialization errors; PTY/shell failures are shown in a dismissible dialog.
pub fn run() -> Result<(), String> {
    let (options, command) = command::parse(std::env::args_os().skip(1).collect())?;
    let Some(options) = options else {
        return Ok(());
    };
    if options.path.is_some() {
        return Err("Terminal does not accept a file path".into());
    }
    let session = vitrallis_native::ui::Session::new("Terminal", &options)?;
    let creator = session.canvas.texture_creator();
    let mut ui = Ui::new(session, &creator)?;
    let _inbox = ui.inbox("terminal")?;
    let (rows, cols) = model::geometry(ui.width, ui.height, ui.scale);
    let mut terminal = Terminal::new(rows, cols);
    if options.smoke || options.screenshot.is_some() {
        terminal
            .process(b"\x1b[1;36mTerminal\x1b[0m\r\nNative PTY / UTF-8 / bounded scrollback\r\n$ ");
        render(&mut ui, &terminal, false)?;
        ui.finish_preview(&options)?;
        return Ok(());
    }
    let (program, args) = match command::launch(command) {
        Ok(command) => command,
        Err(e) => {
            ui.error(&e.to_string())?;
            return Ok(());
        }
    };
    let process = match pty::Pty::spawn(&program, &args, rows, cols, ui.sdl.event()?.event_sender())
    {
        Ok(pty) => pty,
        Err(e) => {
            ui.error(&format!("Cannot start command: {e}"))?;
            return Ok(());
        }
    };
    let mut session = Session {
        terminal,
        process,
        output: Vec::with_capacity(64 * 1024),
        menu: false,
        scroll: 0,
    };
    render(&mut ui, &session.terminal, false)?;
    ui.present();
    loop {
        let input = ui.wait()?;
        let Some(changed) = session.drain(&mut ui)? else {
            return Ok(());
        };
        let input_changed = !matches!(input, Input::Ignore | Input::Wake);
        if session.input(&mut ui, input)? {
            return Ok(());
        }
        if changed || input_changed {
            render(&mut ui, &session.terminal, session.menu)?;
            ui.present();
        }
    }
}
struct Session {
    terminal: Terminal,
    process: pty::Pty,
    output: Vec<u8>,
    menu: bool,
    scroll: usize,
}
impl Session {
    fn drain(&mut self, ui: &mut Ui) -> Result<Option<bool>, String> {
        let end = self
            .process
            .take(&mut self.output)
            .map_err(|e| e.to_string())?;
        let changed = !self.output.is_empty();
        self.terminal.process(&self.output);
        if let Some(message) = end {
            render(ui, &self.terminal, self.menu)?;
            ui.present();
            ui.choose("Terminal closed", &message, &["Close"])?;
            return Ok(None);
        }
        let replies = &mut self.terminal.parser.callbacks_mut().bytes;
        if !replies.is_empty() {
            self.process.send(replies).map_err(|e| e.to_string())?;
            replies.clear();
        }
        Ok(Some(changed))
    }
    fn scroll(&mut self, delta: isize) {
        self.scroll = self
            .scroll
            .saturating_add_signed(delta)
            .min(model::SCROLLBACK);
        self.terminal
            .parser
            .screen_mut()
            .set_scrollback(self.scroll);
        self.scroll = self.terminal.parser.screen().scrollback();
    }
    fn send(&mut self, ui: &mut Ui, bytes: &[u8]) -> Result<(), String> {
        send(ui, &mut self.process, bytes)?;
        self.scroll = 0;
        self.terminal.parser.screen_mut().set_scrollback(0);
        Ok(())
    }
    fn input(&mut self, ui: &mut Ui, input: Input) -> Result<bool, String> {
        let close = matches!(input, Input::Close)
            || matches!(input,Input::Key(Keycode::Q,m) if ui::ctrl(m)&&ui::shift(m));
        if close {
            return ui
                .choose(
                    "Close terminal?",
                    "The shell and its foreground job will be stopped.",
                    &["Cancel", "Close"],
                )
                .map(|choice| choice == 1);
        }
        let rows = isize::try_from(self.terminal.parser.screen().size().0).unwrap_or(1);
        match input {
            Input::Resize => {
                let (rows, cols) = model::geometry(ui.width, ui.height, ui.scale);
                self.terminal.parser.screen_mut().set_size(rows, cols);
                if let Err(e) = self.process.resize(rows, cols) {
                    ui.error(&format!("Resize: {e}"))?;
                }
            }
            Input::Key(Keycode::F10, m) if ui::shift(m) => self.menu = !self.menu,
            Input::Key(Keycode::PageUp, m) if ui::shift(m) => self.scroll(rows),
            Input::Key(Keycode::PageDown, m) if ui::shift(m) => self.scroll(-rows),
            Input::Scroll(y) => self.scroll(isize::try_from(y).unwrap_or(0) * 3),
            Input::Key(Keycode::Return | Keycode::KpEnter, _) if self.menu => {
                terminal_menu(ui, &mut self.process)?;
                self.menu = false;
            }
            Input::Click(_, y)
                if ui
                    .row_at(
                        y,
                        ui.height - model::status_height(ui.scale),
                        model::status_height(ui.scale),
                        1,
                    )
                    .is_some() =>
            {
                terminal_menu(ui, &mut self.process)?;
            }
            Input::Key(key, mods) => {
                if let Some(bytes) = model::key(
                    key,
                    mods,
                    self.terminal.parser.screen().application_cursor(),
                ) {
                    self.send(ui, &bytes)?;
                }
            }
            Input::Text(text) => {
                if let Some(bytes) = model::text(&text, ui.text_modifiers()) {
                    self.send(ui, &bytes)?;
                }
            }
            _ => {}
        }
        Ok(false)
    }
}
fn send(ui: &mut Ui, process: &mut pty::Pty, bytes: &[u8]) -> Result<(), String> {
    if let Err(error) = process.send(bytes) {
        ui.error(&error.to_string())?;
    }
    Ok(())
}
fn terminal_menu(ui: &mut Ui, process: &mut pty::Pty) -> Result<(), String> {
    match ui.choose(
        "Terminal controls",
        "Shift+Page Up/Down: scrollback\nShift+F10: select this menu\nCtrl+Shift+Q: close with confirmation",
        &["Back", "Ctrl+C", "Ctrl+D", "Tab"],
    )? {
        1 => send(ui, process, &[3]),
        2 => send(ui, process, &[4]),
        3 => send(ui, process, b"\t"),
        _ => Ok(()),
    }
}
fn render(ui: &mut Ui, terminal: &Terminal, menu: bool) -> Result<(), String> {
    let scale = ui.scale;
    let status_h = model::status_height(scale);
    let viewport = ui.height - status_h;
    ui.clear();
    let screen = terminal.parser.screen();
    let (rows, cols) = screen.size();
    // Output fills the screen top-down: no title bar, no decorative chrome.
    for row in 0..rows {
        let y = i32::from(row) * 9 * scale;
        if y + 8 * scale > viewport {
            break;
        }
        for col in 0..cols {
            if let Some(cell) = screen.cell(row, col) {
                let x = i32::from(col) * ui.cell();
                let mut fg = color(cell.fgcolor(), TEXT);
                let mut bg = color(cell.bgcolor(), BACKGROUND);
                if cell.inverse() {
                    std::mem::swap(&mut fg, &mut bg);
                }
                if bg != BACKGROUND {
                    ui.fill(
                        Rect::new(x, y, ui.cell().unsigned_abs(), 9 * scale.unsigned_abs()),
                        bg,
                    )?;
                }
                if !cell.is_wide_continuation() {
                    for ch in cell.contents().chars() {
                        ui.glyph(ch, x, y, fg)?;
                    }
                }
                if cell.underline() {
                    ui.fill(Rect::new(x, y + 8 * scale, ui.cell().unsigned_abs(), 1), fg)?;
                }
            }
        }
    }
    // The cursor stays unmistakable: a filled block while the prompt is live,
    // drawn only when the viewport itself is at the current output.
    if !screen.hide_cursor() && screen.scrollback() == 0 {
        let (row, col) = screen.cursor_position();
        let y = i32::from(row) * 9 * scale;
        if row < rows && col < cols && y + 8 * scale <= viewport {
            let x = i32::from(col) * ui.cell();
            let cell = screen.cell(row, col);
            // A filled block cursor stays visible over every cell colour, and the
            // glyph beneath it is redrawn in the inverted ink.
            ui.fill(
                Rect::new(x, y, ui.cell().unsigned_abs(), 8 * scale.unsigned_abs()),
                vitrallis_native::theme::ACCENT,
            )?;
            if let Some(cell) = cell
                && !cell.is_wide_continuation()
            {
                for ch in cell.contents().chars().filter(|c| *c != ' ') {
                    ui.glyph(ch, x, y, vitrallis_native::theme::BACKGROUND)?;
                }
            }
        }
    }
    status(ui, terminal, menu, status_h)
}

/// Lower status line: title, scrollback position and the two shortcuts that are
/// not discoverable from the prompt.
fn status(ui: &mut Ui, terminal: &Terminal, menu: bool, status_h: i32) -> Result<(), String> {
    let y = ui.height - status_h;
    let scale = ui.scale;
    ui.fill(
        Rect::new(0, y, ui.width.unsigned_abs(), status_h.unsigned_abs()),
        if menu {
            vitrallis_native::theme::SELECTED
        } else {
            vitrallis_native::theme::PANEL
        },
    )?;
    let scrollback = terminal.parser.screen().scrollback();
    let state = if scrollback > 0 {
        format!("scrollback {scrollback} / Shift+PageDown")
    } else {
        "Shift+F10: menu   Ctrl+Shift+Q: close".into()
    };
    ui.text(
        "Terminal",
        5,
        y + scale,
        ui.width / 3,
        vitrallis_native::theme::ACCENT,
    )?;
    ui.text(
        &state,
        5 + (ui.width / 3),
        y + scale,
        ui.width - 10 - (ui.width / 3),
        vitrallis_native::theme::MUTED,
    )
}
fn color(color: vt100::Color, default: Color) -> Color {
    const BASE: [(u8, u8, u8); 16] = [
        (20, 29, 38),
        (225, 87, 96),
        (119, 208, 137),
        (232, 191, 104),
        (116, 162, 236),
        (188, 134, 224),
        (102, 209, 204),
        (223, 230, 235),
        (107, 124, 139),
        (255, 125, 134),
        (157, 236, 166),
        (255, 222, 145),
        (151, 191, 255),
        (219, 170, 250),
        (150, 241, 236),
        (255, 255, 255),
    ];
    match color {
        vt100::Color::Default => default,
        vt100::Color::Rgb(r, g, b) => Color::RGB(r, g, b),
        vt100::Color::Idx(index) => {
            let (r, g, b) = if index < 16 {
                BASE[usize::from(index)]
            } else if index >= 232 {
                let value = 8 + 10 * (index - 232);
                (value, value, value)
            } else {
                let n = index - 16;
                let part = |p| if p == 0 { 0 } else { 55 + 40 * p };
                (part(n / 36), part(n / 6 % 6), part(n % 6))
            };
            Color::RGB(r, g, b)
        }
    }
}
