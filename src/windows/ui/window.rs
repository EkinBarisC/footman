//! The tray and the settings window (DESIGN.md §10).
//!
//! One window, two tabs, opened from the tray, and hidden rather than closed —
//! Footman goes on watching the keyboard whether or not anyone is looking at
//! it. The window starts hidden, so the event loop is driven by a heartbeat
//! rather than by anything on screen.
//!
//! A Chord is never typed as a string here. It is recorded by pressing the key,
//! which costs nothing because the settings window is focused while it happens
//! and egui already reports what was pressed.

use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use eframe::egui;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};

use super::keys::key_named;
use crate::windows::{Hook, Installed, applications, init_thread, raise_named};
use crate::{
    Action, Chord, Click, Core, Duty, Key, Modifiers, Notice, Settings, TapAction, TrayEffect,
};

/// The window title, and so the name `raise_named` finds it by.
const TITLE: &str = "Footman";

/// Runs the tray and the settings window on the calling thread. Blocks.
pub fn run(
    path: PathBuf,
    settings: Settings,
    hook: Hook,
    duties: Receiver<Duty>,
    duty: Duty,
) -> Result<(), String> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(TITLE)
            .with_inner_size([820.0, 560.0])
            .with_min_inner_size([640.0, 360.0])
            // Tray-resident: Footman is running long before anyone asks to see
            // it, and goes on running after they stop looking.
            .with_visible(false),
        ..Default::default()
    };

    eframe::run_native(
        TITLE,
        options,
        Box::new(move |cc| {
            let app = Window::new(cc, path, settings, hook, duties, duty)?;
            Ok(Box::new(app))
        }),
    )
    .map_err(|error| error.to_string())
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum Tab {
    Bindings,
    General,
}

struct Window {
    path: PathBuf,
    settings: Settings,
    hook: Hook,
    duties: Receiver<Duty>,
    duty: Duty,

    tray: TrayIcon,
    pause: MenuItem,
    settings_entry: MenuItem,
    quit: MenuItem,

    tab: Tab,
    /// The row whose Chord is being recorded, if any.
    capturing: Option<usize>,
    /// The row choosing an application, if any.
    picking: Option<usize>,
    /// Loaded the first time the picker is opened: asking the shell for every
    /// installed application takes long enough to be worth not doing at start-up.
    installed: Option<Vec<Installed>>,
    filter: String,

    status: Notice,
    /// Whether the form is saying what is wrong with it yet.
    ///
    /// A row is born wrong — no key chosen, nothing to do — and a form that
    /// says so the instant the row appears is telling the user off for a state
    /// it put them in. So it holds its tongue until asked to save, which is the
    /// first moment the answer matters.
    announce_problems: bool,
    shown: bool,
    quitting: bool,
}

