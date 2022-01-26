#[macro_use]
extern crate clap;

use std::io::{stdout, Stdout, Write};

use crossterm::{
    cursor::{Hide, MoveTo, MoveToNextLine, Show},
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    style::{Print, Stylize},
    terminal::{self, Clear, ClearType},
    ExecutableCommand, Result,
};
use rand::prelude::SliceRandom;

const MENU: &'static str = r#"Welcome to Minesweeper

Controls:
    q - quit
    arrow keys/wasd - navigate board
    enter/space - uncover cell
    m/? - mark cell

1. Beginner – 9 * 9 Board and 10 Mines
2. Intermediate – 16 * 16 Board and 40 Mines
3. Advanced – 24 * 24 Board and 99 Mines"#;

const EMPTY: char = ' ';
const MINE: char = '!';
const COVERED: char = 'x';
const MARKED: char = '?';

// characters (spaces) between cells
const SPACE_WIDTH: &'static str = "   ";

// number of newlines between rows (>= 1)
const SPACE_HEIGHT: usize = 2;

// uncover everything at the beginning
// use this for debugging
const SHOW_EVERYTHING: bool = false;

fn main() {
    let app = app_from_crate!()
        .arg(arg!(-l --level <LEVEL> "Which level to play (1-3, defaults to 1 if other is specified)").required(false))
        .get_matches();

    match game_loop(app.value_of("level")) {
        Ok(_) => (),
        Err(error) => {
            eprintln!("Error: {}", error);
            std::process::exit(1);
        }
    }
}

fn game_loop(level: Option<&str>) -> Result<()> {
    loop {
        let game = match level {
            Some(level) => Game::run(stdout(), level.parse::<usize>().unwrap_or(1)),
            None => Game::init(),
        };

        match game {
            Ok(restart) => match restart {
                true => continue,
                false => break,
            }
            Err(error) => return Err(error),
        }
    }

    let mut out = stdout();

    Game::reset_terminal(&mut out)?;
    Game::exit_message(&mut out)?;

    Ok(())
}

#[derive(Copy, Clone, PartialEq)]
enum CellType {
    Empty,
    Adjacent(usize),
    Mine,
}

enum Input {
    // Space / Enter
    Select,
    // Returns the next cursor direction from arrow keys/wasd
    Direction((usize, usize)),
    // q
    Quit,
    // m
    Mark,
}

#[derive(Copy, Clone, PartialEq)]
struct Cell {
    covered: bool,
    cell_type: CellType,
    marked: bool,
}

pub struct Game {
    out: Stdout,
    // Y<X<(covered, CellType)>>
    data: Vec<Vec<Cell>>,
    num_mines: usize,
    width: usize,
    height: usize,
    selection: (usize, usize),
    is_touched: bool,
    show_everything: bool,
}

impl Game {
    pub fn init() -> Result<bool> {
        let mut out = stdout();
        let level = Self::choose_level(&mut out)?;

        Self::run(out, level)
    }

    pub fn run(out: Stdout, level: usize) -> Result<bool> {
        let data = Vec::new();
        let is_touched = false;

        let (width, height) = match level {
            1 => (9, 9),
            2 => (16, 16),
            3 => (24, 24),
            _ => (9, 9),
        };

        let num_mines = match level {
            1 => 10,
            2 => 40,
            3 => 99,
            _ => 10,
        };

        // starts at 0!! the board starts at 1.
        let selection = ((width / 2), (width / 2));
        let show_everything = SHOW_EVERYTHING;

        let mut myself = Self {
            out,
            data,
            num_mines,
            width,
            height,
            selection,
            is_touched,
            show_everything,
        };

        myself.create_blank_board();

        terminal::enable_raw_mode()?;

        // show the cursor
        myself.out.execute(Show)?;

        // draw the boards initial state
        myself.draw_board()?;
        myself.update_cursor()?;

        loop {
            // this event blocks the thread so we don't loop forever and ruin performance.
            let event = event::read()?;

            match myself.get_input(event) {
                Some(input) => match input {
                    Input::Direction(next_selection) => {
                        myself.selection = next_selection;
                        myself.update_cursor()?;
                        continue;
                    }
                    Input::Mark => {
                        if !myself.get_current_cell().marked && myself.get_current_cell().covered {
                            myself.data[myself.selection.1][myself.selection.0].marked = true;
                        } else {
                            myself.data[myself.selection.1][myself.selection.0].marked = false;
                        }
                    }
                    Input::Quit => {
                        return Ok(false);
                    }
                    Input::Select => {
                        if !myself.is_touched {
                            myself.is_touched = true;
                            myself.data[myself.selection.1][myself.selection.0].marked = false;
                            myself.populate_board();
                        }

                        myself.uncover_current_cell();
                    }
                },
                None => continue,
            }

            if myself.get_current_cell().cell_type == CellType::Mine && myself.get_current_cell().covered == false {
                myself.out.execute(Hide)?;

                loop {
                    myself.draw_board()?;
                    myself.out
                        .execute(MoveTo(0, ((SPACE_HEIGHT * myself.height) + 1) as u16))?
                        .execute(Print("You lost! press r to restart and q to quit"))?;

                    let event = event::read()?;

                    if event == Event::Key(KeyEvent {
                        code: KeyCode::Char('q'),
                        modifiers: KeyModifiers::empty(),
                    }) {
                        return Ok(false);
                    } else if event == Event::Key(KeyEvent {
                        code: KeyCode::Char('r'),
                        modifiers: KeyModifiers::empty(),
                    }) {
                        return Ok(true);
                    }
                }
            }

            // update the board after everything else is done
            myself.draw_board()?;
        }
    }

