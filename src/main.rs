#[macro_use]
extern crate clap;

use std::io::{Write, Stdout, stdout};

use crossterm::{
    cursor::{MoveTo, MoveToNextLine, Show, Hide},
    style::{Print, Stylize},
    terminal::{self, Clear, ClearType},
    ExecutableCommand, Result, event::{Event, KeyCode, self},
};
use rand::prelude::SliceRandom;

const MENU: &'static str = r#"Welcome to Minesweeper

Controls:
    q - quit
    arrow keys - navigate board
    enter/space - uncover cell

1. Beginner – 9 * 9 Board and 10 Mines
2. Intermediate – 16 * 16 Board and 40 Mines
3. Advanced – 24 * 24 Board and 99 Mines"#;

const EMPTY: char = ' ';
const MINE: char = '!';
const COVERED: char = 'X';

// uncover everything at the beginning
const SHOW_EVERYTHING: bool = true;

fn main() {
    let app = app_from_crate!()
        .arg(arg!(-l --level <LEVEL> "Which level to play (1-3, defaults to 1 if other is specified)").required(false))
        .get_matches();

    match match app.value_of("level") {
        Some(level) => Game::run(stdout(), level.parse::<usize>().unwrap_or(1)),
        None => Game::init(),
    } {
        Ok(_) => {},
        Err(error) => {
            eprintln!("Error: {}", error);
            std::process::exit(1);
        }
    }
}

#[derive(Copy, Clone, PartialEq)]
enum Cell {
    Empty,
    Adjacent(usize),
    Mine,
}

pub struct Game {
    out: Stdout,
    // Y<X<(covered, Cell)>>
    data: Vec<Vec<(bool, Cell)>>,
    num_mines: usize,
    width: usize,
    height: usize,
    selection: (usize, usize),
    is_touched: bool,
}

impl Game {
    pub fn init() -> Result<()> {
        let mut out = stdout();
        let level = Self::choose_level(&mut out)?;

        Self::run(out, level)?;

        Ok(())
    }

    pub fn run(out: Stdout, level: usize) -> Result<()> {
        let data: Vec<Vec<(bool, Cell)>> = Vec::new();
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

        let mut myself = Self {
            out,
            data,
            num_mines,
            width,
            height,
            selection,
            is_touched,
        };

        myself.create_blank_board();

        terminal::enable_raw_mode()?;

        // draw the boards initial state
        myself.draw_board()?;
        myself.update_cursor()?;

        loop {
            // this event blocks the thread so we don't loop forever and ruin performance.
            let event = event::read()?;

            // exit if we press q or Q
            if event.is_quit() {
                break;
            }

            // if we pressed select (enter/space) uncover the current cell
            if event.is_select() {
                if !myself.is_touched {
                    myself.populate_board();
                    myself.is_touched = true;
                }

                myself.uncover_current_cell();
            }

            // if we pressed a direction then update the cursor and continue the loop
            match event.handle_direction(&myself) {
                Some(next_selection) => {
                    myself.selection = next_selection;
                    myself.update_cursor()?;
                    continue;
                }
                None => {},
            };

            // update the board after everything else is done
            myself.draw_board()?;
        }

        Self::exit_message(&mut myself.out)?;
        std::process::exit(0);
    }

    fn uncover_current_cell(&mut self) {
        self.data[self.selection.1][self.selection.0].0 = false;
    }

    fn update_cursor(&mut self) -> Result<()> {
        // move our cursor to the correct position
        let right = (self.selection.0 * 4) as u16;
        let up = ((self.height - (self.selection.1 + 1)) * 2) as u16;

        self.out.execute(MoveTo(right, up))?;

        Ok(())
    }

