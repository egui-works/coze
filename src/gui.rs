use eframe::egui::*;
use serde::{Deserialize, Serialize};

use crate::generator::{Generator, GeneratorMode, Message, PromptId};

const TEXT_FONT: FontId = FontId::new(15.0, FontFamily::Monospace);
const ROUNDING: f32 = 8.0;

mod config;
mod error;
mod help;

#[derive(Debug)]
pub struct App {
    prompt: String,
    prompt_field_id: Id,
    last_prompt_id: PromptId,
    state: PersistedState,
    generator: Generator,
    error: Option<String>,
    show_config: bool,
    show_help: bool,
    matcher: HistoryNavigator,
    ctx: Context,
    frame_counter: usize,
}

#[derive(Clone, Copy, Deserialize, Serialize, Debug, Default, PartialEq)]
enum UiMode {
    #[default]
    Light,
    Dark,
}

impl UiMode {
    fn visuals(&self) -> Visuals {
        match self {
            UiMode::Light => Visuals::light(),
            UiMode::Dark => Visuals::dark(),
        }
    }

    fn description(&self) -> &'static str {
        match self {
            UiMode::Light => "Light",
            UiMode::Dark => "Dark",
        }
    }

    fn fill_color(&self) -> Color32 {
        match &self {
            UiMode::Light => Color32::from_gray(230),
            UiMode::Dark => Color32::from_gray(50),
        }
    }
}

/// State persisted by egui.
#[derive(Deserialize, Serialize, Debug, Default)]
struct PersistedState {
    history: Vec<Prompt>,
    generator_mode: GeneratorMode,
    ui_mode: UiMode,
}

#[derive(Deserialize, Serialize, Debug)]
struct Prompt {
    prompt: String,
    reply: String,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let state: PersistedState = if let Some(storage) = cc.storage {
            // Load previous app state (if any).
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        };

        cc.egui_ctx.set_visuals(state.ui_mode.visuals());

        let generator = Generator::new(state.generator_mode);

        Self {
            prompt_field_id: Id::new("prompt-id"),
            last_prompt_id: PromptId::default(),
            prompt: Default::default(),
            state,
            generator,
            error: None,
            show_config: false,
            show_help: false,
            matcher: HistoryNavigator::new(),
            ctx: cc.egui_ctx.clone(),
            frame_counter: 0,
        }
    }

    fn send_prompt(&mut self) {
        let prompt = self.prompt.trim();
        if !prompt.is_empty() {
            // Flush tokens from previous prompt
            while self.generator.next_message().is_some() {}

            self.last_prompt_id = self.generator.send_prompt(prompt);
            self.state.history.push(Prompt {
                prompt: prompt.to_owned(),
                reply: Default::default(),
            });
        }

        self.reset_prompt("".to_string());
        self.matcher.reset(&self.prompt);
    }

    fn reset_prompt(&mut self, prompt: String) {
        self.prompt = prompt;

        let state = text_edit::TextEditState::default();
        state.store(&self.ctx, self.prompt_field_id);
    }

    fn process_input(&mut self) {
        // Stops tokens generation for the current prompt.
        if self
            .ctx
            .input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
        {
            self.generator.stop();
            self.reset_prompt("".to_string());
            self.matcher.reset(&self.prompt);
        }

        // Manage history
        if self
            .ctx
            .input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp))
        {
            if let Some(prompt) = self.matcher.up(&self.state.history) {
                self.reset_prompt(prompt);
            }
        }

        if self
            .ctx
            .input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown))
        {
            if let Some(prompt) = self.matcher.down(&self.state.history) {
                self.reset_prompt(prompt);
            }
        }
    }
}