    // fn has_won(&mut self) -> bool {
    //     for y in self.data {
    //         for x in y {
    //             if cell.cell_type == Cell::Mine && cell.covered == false {
    //                 return 
    //             }
    //         }
    //     }
    // }

    fn uncover_current_cell(&mut self) {
        // clear the current cell
        self.data[self.selection.1][self.selection.0].covered = false;
        self.data[self.selection.1][self.selection.0].marked = false;

        // clear the empty cells around it if we're already empty
        if self.get_current_cell().cell_type == CellType::Empty {
            self.remove_surrounding_empty_cells(self.selection);
        }
    }

    fn remove_surrounding_empty_cells(&mut self, cell: (usize, usize)) {
        for (x, y, cell_type) in self.get_surrounding_cells(cell) {
            if cell_type == CellType::Empty && self.data[y][x].covered == true {
                self.data[y][x].covered = false;
                self.remove_surrounding_empty_cells((x, y));
            } else {
                self.data[y][x].covered = false;
            }
        }
    }

    fn get_current_cell(&self) -> Cell {
        return self.data[self.selection.1][self.selection.0];
    }

    fn update_cursor(&mut self) -> Result<()> {
        // move our cursor to the correct position
        let right = (self.selection.0 * (SPACE_WIDTH.chars().count() + 1)) as u16;
        let up = ((self.height - (self.selection.1 + 1)) * SPACE_HEIGHT) as u16;

        self.out.execute(MoveTo(right, up))?;

        Ok(())
    }

    // particularly proud of this function
    fn populate_board(&mut self) {
        // Random mine placement indice idea: credit @asuradev99
        let num_cells = self.width * self.height;

        let mut mine_indeces: Vec<usize> = (0..num_cells).collect();

        // remove the spot at our cursor from the indices
        mine_indeces.remove((self.selection.1 * self.width) + self.selection.0);

        // remove all the spots around our cursor so that we don't click on an adjacent square.
        for (x, y, _cell) in self.get_surrounding_cells((self.selection.0, self.selection.1)) {
            mine_indeces.remove(
                mine_indeces
                    .iter()
                    .rposition(|&a| a == ((y * self.width) + x))
                    .unwrap(),
            );
        }

        // shuffle mine placement
        mine_indeces.shuffle(&mut rand::thread_rng());

        // place mines on board based on indices
        for i in &mine_indeces[0..self.num_mines] {
            let x = i % self.width;
            let y = i / self.height;
            self.data[y][x].cell_type = CellType::Mine;
        }

        // add adjacent cells
        for y in 0..self.height {
            for x in 0..self.width {
                if self.data[y][x].cell_type == CellType::Empty {
                    let mut num_adj_mines = 0;

                    for cell in self.get_surrounding_cells((x, y)) {
                        if cell.2 == CellType::Mine {
                            num_adj_mines += 1;
                        }
                    }

                    if num_adj_mines > 0 {
                        self.data[y][x].cell_type = CellType::Adjacent(num_adj_mines);
                    }
                }
            }
        }
    }

    fn draw_board(&mut self) -> Result<()> {
        // clear the screen
        self.out
            .execute(Clear(ClearType::All))?
            .execute(MoveTo(0, 0))?;

        // draw all of the dots
        // we reverse the iterator so that we'll show the data from the bottom up while still drawing from the top down
        for line in self.data.iter().rev() {
            for cell in line {
                if cell.marked {
                    self.out
                        .execute(Print(&format!("{}{}", MARKED.cyan().bold(), SPACE_WIDTH)))?;
                } else if cell.covered && !self.show_everything {
                    self.out.execute(Print(format!("{COVERED}{SPACE_WIDTH}")))?;
                } else {
                    match cell.cell_type {
                        CellType::Empty => {
                            self.out.execute(Print(format!("{EMPTY}{SPACE_WIDTH}")))?
                        }
                        CellType::Adjacent(num) => self.out.execute(Print(format!(
                            "{}{}",
                            num.to_string().bold(),
                            SPACE_WIDTH
                        )))?,
                        CellType::Mine => self.out.execute(Print(format!(
                            "{}{}",
                            MINE.red().bold(),
                            SPACE_WIDTH
                        )))?,
                    };
                }
            }

            self.out.execute(MoveToNextLine(SPACE_HEIGHT as u16))?;
        }

        self.update_cursor()?;

        Ok(())
    }