    fn populate_board(&mut self) {
        let num_cells = self.width * self.height;

        let mut mine_locations: Vec<bool> = Vec::new();
        
        // add the correct number of mines as true
        for _cell in 0..self.num_mines {
            mine_locations.push(true);
        }

        // fill in the rest with false
        for _cell in 0..(num_cells - (self.num_mines + 1)) {
            mine_locations.push(false);
        }

        // self.out.execute(Print(format!("locations len: {}, num_cells: {}, num_mines: {}", mine_locations.len(), num_cells, self.num_mines))).unwrap();
        // std::thread::sleep(std::time::Duration::from_secs(3));

        mine_locations.shuffle(&mut rand::thread_rng());

        // add to the board
        let mut current_num = 0;

        for y in 0..self.height {
            for x in 0..self.width {
                if x != self.selection.0 && y != self.selection.1 {
                    if mine_locations[current_num] == true {
                        self.data[y][x].1 = Cell::Mine;
                    }
    
                    current_num += 1;
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
            for (uncovered, cell) in line {
                match *uncovered {
                    false => match *cell {
                        Cell::Empty => self.out.execute(Print(format!("{EMPTY}   ")))?,
                        Cell::Adjacent(num) => self.out.execute(Print(format!("{num}   ")))?,
                        Cell::Mine => self.out.execute(Print(format!("{MINE}   ").bold().red()))?,
                    }
                    true => self.out.execute(Print(format!("{COVERED}   ")))?,
                };
            }

            self.out.execute(MoveToNextLine(2))?;
        }

        self.update_cursor()?;

        Ok(())
    }

    fn create_blank_board(&mut self) {
        self.data.clear();

        for _y in 0..self.height {
            let mut row_data: Vec<(bool, Cell)> = Vec::new();
    
            for _x in 0..self.width {
                if SHOW_EVERYTHING {
                    row_data.push((false, Cell::Empty));
                } else {
                    row_data.push((true, Cell::Empty));
                }
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


            out.flush()?;

            // get our event
            // this blocks so the loop doesn't run constantly
            let event = event::read()?;

            // quit if we press q or Q
            if event.is_quit() {
                Self::exit_message(&mut out)?;
                std::process::exit(0);
            }

            // get our next level from the key event
            level = match event {
                Event::Key(key) => match key.code {
                    KeyCode::Up => level - 1,
                    KeyCode::Down => level + 1,
                    KeyCode::Enter => break,
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

        // make sure the terminal is set back to normal
        out
            .execute(Clear(ClearType::All))?
            .execute(MoveTo(0, 0))?
            .execute(Show)?;
    
        terminal::disable_raw_mode()?;
        out.flush()?;
    
        // return our level :)
        Ok(level)
    }

    // make sure the terminal is back to normal
    fn reset_terminal<W: Write>(out: &mut W) -> Result<()> {
        terminal::disable_raw_mode()?;
        out.flush()?;

        out
            .execute(Clear(ClearType::All))?
            .execute(MoveTo(0, 0))?
            .execute(Show)?;

        Ok(())
    }

    // print a nice goodbye message and clear the screen
    fn exit_message<W: Write>(out: &mut W) -> Result<()> {
        let mut out = out;

        Self::reset_terminal(&mut out)?;

        out
            .execute(Print("Thanks for playing!"))?
            .execute(MoveToNextLine(2))?;    
        Ok(())
    }

    // return true if the cell is on the right side of the board
    fn cell_is_on_right(&self, cell: (usize, usize)) -> bool {
        if cell.0 >= self.width - 1 {
            return true;
        }

        return false;
    }

    // return true if the cell is on the left side of the board
    fn cell_is_on_left(&self, cell: (usize, usize)) -> bool {
        if cell.0 == 0 {
            return true;
        }

        return false;
    }

    // return true if the cell is on the top of the board
    fn cell_is_on_top(&self, cell: (usize, usize)) -> bool {
        if cell.1 >= self.height - 1 {
            return true;
        }

        return false;
    }

    // return true if the cell is on the bottom of the board
    fn cell_is_on_bottom(&self, cell: (usize, usize)) -> bool {
        if cell.1 == 0 {
            return true;
        }

        return false;
    }
}

trait KeyDetector {
    fn is_select(&self) -> bool;
    fn is_quit(&self) -> bool;
    fn handle_direction(&self, game: &Game) -> Option<(usize, usize)>;
}

impl KeyDetector for Event {
    fn is_select(&self) -> bool {
        match self {
            Event::Key(key) => match key.code {
                KeyCode::Enter => true,
                KeyCode::Char(char) => match char {
                    ' ' => true,
                    _ => false,
                }
                _ => false,
            }
            _ => false,
        }
    }

    fn is_quit(&self) -> bool {
        match self {
            Event::Key(key) => match key.code {
                KeyCode::Enter => false,
                KeyCode::Char(char) => match char {
                    'q' => true,
                    'Q' => true,
                    _ => false,
                }
                _ => false,
            }
            _ => false,
        }
    }

    // returns the next selection from a direction on the keyboard
    // this is required because we must check that it's on a side of the board so we don't place the cursor outside of the board
    fn handle_direction(&self, game: &Game) -> Option<(usize, usize)> {
        match self {
            Event::Key(key) => match key.code {
                KeyCode::Up => {
                    if game.cell_is_on_top(game.selection) {
                        Some(game.selection)
                    } else {
                        Some((game.selection.0, game.selection.1 + 1))
                    }
                },
                KeyCode::Down => {
                    if game.cell_is_on_bottom(game.selection) {
                        Some(game.selection)
                    } else {
                        Some((game.selection.0, game.selection.1 - 1))
                    }
                },
                KeyCode::Left => {
                    if game.cell_is_on_left(game.selection) {
                        Some(game.selection)
                    } else {
                        Some((game.selection.0 - 1, game.selection.1))
                    }
                },
                KeyCode::Right => {
                    if game.cell_is_on_right(game.selection) {
                        Some(game.selection)
                    } else {
                        Some((game.selection.0 + 1, game.selection.1))
                    }
                },
                KeyCode::Char(char) => match char {
                    'w' => {
                        if game.cell_is_on_top(game.selection) {
                            Some(game.selection)
                        } else {
                            Some((game.selection.0, game.selection.1 + 1))
                        }
                    },
                    's' => {
                        if game.cell_is_on_bottom(game.selection) {
                            Some(game.selection)
                        } else {
                            Some((game.selection.0, game.selection.1 - 1))
                        }
                    },
                    'a' => {
                        if game.cell_is_on_left(game.selection) {
                            Some(game.selection)
                        } else {
                            Some((game.selection.0 - 1, game.selection.1))
                        }
                    }
                    'd' => {
                        if game.cell_is_on_right(game.selection) {
                            Some(game.selection)
                        } else {
                            Some((game.selection.0 + 1, game.selection.1))
                        }
                    },
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        }
    }
}