use std::{fs, path::PathBuf, str::FromStr, process};

use crossterm::terminal;

fn main() {
    let args: Args = argh::from_env();

    let game = GameState {
        style: args.style_config,
        board_size: args.board_size.unwrap_or(args.level.board_size()),
        num_mines: args.num_mines.unwrap_or(args.level.num_mines()),
    };

    'game: loop {
        match game.run() {
            Ok(true) => continue 'game,
            Ok(false) => break 'game,
            Err(err) => {
                eprintln!("error drawing to terminal: {}", err.to_string());
                terminal::disable_raw_mode().expect("failed to disable raw mode");
                process::exit(1);
            }
        }
    }
}

#[derive(Debug)]
struct GameState {
    style: BoardStyle,
    board_size: (u32, u32),
    num_mines: u32,
}

impl GameState {
    pub fn run(&self) -> crossterm::Result<bool> {
        Ok(true)
    }
}

/// Minesweeper in the terminal.
#[derive(Clone, Debug, argh::FromArgs)]
struct Args {
    #[argh(
        option,
        short = 'l',
        description = "which level preset to use",
        default = "LevelPreset::default()"
    )]
    level: LevelPreset,
    #[argh(
        option,
        short = 'c',
        description = "path to style config file",
        default = "BoardStyle::default()"
    )]
    style_config: BoardStyle,
    #[argh(
        option,
        short = 's',
        description = "custom size of the board",
        from_str_fn(board_size_from_str)
    )]
    board_size: Option<(u32, u32)>,
    #[argh(
        option,
        short = 'm',
        description = "number of mines on the board"
    )]
    num_mines: Option<u32>,
}

/// Preset level styles.
#[derive(Copy, Clone, Debug)]
enum LevelPreset {
    Beginner,
    Intermediate,
    Advanced,
}

impl Default for LevelPreset {
    fn default() -> Self {
        Self::Beginner
    }
}

impl FromStr for LevelPreset {
    type Err = String;

    fn from_str(str: &str) -> Result<Self, Self::Err> {
        match str.to_lowercase().as_str() {
            "1" | "beginner" => Ok(Self::Beginner),
            "2" | "intermediate" => Ok(Self::Intermediate),
            "3" | "advanced" => Ok(Self::Advanced),
            _ => Err("level must be one of 1,2,3,beginner,intermediate,advanced".to_string()),
        }
    }
}

impl LevelPreset {
    pub fn board_size(&self) -> (u32, u32) {
        match self {
            LevelPreset::Beginner => (9, 9),
            LevelPreset::Intermediate => (16, 16),
            LevelPreset::Advanced => (24, 24),
        }
    }

    pub fn num_mines(&self) -> u32 {
        match self {
            LevelPreset::Beginner => 10,
            LevelPreset::Intermediate => 40,
            LevelPreset::Advanced => 99,
        }
    }
}

fn board_size_from_str(str: &str) -> Result<(u32, u32), String> {
    let dims: Vec<Result<u32, String>> = str
        .split(',')
        .map(|c| c.parse::<u32>().map_err(|err| err.to_string()))
        .collect::<Vec<Result<u32, String>>>();

    let x: u32 = dims.get(0).unwrap_or(&Ok(9)).clone()?;
    let y: u32 = dims.get(1).unwrap_or(&Ok(9)).clone()?;

    return Ok((x, y));
}

/// Info about the way the game should be drawn to the screen
#[derive(Copy, Clone, Debug, serde::Serialize, serde::Deserialize)]
struct BoardStyle {
    /// char to be used for empty cells
    pub empty: char,
    /// char to be used for mine cells
    pub mine: char,
    /// char to be used for covered cells
    pub covered: char,
    /// char to be used for marked cells
    pub marked: char,
    /// # of space chars to be used between cells
    pub gap_x: u8,
    /// # of newline chars between rows
    pub gap_y: u8,
}

impl Default for BoardStyle {
    fn default() -> Self {
        Self {
            empty: ' ',
            mine: '!',
            covered: '.',
            marked: '?',
            gap_x: 1,
            gap_y: 1,
        }
    }
}

impl FromStr for BoardStyle {
    type Err = String;

    fn from_str(str: &str) -> Result<Self, Self::Err> {
        return serde_yaml::from_slice(
            &fs::read(PathBuf::from_str(str).map_err(|err| err.to_string())?)
                .map_err(|err| err.to_string())?,
        )
        .map_err(|err| err.to_string());
    }
}
