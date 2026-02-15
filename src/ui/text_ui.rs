use crate::analysis_result::AnalysisResult;
use crate::data::{Data, Kind};
use crate::settings::Settings;
use crate::task::Task;
use crate::ui::app_state::analyzer::Message;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use humansize::DECIMAL;
use ratatui::{
    Terminal,
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect as RatatuiRect},
    style::{Color, Style},
    widgets::{Block, Borders, TableState},
};
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use treemap::{Mappable, Rect, TreemapLayout};

#[derive(Debug)]
pub(crate) struct TextUi {
    analysis_result: AnalysisResult,
    table_state: TableState,
    scanned_directories: u64,
    file_count: u64,
    total_size: u64,
    current_scanning_path: String,
}

impl TextUi {
    pub(crate) fn run(path: PathBuf) -> io::Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let app = TextUi::new(path);
        let res = app.run_loop(&mut terminal);

        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        if let Err(err) = res {
            println!("{:?}", err);
        }

        Ok(())
    }

    fn new(path: PathBuf) -> Self {
        let root_data = Data::new_directory(&path);
        Self {
            analysis_result: AnalysisResult::new(path, vec![root_data]),
            table_state: TableState::default(),
            scanned_directories: 0,
            file_count: 0,
            total_size: 0,
            current_scanning_path: String::new(),
        }
    }

    fn run_loop<B: Backend>(mut self, terminal: &mut Terminal<B>) -> io::Result<()> {
        let settings = Arc::new(Mutex::new(Settings::default()));
        let stopper = Arc::new(AtomicBool::new(false));
        let (tx, rx) = std::sync::mpsc::channel();

        let settings_clone = Arc::clone(&settings);
        let stopper_clone = Arc::clone(&stopper);
        let path_clone = self.analysis_result.root_path.clone();

        std::thread::spawn(move || {
            Task::scan_directory_channel(&path_clone, &tx, &stopper_clone, settings_clone);
        });

        let mut last_tick = Instant::now();
        let tick_rate = Duration::from_millis(100);

        loop {
            terminal
                .draw(|f| self.ui(f))
                .map_err(|e| io::Error::other(e.to_string()))?;

            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));

            if event::poll(timeout)?
                && let Event::Key(key) = event::read()?
            {
                match key.code {
                    KeyCode::Char('q') => {
                        stopper.store(true, Ordering::Relaxed);
                        return Ok(());
                    }
                    KeyCode::Down | KeyCode::Right => self.next(),
                    KeyCode::Up | KeyCode::Left => self.previous(),
                    KeyCode::Enter => {
                        if let Some(index) = self.table_state.selected() {
                            self.zoom_in(index);
                        }
                    }
                    KeyCode::Backspace | KeyCode::Esc => self.zoom_out(),
                    _ => {}
                }
            }

            // Receive data from scan thread
            for message in rx.try_iter() {
                match message {
                    Message::Data(data) => {
                        if data.size() > 0.0 {
                            self.analysis_result
                                .data_stack
                                .last_mut()
                                .unwrap()
                                .push(data);
                        }
                    }
                    Message::DirectoryScanStart(d) => {
                        self.current_scanning_path = d;
                        self.scanned_directories += 1;
                    }
                    Message::DirectoryScanDone(res) => {
                        self.file_count += res.file_count;
                        self.total_size += res.size;
                    }
                }
            }

            if last_tick.elapsed() >= tick_rate {
                last_tick = Instant::now();
                // Check if scan thread finished - we don't have the handle here but we can check if rx is closed
                // and no more data. Actually Message doesn't have a "Finished" variant but Task::scan_directory_channel
                // finishes when done.
                // For simplicity, let's just keep scanning true until we decide otherwise or user quits.
                // In the GUI, handle.is_finished() is used.
            }
        }
    }

    fn next(&mut self) {
        let i = match self.table_state.selected() {
            Some(i) => {
                let count = self.get_children_count();
                if count == 0 || i >= count - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    fn previous(&mut self) {
        let i = match self.table_state.selected() {
            Some(i) => {
                let count = self.get_children_count();
                if count == 0 {
                    0
                } else if i == 0 {
                    count.saturating_sub(1)
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    fn get_children_count(&self) -> usize {
        if let Some(root) = self.analysis_result.data_stack.last()
            && let Kind::Dir(children) = &root.kind
        {
            return children.len();
        }
        0
    }

    fn zoom_in(&mut self, index: usize) {
        if let Some(parent_node) = self.analysis_result.data_stack.last_mut()
            && let Kind::Dir(children) = &mut parent_node.kind
            && index < children.len()
            && matches!(children[index].kind, Kind::Dir(_))
        {
            let taken_data = children.swap_remove(index);
            self.analysis_result.data_stack.push(taken_data);
            self.table_state.select(Some(0));
        }
    }

    fn zoom_out(&mut self) {
        if self.analysis_result.data_stack.len() >= 2 {
            let index = self.analysis_result.data_stack.len() - 2;
            self.analysis_result.selected_index(index);
            self.table_state.select(Some(0));
        }
    }

    fn ui(&mut self, f: &mut ratatui::Frame) {
        // Initialize selection if not set
        if self.table_state.selected().is_none() && self.get_children_count() > 0 {
            self.table_state.select(Some(0));
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints(
                [
                    Constraint::Length(3),
                    Constraint::Min(0),
                    Constraint::Length(3),
                ]
                .as_ref(),
            )
            .split(f.area());

        let root_path = self.analysis_result.root_path.to_string_lossy();
        let title = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Disk Mosaic - {} ", root_path));

        let root = self.analysis_result.data_stack.last().unwrap();
        let total_size = if let Kind::Dir(children) = &root.kind {
            children.iter().map(|c| c.size).sum::<u64>()
        } else {
            root.size
        };

        let info = format!(
            "Scanned Dirs: {} | Files: {} | Total Size: {}",
            self.scanned_directories,
            self.file_count,
            humansize::format_size(total_size, DECIMAL)
        );
        let header_text = ratatui::widgets::Paragraph::new(info)
            .block(title)
            .style(Style::default().fg(Color::Cyan));
        f.render_widget(header_text, chunks[0]);

        self.render_treemap(f, chunks[1]);

        // Footer
        let footer_text = if self.analysis_result.data_stack.len() > 1 {
            format!(
                "Scanning: {} | Arrows: Navigate | Enter: Open | Backspace: Back | 'q': Quit",
                self.current_scanning_path
            )
        } else {
            format!(
                "Scanning: {} | Arrows: Navigate | Enter: Open | 'q': Quit",
                self.current_scanning_path
            )
        };
        let footer = ratatui::widgets::Paragraph::new(footer_text)
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(footer, chunks[2]);
    }

    fn render_treemap(&mut self, f: &mut ratatui::Frame, area: RatatuiRect) {
        let root = self.analysis_result.data_stack.last_mut().unwrap();
        if let Kind::Dir(children) = &mut root.kind {
            if children.is_empty() {
                let empty_block = Block::default().borders(Borders::ALL).title(" Results ");
                f.render_widget(empty_block, area);
                return;
            }

            let rect = Rect::from_points(
                area.x as f64,
                area.y as f64,
                area.width as f64,
                area.height as f64,
            );
            TreemapLayout::new().layout_items(children, rect);

            for (index, child) in children.iter().enumerate() {
                if child.bounds.w < 1.0 || child.bounds.h < 1.0 {
                    continue;
                }

                let child_area = RatatuiRect::new(
                    child.bounds.x.round() as u16,
                    child.bounds.y.round() as u16,
                    child.bounds.w.round() as u16,
                    child.bounds.h.round() as u16,
                );

                // Ensure it doesn't exceed the parent area due to rounding
                let child_area = area.intersection(child_area);

                let is_selected = self.table_state.selected() == Some(index);

                if child_area.width < 3 || child_area.height < 2 {
                    // Too small to draw a box with text
                    // We could fill it with something or just skip
                    let mut style = Style::default().bg(Color::Rgb(
                        child.color.r(),
                        child.color.g(),
                        child.color.b(),
                    ));
                    if is_selected {
                        style = style
                            .fg(Color::White)
                            .add_modifier(ratatui::style::Modifier::REVERSED);
                    }
                    f.render_widget(Block::default().style(style), child_area);
                    continue;
                }

                let mut border_style = Style::default().fg(Color::Rgb(
                    child.color.r(),
                    child.color.g(),
                    child.color.b(),
                ));

                if is_selected {
                    border_style = border_style
                        .add_modifier(ratatui::style::Modifier::BOLD)
                        .fg(Color::White);
                }

                let block = Block::default()
                    .borders(Borders::ALL)
                    .border_style(border_style)
                    .title(child.name.as_str());

                let content = humansize::format_size(child.size, DECIMAL);
                let mut style = Style::default().fg(Color::White);
                if is_selected {
                    style = style.add_modifier(ratatui::style::Modifier::BOLD);
                }
                let paragraph = ratatui::widgets::Paragraph::new(content)
                    .block(block)
                    .style(style);

                f.render_widget(paragraph, child_area);
            }
        }
    }
}
