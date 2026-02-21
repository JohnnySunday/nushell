//! The explore regex command implementation.

// Borrowed from the ut project and tweaked. Thanks!
// https://github.com/ksdme/ut
// Below is the ut license:
// MIT License
//
// Copyright (c) 2025 Kilari Teja
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use crate::explore_regex::app::App;
use crate::explore_regex::ui::run_app_loop;
use nu_engine::command_prelude::*;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    crossterm::{
        cursor::{SetCursorStyle, Show},
        event::{DisableMouseCapture, EnableMouseCapture},
        execute,
        terminal::{
            Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
            enable_raw_mode,
        },
    },
};

use ratatui::{TerminalOptions, Viewport};

// use ratatui::{Terminal::with_options, layout::Rect};

use std::io;

#[derive(Clone, Copy)]
pub struct Config {
    inline: Option<i64>,
}

impl Config {
    pub fn new(inline: Option<i64>) -> Self {
        Self { inline }
    }
}
/// A `regular expression explorer` program.
#[derive(Clone)]
pub struct ExploreRegex;

impl Command for ExploreRegex {
    fn name(&self) -> &str {
        "explore regex"
    }

    fn description(&self) -> &str {
        "Launch a TUI to create and explore regular expressions interactively."
    }

    fn signature(&self) -> nu_protocol::Signature {
        Signature::build("explore regex")
            .input_output_types(vec![
                (Type::Nothing, Type::String),
                (Type::String, Type::String),
            ])
            .named(
                // Add option to open explore regex inline with a fixed height
                "inline",
                SyntaxShape::Int,
                "Display the pager inline with a fixed height",
                Some('I'),
            )
            .category(Category::Viewers)
    }

    fn extra_description(&self) -> &str {
        r#"Press `Ctrl-Q` to quit and provide constructed regular expression as the output.
Supports AltGr key combinations for international keyboard layouts."#
    }

    fn run(
        &self,
        engine_state: &EngineState,
        stack: &mut Stack,
        call: &Call,
        input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        let input_span = input.span().unwrap_or(call.head);
        let (string_input, _span, _metadata) = input.collect_string_strict(input_span)?;
        let inline_height: Option<i64> = call.get_flag(engine_state, stack, "inline")?;
        let config = Config::new(inline_height);
        let regex = execute_regex_app(call.head, string_input, config)?;

        Ok(PipelineData::Value(
            nu_protocol::Value::string(regex, call.head),
            None,
        ))
    }

    fn examples(&self) -> Vec<Example<'_>> {
        vec![
            Example {
                description: "Explore a regular expression interactively",
                example: r#"explore regex"#,
                result: None,
            },
            Example {
                description: "Explore a regular expression interactively with sample text",
                example: r#"open -r Cargo.toml | explore regex"#,
                result: None,
            },
            Example {
                description: "Explore a regular expression interactively with sample text in
terminal instead of opening alternatescreen",
                example: r#"open -r Cargo.toml | explore regex"#,
                result: None,
            },
        ]
    }
}

/// Converts a terminal/IO error into a ShellError with consistent formatting.
fn terminal_error(error: &str, cause: impl std::fmt::Display, span: Span) -> ShellError {
    ShellError::GenericError {
        error: error.into(),
        msg: format!("terminal error: {cause}"),
        span: Some(span),
        help: None,
        inner: vec![],
    }
}

fn execute_regex_app(
    span: Span,
    string_input: String,
    config: Config,
) -> Result<String, ShellError> {
    let mut terminal = setup_terminal(span, config)?;
    let mut app = App::new(string_input);

    let result = run_app_loop(&mut terminal, &mut app);

    // Always attempt to restore terminal, even if app loop failed
    let restore_result = restore_terminal(&mut terminal, span, config);

    // Propagate app loop error first, then restore error
    result.map_err(|e| terminal_error("Application error", e, span))?;
    restore_result?;

    Ok(app.get_regex_input())
}

fn setup_terminal(
    span: Span,
    config: Config,
) -> Result<Terminal<CrosstermBackend<io::Stdout>>, ShellError> {
    let mut stdout = io::stdout();

    // Conditionally set up the terminal for inline or alternate screen mode
    if config.inline.is_some() {
        enable_raw_mode().map_err(|e| terminal_error("Could not enable raw mode", e, span))?;
        execute!(stdout).map_err(|e| terminal_error("Could not execute stdout", e, span))?;
        let backend = CrosstermBackend::new(stdout);
        let height = config.inline.unwrap_or(10);
        Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Inline(height as u16),
            },
        )
        .map_err(|e| terminal_error("Could not initialize terminal", e, span))
    } else {
        enable_raw_mode().map_err(|e| terminal_error("Could not enable raw mode", e, span))?;

        execute!(
            stdout,
            EnterAlternateScreen,
            EnableMouseCapture,
            Show,
            SetCursorStyle::SteadyBar,
            Clear(ClearType::All)
        )
        .map_err(|e| terminal_error("Could not execute stdout", e, span))?;
        let backend = CrosstermBackend::new(stdout);
        Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Fullscreen,
            },
        )
        .map_err(|e| terminal_error("Could not initialize terminal", e, span))
    }
}

fn restore_terminal(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    span: Span,
    config: Config,
) -> Result<(), ShellError> {
    disable_raw_mode().map_err(|e| terminal_error("Could not disable raw mode", e, span))?;
    if config.inline.is_none() {
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )
        .map_err(|e| terminal_error("Could not leave alternate screen", e, span))?;
    } else {
        execute!(terminal.backend_mut())
            .map_err(|e| terminal_error("Could not close inline backend", e, span))?;
        terminal
            .clear()
            .map_err(|e| terminal_error("Could not clear inline lines from buffer", e, span))?;
    }
    terminal
        .show_cursor()
        .map_err(|e| terminal_error("Could not show terminal cursor", e, span))
}