impl eframe::App for App {
    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.state);
    }

    /// Handle input and repaint screen.
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.frame_counter += 1;
        let mut scroll_to_bottom = false;

        ctx.send_viewport_cmd(ViewportCommand::Title(format!(
            "Coze ({})",
            self.generator.mode().description()
        )));

        match self.generator.next_message() {
            Some(Message::Token(prompt_id, s)) => {
                // Skip tokens from a previous prompt.
                if self.last_prompt_id == prompt_id {
                    if let Some(prompt) = self.state.history.last_mut() {
                        prompt.reply.push_str(&s);
                        scroll_to_bottom = true;
                    }
                }
            }
            Some(Message::Error(msg)) => {
                self.error = Some(msg);
            }
            None => (),
        };

        self.process_input();

        // Render menu
        TopBottomPanel::top("top_panel").show(ctx, |ui| {
            menu::bar(ui, |ui| {
                ui.menu_button("Edit", |ui| {
                    if ui.button("Config").clicked() {
                        self.show_config = true;
                        ui.close_menu();
                    }

                    if ui.button("Clear history").clicked() {
                        self.state.history.clear();
                        ui.close_menu();
                    }
                });

                if ui.button("Help").clicked() {
                    self.show_help = true;
                    ui.close_menu();
                }
            });
        });

        let prompt_frame = Frame::none()
            .fill(ctx.style().visuals.window_fill)
            .outer_margin(Margin::same(0.0))
            .inner_margin(Margin::same(10.0));

        // Render prompt panel.
        TopBottomPanel::bottom("bottom_panel")
            .show_separator_line(false)
            .frame(prompt_frame)
            .show(ctx, |ui| {
                Frame::group(ui.style())
                    .rounding(Rounding::same(ROUNDING))
                    .fill(self.state.ui_mode.fill_color())
                    .show(ui, |ui| {
                        ctx.memory_mut(|m| m.request_focus(self.prompt_field_id));

                        // Override multiline Enter behavior
                        if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
                            self.send_prompt();
                            scroll_to_bottom = true;
                        }

                        let text = TextEdit::multiline(&mut self.prompt)
                            .id(self.prompt_field_id)
                            .cursor_at_end(true)
                            .font(TEXT_FONT)
                            .frame(false)
                            .margin(Vec2::new(5.0, 5.0))
                            .desired_rows(1)
                            .hint_text("Prompt me! (Enter to send)");

                        let r = ui.add_sized([ui.available_width(), 10.0], text);
                        if r.changed() {
                            self.matcher.reset(&self.prompt);
                        }
                    })
            });

        // Render message panel.
        CentralPanel::default().show(ctx, |ui| {
            ScrollArea::vertical()
                .auto_shrink(false)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for prompt in &self.state.history {
                        let r = ui.add(Bubble::new(
                            &prompt.prompt,
                            BubbleContent::Prompt,
                            self.state.ui_mode,
                        ));
                        if r.clicked() {
                            ui.ctx().copy_text(prompt.prompt.clone());
                        }

                        if r.double_clicked() {
                            self.prompt = prompt.prompt.clone();
                            scroll_to_bottom = true;
                        }

                        ui.add_space(ui.spacing().item_spacing.y);

                        if !prompt.reply.is_empty() {
                            let r = ui.add(Bubble::new(
                                &prompt.reply,
                                BubbleContent::Reply,
                                self.state.ui_mode,
                            ));
                            if r.clicked() {
                                ui.ctx().copy_text(prompt.reply.clone());
                            }

                            ui.add_space(ui.spacing().item_spacing.y * 2.5);
                        } else {
                            let dots = ["⏺   ", " ⏺  ", "  ⏺ ", "   ⏺", "  ⏺ ", " ⏺  "];
                            ui.add(Bubble::new(
                                dots[(self.frame_counter / 18) % dots.len()],
                                BubbleContent::Reply,
                                self.state.ui_mode,
                            ));
                            ui.add_space(ui.spacing().item_spacing.y * 2.5);
                        }
                    }

                    if scroll_to_bottom {
                        ui.scroll_to_cursor(Some(Align::BOTTOM));
                    }
                });
            ui.allocate_space(ui.available_size());
        });

        self.config_window();
        self.error_window();
        self.help_window();

        // Run 20 frames per second.
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.generator.shutdown();
    }
}

enum BubbleContent {
    Prompt,
    Reply,
}

struct Bubble {
    text: WidgetText,
    content: BubbleContent,
    ui_mode: UiMode,
}

impl Bubble {
    fn new(text: &str, content: BubbleContent, ui_mode: UiMode) -> Self {
        let text = WidgetText::from(RichText::new(text).font(TEXT_FONT).monospace());
        Self {
            text,
            content,
            ui_mode,
        }
    }

    fn fill_color(content: &BubbleContent, ui_mode: UiMode) -> Color32 {
        match content {
            BubbleContent::Prompt => Color32::from_rgb(15, 85, 235),
            BubbleContent::Reply => ui_mode.fill_color(),
        }
    }