impl Window {
    fn new(
        cc: &eframe::CreationContext<'_>,
        path: PathBuf,
        settings: Settings,
        hook: Hook,
        duties: Receiver<Duty>,
        duty: Duty,
    ) -> Result<Self, String> {
        init_thread();
        outline_widgets(&cc.egui_ctx);

        // The window starts hidden, so nothing on screen is asking for frames.
        // Without a heartbeat the tray would stop responding.
        let ctx = cc.egui_ctx.clone();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_millis(100));
                ctx.request_repaint();
            }
        });

        let settings_entry = MenuItem::new("Settings…", true, None);
        let pause = MenuItem::new(duty.pause_label(), true, None);
        let quit = MenuItem::new("Quit", true, None);

        let menu = Menu::new();
        let separator = PredefinedMenuItem::separator();
        for item in [
            &settings_entry as &dyn tray_icon::menu::IsMenuItem,
            &pause,
            &separator,
            &quit,
        ] {
            menu.append(item).map_err(|error| error.to_string())?;
        }

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip(duty.tooltip())
            .with_icon(icon_of(duty)?)
            .build()
            .map_err(|error| error.to_string())?;

        Ok(Window {
            path,
            settings,
            hook,
            duties,
            duty,
            tray,
            pause,
            settings_entry,
            quit,
            tab: Tab::Bindings,
            capturing: None,
            picking: None,
            installed: None,
            filter: String::new(),
            status: Notice::default(),
            announce_problems: false,
            shown: false,
            quitting: false,
        })
    }

    fn show(&mut self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        // Windows refuses the foreground to a background process, and Footman
        // is one — the same refusal the App Action has to work around, and the
        // same way around it.
        self.shown = true;
    }

    fn hide(&mut self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        self.shown = false;
        // Whatever was being said is over. The window is hidden rather than
        // closed, so anything left here would still be on screen the next time
        // it is opened.
        self.status.clear();
    }

    fn drain_duties(&mut self) {
        let mut changed = false;
        while let Ok(duty) = self.duties.try_recv() {
            self.duty = duty;
            changed = true;
        }
        if changed {
            self.pause.set_text(self.duty.pause_label());
            let _ = self.tray.set_tooltip(Some(self.duty.tooltip()));
            let _ = self.tray.set_icon(icon_of(self.duty).ok());
        }
    }

    fn drain_tray(&mut self, ctx: &egui::Context) {
        // A left click on the icon opens the settings, as §10 says.
        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if let TrayIconEvent::Click {
                button: tray_icon::MouseButton::Left,
                button_state: tray_icon::MouseButtonState::Up,
                ..
            } = event
            {
                self.show(ctx);
            }
        }

        while let Ok(event) = MenuEvent::receiver().try_recv() {
            let click = if event.id == self.settings_entry.id() {
                Click::Settings
            } else if event.id == self.pause.id() {
                Click::PauseOrResume
            } else if event.id == self.quit.id() {
                Click::Quit
            } else {
                continue;
            };

            match self.duty.resolve(click) {
                TrayEffect::OpenSettings => self.show(ctx),
                TrayEffect::Pause => self.hook.pause(),
                TrayEffect::Resume => self.hook.resume(),
                TrayEffect::Quit => {
                    // The hook comes down before the process does, so the
                    // keyboard is normal by the time Footman stops existing.
                    self.hook.quit();
                    self.quitting = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    /// Records the next key pressed as the Chord of the row being captured.
    fn capture(&mut self, ctx: &egui::Context, now: f64) {
        let Some(row) = self.capturing else { return };

        let pressed = ctx.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => Some((*key, *modifiers)),
                _ => None,
            })
        });

        let Some((key, modifiers)) = pressed else {
            return;
        };

        // Esc cancels (DESIGN.md §10), which is also why Esc cannot be captured
        // as a Chord. Nothing else can cancel without leaving the row half-set.
        if key == egui::Key::Escape {
            self.capturing = None;
            return;
        }

        let Some(key) = key_named(key.name()) else {
            self.status
                .say(format!("Footman has no name for {}", key.name()), now);
            return;
        };

        let mut mods = Modifiers::NONE;
        if modifiers.shift {
            mods.insert(Modifiers::SHIFT);
        }
        if modifiers.ctrl {
            mods.insert(Modifiers::CTRL);
        }
        if modifiers.alt {
            mods.insert(Modifiers::ALT);
        }

        if let Some(binding) = self.settings.bindings.get_mut(row) {
            binding.0 = Chord::key(key).with(mods);
        }
        self.capturing = None;
        self.status.clear();
    }

    fn save(&mut self, now: f64) {
        let problems = self.settings.problems();
        if !problems.is_empty() {
            self.announce_problems = true;
            self.status
                .say(format!("{} thing(s) to fix first", problems.len()), now);
            return;
        }
        self.announce_problems = false;

        let config = self.settings.clone().into_config();
        let live = config.bindings.sorted().len();
        match config.save(&self.path) {
            Ok(()) => {
                // The hook keeps running: pressing Save should not make the
                // keyboard flicker. Nor should it need this window to go away
                // first — the Bindings are live from the next keystroke, which
                // is what the message says, because a user who has to close a
                // window to find out has been told nothing.
                self.hook
                    .rebind(Core::new(config.hyper, config.tap, config.bindings.clone()));
                self.status.say(
                    format!("Saved — {} + {live} binding(s) live now", config.hyper),
                    now,
                );
            }
            Err(error) => self.status.say(error.message, now),
        }
    }
}

