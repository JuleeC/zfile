// for handling the terminal with user input
mod file_manipulation;

// for the help popup
use tui_popup::Popup;
// for input handling
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
// for standard input/output
use std::process::{Command, Stdio};
// for the ui components
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};
// for getting the data from the file system
use std::{fs, io, path::PathBuf};

// fuzzy matching
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

// different modes like in nvim
enum InputMode {
    Normal,
    Rename,
    Move,
    DeleteConfirm,
    Create,
}

fn main() -> Result<(), io::Error> {
    //------------------------------------------------------------------------------
    //
    //  INITIALIZATION
    //
    //------------------------------------------------------------------------------
    //entering an alternaate screen and enabling raw mode
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let mut terminal = Some(init_terminal()?);

    //current directory
    let mut current_directory: PathBuf = std::env::current_dir()?;
    //the directory from where the program starts
    let root_dir: PathBuf = current_directory.clone();
    //the currently selected file
    let mut selected_file: usize = 0;

    //the query string when searching through the files
    let mut query: String = String::new();
    //if the search bar is active
    let mut in_search: bool = false;
    //if the fuzzy search is active
    let mut fuzzy_mode: bool = false;

    //the current mode of the program
    let mut input_mode = InputMode::Normal;
    //the buffer for the input
    let mut input_buffer = String::new();
    //the buffer when creating a new file
    let mut create_buffer = String::new();

    //for fuzzy matching
    let matcher = SkimMatcherV2::default();

    //the main loop that recursively runs until user presses 'q'
    loop {
        //get the entries from the current directory but unfiltered
        let mut entries_raw = get_entries(&current_directory);

        //filter the entries based on the query
        let entries: Vec<PathBuf> = if query.is_empty() {
            entries_raw
        } else if fuzzy_mode {
            let q = query.clone();
            let mut scored: Vec<(PathBuf, i64)> = entries_raw
                .drain(..)
                .filter_map(|p| {
                    let name = p
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    matcher.fuzzy_match(&name, &q).map(|score| (p, score))
                })
                .collect();
            scored.sort_by(|a, b| b.1.cmp(&a.1));
            scored.into_iter().map(|(p, _)| p).collect()
        } else {
            let q = query.to_lowercase();
            entries_raw
                .into_iter()
                .filter(|p| {
                    p.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_lowercase()
                        .contains(&q)
                })
                .collect()
        };
        //------------------------------------------------------------------------------

        //check if the list is empty
        if entries.is_empty() {
            //i think its unnecessary to set the selected file to 0 because it will be set to the last entry and that is 0
            selected_file = 0;
        } else if selected_file >= entries.len() {
            selected_file = entries.len().saturating_sub(1);
        }
        //------------------------------------------------------------------------------
        //
        //  DRAWING
        //
        //------------------------------------------------------------------------------

        if let Some(ref mut term) = terminal {
            //draw the ui
            term.draw(|f| {
                //split the screen into two columns
                let layout = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                    //f is the frame
                    .split(f.area());

                //split the first column into two rows
                let nav_column = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3), // search bar
                        Constraint::Min(0),    // file list
                        Constraint::Length(3), // mode display
                    ])
                    .split(layout[0]);
                //split the mode column for a helper hover_button

                //

                //the title of the search bar
                let title = if fuzzy_mode {
                    "Search (Fuzzy): type to filter, Esc to exit"
                    //if the search bar is active and the fuzzy search is unactive
                } else if in_search {
                    "Search: type to filter, Esc to exit"
                    //if the search bar is unactive
                } else {
                    "Search (press 'f' for fuzzy, 's' for normal)"
                };

                //render the search bar
                let search_paragraph = Paragraph::new(query.as_str())
                    .block(Block::default().title(title).borders(Borders::ALL));
                f.render_widget(search_paragraph, nav_column[0]);
                //declare the items for the list
                let items: Vec<ListItem> = entries
                    .iter()
                    .map(|entry| {
                        let name = entry.file_name().unwrap_or_default().to_string_lossy();
                        let display_name = if entry.is_dir() {
                            format!("{}/", name)
                        } else {
                            name.to_string()
                        };
                        ListItem::new(display_name)
                    })
                    .collect();

                //declaring a "frame" for the list where it can be rendered
                let ui_list = List::new(items)
                    .block(Block::default().title("Files").borders(Borders::ALL))
                    .highlight_style(Style::default().fg(Color::Cyan));

                let mut list_state = ratatui::widgets::ListState::default();
                if !entries.is_empty() {
                    list_state.select(Some(selected_file));
                }
                //render the list
                f.render_stateful_widget(ui_list, nav_column[1], &mut list_state);
                // footer text + color per mode
                let (footer_text, footer_style) = match input_mode {
                    InputMode::Normal => {
                        ("NORMAL MODE".to_string(), Style::default().fg(Color::White))
                    }
                    InputMode::Rename => (
                        format!("RENAME: {}", input_buffer),
                        Style::default().fg(Color::LightBlue),
                    ),
                    InputMode::Move => (
                        format!("MOVE: {}", input_buffer),
                        Style::default().fg(Color::Magenta),
                    ),
                    InputMode::DeleteConfirm => {
                        ("DELETE? (y/n)".to_string(), Style::default().fg(Color::Red))
                    }
                    InputMode::Create => (
                        format!("CREATE: {}", create_buffer),
                        Style::default().fg(Color::Green),
                    ),
                };

                let mode_footer_layout = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(90), Constraint::Percentage(10)])
                    .split(nav_column[2]);

                //where the mode and the hotkey helper get rendered
                let footer = ratatui::widgets::Paragraph::new(footer_text)
                    .style(footer_style)
                    .block(Block::default().borders(Borders::ALL));
                f.render_widget(footer, mode_footer_layout[0]);

                // Render the question mark "?" in the 10% column
                let question_mark = ratatui::widgets::Paragraph::new("PRESS ?")
                    .style(Style::default().fg(Color::Yellow))
                    .block(Block::default().borders(Borders::ALL));
                f.render_widget(question_mark, mode_footer_layout[1]);
                //open the file for the preview
                let preview_content = if let Some(entry) = entries.get(selected_file) {
                    if entry.is_file() {
                        fs::read_to_string(entry)
                            .unwrap_or_else(|_| "[Could not read file]".to_string())
                    } else {
                        "".to_string()
                    }
                } else {
                    "".to_string()
                };

                //declaring a "frame" for the preview where it can be rendered
                let preview = Paragraph::new(preview_content)
                    .block(Block::default().title("Preview").borders(Borders::ALL))
                    .wrap(Wrap { trim: true });

                //render the preview
                f.render_widget(preview, layout[1]);
            })?;

            //------------------------------------------------------------------------------
            //
            //  EVENT HANDLING
            //
            //------------------------------------------------------------------------------
            //all 100ms
            if event::poll(std::time::Duration::from_millis(100))? {
                //when a event is received(key pressed)
                if let Event::Key(key) = event::read()? {
                    //swtich on the current mode
                    match input_mode {
                        InputMode::Normal => match key.code {
                            KeyCode::Esc => {
                                in_search = false;
                                query.clear();
                            }
                            // For help preview
                            KeyCode::Char('?') => {
                                println!("showing help preview");
                            }
                            KeyCode::Backspace if in_search => {
                                query.pop();
                            }
                            KeyCode::Enter if in_search => {
                                if let Some(entry) = entries.get(selected_file) {
                                    if entry.is_dir() {
                                        current_directory = entry.clone();
                                        selected_file = 0;
                                    } else if entry.is_file() {
                                        if file_helper(entry).is_ok() {
                                            terminal = Some(init_terminal()?);
                                            current_directory = entry
                                                .parent()
                                                .map(PathBuf::from)
                                                .unwrap_or(current_directory.clone());
                                            selected_file = 0;
                                        }
                                    }
                                }
                                query.clear();
                                in_search = false;
                            }
                            KeyCode::Char(c) if in_search => {
                                query.push(c);
                            }
                            KeyCode::Char('q') => break,
                            KeyCode::Char('f') if !in_search => {
                                in_search = true;
                                fuzzy_mode = true;
                            }
                            KeyCode::Char('s') if !in_search => {
                                in_search = true;
                                fuzzy_mode = false;
                            }
                            KeyCode::Char('r') if !entries.is_empty() => {
                                input_mode = InputMode::Rename;
                                input_buffer.clear();
                            }
                            KeyCode::Char('m') if !entries.is_empty() => {
                                input_mode = InputMode::Move;
                                input_buffer.clear();
                            }
                            KeyCode::Char('d') if !entries.is_empty() => {
                                input_mode = InputMode::DeleteConfirm;
                            }
                            KeyCode::Char('c') if !entries.is_empty() => {
                                input_mode = InputMode::Create;
                            }
                            KeyCode::Char('j') => {
                                if !entries.is_empty()
                                    && selected_file < entries.len().saturating_sub(1)
                                {
                                    selected_file += 1;
                                }
                            }
                            KeyCode::Char('k') => {
                                if selected_file > 0 {
                                    selected_file -= 1;
                                }
                            }
                            KeyCode::Char('J') => {
                                selected_file = entries.len().saturating_sub(1);
                            }
                            KeyCode::Char('K') => {
                                selected_file = 0;
                            }
                            KeyCode::Char('H') => {
                                current_directory = root_dir.clone();
                                selected_file = 0;
                            }
                            KeyCode::Char('h') => {
                                current_directory.pop();
                                selected_file = 0;
                            }
                            KeyCode::Char('l') => {
                                if let Some(entry) = entries.get(selected_file) {
                                    if entry.is_dir() {
                                        current_directory = entry.clone();
                                        selected_file = 0;
                                    } else if entry.is_file() {
                                        if file_helper(entry).is_ok() {
                                            terminal = Some(init_terminal()?);
                                            current_directory = entry
                                                .parent()
                                                .map(PathBuf::from)
                                                .unwrap_or(current_directory.clone());
                                            selected_file = 0;
                                        }
                                    }
                                }
                            }
                            _ => {}
                        },
                        InputMode::Rename | InputMode::Move => match key.code {
                            KeyCode::Esc => {
                                input_mode = InputMode::Normal;
                                input_buffer.clear();
                            }
                            KeyCode::Backspace => {
                                input_buffer.pop();
                            }
                            KeyCode::Char(c) => {
                                input_buffer.push(c);
                            }
                            KeyCode::Enter => {
                                if let Some(entry) = entries.get(selected_file) {
                                    match input_mode {
                                        InputMode::Rename => {
                                            file_manipulation::rename_file(entry, &input_buffer)
                                                .ok();
                                        }
                                        InputMode::Move => {
                                            file_manipulation::move_file(entry, &input_buffer).ok();
                                        }
                                        _ => {}
                                    }
                                }
                                input_mode = InputMode::Normal;
                                input_buffer.clear();
                            }
                            _ => {}
                        },
                        InputMode::Create => match key.code {
                            KeyCode::Esc => {
                                input_mode = InputMode::Normal;
                                create_buffer.clear();
                            }
                            KeyCode::Backspace => {
                                create_buffer.pop();
                            }
                            KeyCode::Char(c) => {
                                create_buffer.push(c);
                            }
                            KeyCode::Enter => {
                                if let Some(entry) = entries.get(selected_file) {
                                    println!("{:?}", entry);
                                    file_manipulation::create_file(
                                        &current_directory,
                                        &create_buffer,
                                    )
                                    .ok();
                                }
                                input_mode = InputMode::Normal;
                                create_buffer.clear();
                            }
                            _ => {}
                        },
                        InputMode::DeleteConfirm => match key.code {
                            KeyCode::Char('y') => {
                                if let Some(entry) = entries.get(selected_file) {
                                    file_manipulation::delete_file(entry).ok();
                                }
                                input_mode = InputMode::Normal;
                            }
                            KeyCode::Char('n') | KeyCode::Esc => {
                                input_mode = InputMode::Normal;
                            }
                            _ => {}
                        },
                    }
                }
            }
        }
    }

    //------------------------------------------------------------------------------
    //
    //  CLEANUP
    //
    //  disable raw mode and exit the program
    //
    // -------------------------------------------------------------------------------
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    std::process::exit(0);
}

//get the entries from the directory
fn get_entries(path: &PathBuf) -> Vec<PathBuf> {
    fs::read_dir(path)
        .unwrap_or_else(|_| fs::read_dir(".").unwrap())
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect()
}

//open the file in nvim
#[allow(unused)]
fn file_helper(path: &PathBuf) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;

    Command::new("nvim")
        .arg(path)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;
    Ok(())
}

//initialize the terminal
fn init_terminal() -> io::Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}
//
//TODO:add a hotkey helper,
//PLAN: render a question mark where the fuzzy finder is located and when hovering there should a
//popup appear