    fn create_blank_board(&mut self) {
        self.data.clear();

        for _y in 0..self.height {
            let mut row_data = Vec::new();

            for _x in 0..self.width {
                row_data.push(Cell {
                    covered: true,
                    cell_type: CellType::Empty,
                    marked: false,
                });
            }

            self.data.push(row_data);
        }
    }

    fn choose_level<W: Write>(out: &mut W) -> Result<usize> {
        let mut level = 1;
        let mut out = out;

        terminal::enable_raw_mode()?;

        // hide the cursor
        out.execute(Hide)?;

        // loop on every keypress
        loop {
            // clear the screen
            out.execute(Clear(ClearType::All))?.execute(MoveTo(0, 0))?;

            // draw the menu
            for line in MENU.split("\n") {
                // if the line as our number we draw it in bold to show our selection
                if line.contains(&format!("{}. ", level)) {
                    out.execute(Print(line.bold()))?;
                } else {
                    out.execute(Print(line))?;
                }

                out.execute(MoveToNextLine(1))?;
            }

            // get our event
            // this blocks so the loop doesn't run constantly
            let event = event::read()?;

            // quit if we press q
            if event
                == Event::Key(KeyEvent {
                    code: KeyCode::Char('q'),
                    modifiers: KeyModifiers::empty(),
                })
            {
                Self::exit_message(&mut out)?;
                std::process::exit(0);
            }

            // get our next level from the key event
            level = match event {
                Event::Key(key) => match key.code {
                    KeyCode::Up => level - 1,
                    KeyCode::Down => level + 1,
                    KeyCode::Enter => break,
                    KeyCode::Char(char) => match char {
                        ' ' => break,
                        _ => continue,
                    },
                    _ => continue,
                },
                _ => continue,
            };

            // if we try to set level as a level out of bounds set it back in bounds
            if level > 3 {
                level = 3;
            } else if level < 1 {
                level = 1;
            }
        }

        Self::reset_terminal(out)?;

        // return our level :)
        Ok(level)
    }

    // make sure the terminal is back to normal
    fn reset_terminal<W: Write>(out: &mut W) -> Result<()> {
        terminal::disable_raw_mode()?;
        out.flush()?;

        Ok(())
    }

    // print a nice goodbye message and clear the screen
    fn exit_message<W: Write>(out: &mut W) -> Result<()> {
        let mut out = out;

        Self::reset_terminal(&mut out)?;

        out.execute(Clear(ClearType::All))?
            .execute(MoveTo(0, 0))?
            .execute(Show)?
            .execute(Print("Thanks for playing!"))?
            .execute(MoveToNextLine(2))?;
        Ok(())
    }

    // return true if the cell exists on the board
    fn cell_exists(&self, cell: (usize, usize)) -> bool {
        if self.data.get(cell.1) != None && self.data[cell.1].get(cell.0) != None {
            return true;
        } else {
            return false;
        }
    }

    fn get_surrounding_cells(&self, cell: (usize, usize)) -> Vec<(usize, usize, CellType)> {
        let cell = (cell.0 as isize, cell.1 as isize);
        let directions = [
            (-1, 1),  // NW
            (-1, 0),  // W
            (-1, -1), // SW
            (0, 1),   // N
            (0, -1),  // S
            (1, 1),   // NE
            (1, 0),   // E
            (1, -1),  // SE
        ];

        let mut cells = Vec::new();

        for d in directions {
            if self.cell_exists(((cell.0 + d.0) as usize, (cell.1 + d.1) as usize)) {
                cells.push((
                    (cell.0 + d.0) as usize,
                    (cell.1 + d.1) as usize,
                    self.data[(cell.1 + d.1) as usize][(cell.0 + d.0) as usize].cell_type,
                ));
            }
        }

        return cells;
    }

    fn get_input(&self, event: Event) -> Option<Input> {
        let selection = (self.selection.0 as isize, self.selection.1 as isize);

        let change = match event {
            Event::Key(key) => match key.code {
                KeyCode::Enter => return Some(Input::Select),
                KeyCode::Up => (selection.0, selection.1 + 1),
                KeyCode::Down => (selection.0, selection.1 - 1),
                KeyCode::Left => (selection.0 - 1, selection.1),
                KeyCode::Right => (selection.0 + 1, selection.1),
                KeyCode::Char(char) => match char {
                    'q' => return Some(Input::Quit),
                    ' ' => return Some(Input::Select),
                    'w' => (selection.0, selection.1 + 1),
                    's' => (selection.0, selection.1 - 1),
                    'a' => (selection.0 - 1, selection.1),
                    'd' => (selection.0 + 1, selection.1),
                    'm' => return Some(Input::Mark),
                    '?' => return Some(Input::Mark),
                    _ => return None,
                },
                _ => return None,
            },
            _ => return None,
        };

        if self.cell_exists((change.0 as usize, change.1 as usize)) {
            Some(Input::Direction((change.0 as usize, change.1 as usize)))
        } else {
            Some(Input::Direction(self.selection))
        }
    }
}