/// Gives every interactive widget an edge.
///
/// egui's palette leaves an unhovered button almost exactly the colour of the
/// panel behind it, so a row of controls reads as a row of text and only admits
/// to being a control once the pointer is already on it. Nothing here is meant
/// to be discovered by sweeping the mouse across the window.
fn outline_widgets(ctx: &egui::Context) {
    let mut style = (*ctx.global_style()).clone();
    let widgets = &mut style.visuals.widgets;

    let edge = egui::Stroke::new(
        1.0,
        widgets.noninteractive.fg_stroke.color.gamma_multiply(0.4),
    );
    widgets.inactive.bg_stroke = edge;
    // The fill an unhovered widget gets is the weaker of the two by default.
    widgets.inactive.weak_bg_fill = widgets.inactive.bg_fill;

    ctx.set_global_style(style);
}

fn icon_of(duty: Duty) -> Result<Icon, String> {
    Icon::from_rgba(duty.icon(), 32, 32).map_err(|error| error.to_string())
}

impl eframe::App for Window {
    /// Runs whether or not the window is visible, which is what keeps the tray
    /// answering while only the icon is on screen.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_duties();
        self.drain_tray(ctx);
        self.capture(ctx, ctx.input(|input| input.time));

        if self.shown {
            // Asked for every frame it is visible rather than once: the window
            // is not there to be raised until the frame after it is shown.
            raise_named(TITLE);
            self.shown = false;
        }

        // Closing the window hides it. Footman is not the window (DESIGN.md
        // §10); leaving is what Quit is for.
        if ctx.input(|input| input.viewport().close_requested()) && !self.quitting {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.hide(ctx);
        }

        ctx.request_repaint_after(Duration::from_millis(100));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.tab, Tab::Bindings, "Bindings");
            ui.selectable_value(&mut self.tab, Tab::General, "General");
        });
        ui.separator();

        match self.tab {
            Tab::Bindings => self.bindings_tab(ui),
            Tab::General => self.general_tab(ui),
        }

        ui.separator();
        let now = ui.input(|input| input.time);
        ui.horizontal(|ui| {
            if ui.button("Save").clicked() {
                self.save(now);
            }
            ui.label(self.status.text(now));
        });

        if self.picking.is_some() {
            self.picker(ui.ctx());
        }
    }
}

impl Window {
    fn bindings_tab(&mut self, ui: &mut egui::Ui) {
        let problems = if self.announce_problems {
            self.settings.problems()
        } else {
            Vec::new()
        };

        if ui.button("Add Binding").clicked() {
            self.settings
                .add(Chord::key(Key::A), Action::App { id: String::new() });
            // Straight into capture: the first thing anyone wants to do with a
            // new row is say which key it is.
            self.capturing = Some(self.settings.bindings.len() - 1);
        }
        ui.add_space(4.0);

        let mut remove = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for row in 0..self.settings.bindings.len() {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        let capturing = self.capturing == Some(row);
                        let chord = if capturing {
                            "press a key…".to_string()
                        } else {
                            format!(
                                "{} + {}",
                                self.settings.hyper, self.settings.bindings[row].0
                            )
                        };
                        // A button rather than a selectable label: unselected,
                        // a label carries no frame at all and reads as text the
                        // user is not meant to touch. The one thing this cell
                        // has to say is that it can be clicked.
                        let cell = egui::Button::new(chord).min_size(egui::vec2(150.0, 0.0));
                        let cell = if capturing {
                            cell.fill(ui.visuals().selection.bg_fill)
                        } else {
                            cell
                        };
                        if ui.add(cell).clicked() {
                            self.capturing = Some(row);
                            self.status.clear();
                        }

                        self.action_editor(ui, row);

                        if ui.button("Remove").clicked() {
                            remove = Some(row);
                        }
                    });