    fn text_color(content: &BubbleContent, ui_mode: UiMode) -> Color32 {
        match content {
            BubbleContent::Prompt => Color32::from_rgb(210, 225, 250),
            BubbleContent::Reply => match ui_mode {
                UiMode::Light => Color32::from_gray(60),
                UiMode::Dark => Color32::from_gray(180),
            },
        }
    }
}

impl Widget for Bubble {
    fn ui(self, ui: &mut Ui) -> Response {
        const PADDING: f32 = 10.0;
        const WIDTH_PCT: f32 = 0.80;

        let Bubble {
            text,
            content,
            ui_mode,
        } = self;

        let text_wrap_width = ui.available_width() * WIDTH_PCT - 2.0 * PADDING;
        let galley = text.into_galley(ui, Some(true), text_wrap_width, TextStyle::Monospace);
        let bubble_size = galley.size() + Vec2::splat(2.0 * PADDING);

        let desired_size = Vec2::new(ui.available_width(), bubble_size.y);
        let (rect, response) = ui.allocate_at_least(desired_size, Sense::click());

        let dx = ui.available_width() - bubble_size.x;
        let paint_rect = if matches!(content, BubbleContent::Prompt) {
            // Move prompt to the right
            Rect::from_min_max(Pos2::new(rect.min.x + dx, rect.min.y), rect.max)
        } else {
            Rect::from_min_max(rect.min, Pos2::new(rect.max.x - dx, rect.max.y))
        };

        if ui.is_rect_visible(rect) {
            let fill_color = Self::fill_color(&content, ui_mode);
            let text_color = Self::text_color(&content, ui_mode);

            // On click expand animation.
            let expand = ui
                .ctx()
                .animate_value_with_time(response.id, std::f32::consts::PI, 0.5)
                .sin()
                * ui.spacing().item_spacing.y;
            let paint_rect = paint_rect.expand(expand);

            if response.clicked() {
                ui.ctx().clear_animations();
                ui.ctx().animate_value_with_time(response.id, 0.0, 0.5);
            }

            ui.painter().rect(
                paint_rect,
                Rounding::same(ROUNDING),
                fill_color,
                Stroke::default(),
            );

            let text_pos = ui
                .layout()
                .align_size_within_rect(
                    galley.size(),
                    paint_rect.shrink2(Vec2::splat(PADDING + expand)),
                )
                .min;

            ui.painter().galley(text_pos, galley, text_color);
        }

        response
    }
}

#[derive(Debug)]
struct HistoryNavigator {
    pattern: String,
    cursor: usize,
}

impl HistoryNavigator {
    fn new() -> Self {
        Self {
            pattern: Default::default(),
            cursor: usize::MAX,
        }
    }

    fn reset(&mut self, pattern: &str) {
        self.pattern = pattern.to_lowercase();
        self.cursor = usize::MAX;
    }

    fn up(&mut self, history: &[Prompt]) -> Option<String> {
        if history.is_empty() {
            return None;
        }

        let mut cursor = self.cursor.min(history.len());

        loop {
            cursor = cursor.saturating_sub(1);
            if let Some(prompt) = history.get(cursor) {
                if self.is_match(history, &prompt.prompt) {
                    self.cursor = cursor;
                    return Some(prompt.prompt.clone());
                }
            }

            if cursor == 0 {
                return None;
            }
        }
    }

    fn down(&mut self, history: &[Prompt]) -> Option<String> {
        if history.is_empty() {
            return None;
        }

        let mut cursor = self.cursor.min(history.len() - 1);

        loop {
            cursor = cursor.saturating_add(1);
            if let Some(prompt) = history.get(cursor) {
                if self.is_match(history, &prompt.prompt) {
                    self.cursor = cursor;
                    return Some(prompt.prompt.clone());
                }
            } else {
                return None;
            }
        }
    }

    fn is_match(&self, history: &[Prompt], text: &str) -> bool {
        // Skip repeated prompts.
        let match_current = history
            .get(self.cursor)
            .map(|p| text.eq_ignore_ascii_case(&p.prompt))
            .unwrap_or_default();

        if match_current {
            return false;
        }

        let mut pit = self.pattern.chars().peekable();

        for c in text.chars() {
            if let Some(p) = pit.peek() {
                if p.eq_ignore_ascii_case(&c) {
                    pit.next();
                }
            } else {
                break;
            }
        }

        pit.peek().is_none()
    }
}
