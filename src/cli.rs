use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "ttd", version, about = "A plain-text task manager inspired by todo.txt")]
pub struct Cli {
    /// Directory containing the task files
    #[arg(long)]
    pub task_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Add a new task, e.g. `ttd add due:2026-04-02 learning for exam`
    Add {
        #[arg(trailing_var_arg = true, num_args = 1..)]
        line: Vec<String>,
    },
    /// List all open tasks
    List,
    /// Mark a task as done
    Done {
        /// Task ID to mark as done
        id: String,
    },
    /// Search tasks by keyword
    Search {
        /// Search term
        query: String,
    },
}