                    for problem in problems.iter().filter(|problem| problem.row() == row) {
                        ui.colored_label(ui.visuals().warn_fg_color, problem.message());
                    }
                });
            }
        });

        if let Some(row) = remove {
            self.settings.remove(row);
            self.capturing = None;
            self.picking = None;
        }
    }

    fn action_editor(&mut self, ui: &mut egui::Ui, row: usize) {
        let action = &mut self.settings.bindings[row].1;

        egui::ComboBox::from_id_salt(("kind", row))
            .selected_text(kind_of(action))
            .width(80.0)
            .show_ui(ui, |ui| {
                for candidate in [
                    Action::App { id: String::new() },
                    Action::Open {
                        target: String::new(),
                    },
                    Action::Run {
                        command: String::new(),
                        show_window: false,
                    },
                    Action::Desktop { index: 1 },
                ] {
                    let name = kind_of(&candidate);
                    // Changing the kind clears the old fields rather than
                    // trying to carry them across: a command is not a URL.
                    if ui.selectable_label(kind_of(action) == name, name).clicked() {
                        *action = candidate;
                    }
                }
            });

        // Bounded rather than left to take what is available: in a row, a
        // field that asks for the rest of the width pushes everything after it
        // off the edge of the window — which is where the Choose and remove
        // buttons were going.
        const FIELD: f32 = 190.0;

        match action {
            Action::App { id } => {
                ui.add(egui::TextEdit::singleline(id).desired_width(FIELD));
                if ui.button("Choose…").clicked() {
                    self.picking = Some(row);
                    self.filter.clear();
                }
            }
            Action::Open { target } => {
                ui.add(egui::TextEdit::singleline(target).desired_width(FIELD));
            }
            Action::Run {
                command,
                show_window,
            } => {
                ui.add(egui::TextEdit::singleline(command).desired_width(FIELD));
                ui.checkbox(show_window, "show window");
            }
            Action::Desktop { index } => {
                ui.add(egui::DragValue::new(index).range(1..=32));
            }
        }
    }

    /// The list of installed applications (DESIGN.md §10).
    ///
    /// Not a palette: it exists only while a Binding is being created, and
    /// never appears on the hot path.
    fn picker(&mut self, ctx: &egui::Context) {
        let installed = self.installed.get_or_insert_with(applications);

        let mut chosen: Option<String> = None;
        let mut open = true;

        egui::Window::new("Choose an application")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([420.0, 420.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Filter");
                    ui.text_edit_singleline(&mut self.filter);
                });
                ui.separator();

                let needle = self.filter.to_lowercase();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    // Justified, so every row is a full-width button with its
                    // name against the left edge. As selectable labels they
                    // were never selected and so never framed: a column of
                    // plain text that only admitted to being a list of choices
                    // once the pointer was already on one of them.
                    let rows = egui::Layout::top_down_justified(egui::Align::LEFT);
                    ui.with_layout(rows, |ui| {
                        for app in installed
                            .iter()
                            .filter(|app| app.name.to_lowercase().contains(&needle))
                        {
                            if ui.button(&app.name).clicked() {
                                chosen = Some(app.identity.clone());
                            }
                        }
                    });
                });

                ui.separator();
                // The escape hatch §10 asks for: anything the shell's list does
                // not know about can still be named by hand.
                ui.label("Not listed? Type a path: identity into the field instead.");
            });

        if let (Some(identity), Some(row)) = (chosen, self.picking) {
            if let Some((_, Action::App { id })) = self.settings.bindings.get_mut(row) {
                *id = identity;
            }
            self.picking = None;
        } else if !open {
            self.picking = None;
        }
    }

    fn general_tab(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("general").num_columns(2).show(ui, |ui| {
            ui.label("Hyper Key");
            egui::ComboBox::from_id_salt("hyper")
                .selected_text(self.settings.hyper.to_string())
                .show_ui(ui, |ui| {
                    for &key in Key::ALL {
                        ui.selectable_value(&mut self.settings.hyper, key, key.to_string());
                    }
                });
            ui.end_row();

            ui.label("Tap");
            egui::ComboBox::from_id_salt("tap")
                .selected_text(self.settings.tap.to_string())
                .show_ui(ui, |ui| {
                    for tap in [TapAction::None, TapAction::Escape] {
                        ui.selectable_value(&mut self.settings.tap, tap, tap.to_string());
                    }
                });
            ui.end_row();

            ui.label("Start with Windows");
            ui.checkbox(&mut self.settings.autostart, "");
            ui.end_row();

            ui.label("Config file");
            ui.label(self.path.display().to_string());
            ui.end_row();
        });

        ui.add_space(8.0);
        ui.label("Autostart and Uninstall take effect in slice 8.");
    }
}

fn kind_of(action: &Action) -> &'static str {
    match action {
        Action::App { .. } => "app",
        Action::Open { .. } => "open",
        Action::Run { .. } => "run",
        Action::Desktop { .. } => "desktop",
    }
}
