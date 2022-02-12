use std::io::{stdout, Stdout, Write};

use crossterm::{
    cursor::{Hide, MoveTo, MoveToNextLine, Show},
    event::{self, Event, KeyCode},
    style::{Print, Stylize, StyledContent, self, Color},
    terminal::{self, Clear, ClearType},
    ExecutableCommand, Result,
};

use rand::prelude::SliceRandom;

const MENU: &'static str = r#"Welcome to Minesweeper
Copyright 2022 Grant Handy

Controls:
    q - quit
    r - restart
    arrow keys/wasd - navigate board
    enter/space - uncover cell
    m/? - mark cell

1. Beginner – 9 * 9 Board and 10 Mines
2. Intermediate – 16 * 16 Board and 40 Mines
3. Advanced – 24 * 24 Board and 99 Mines"#;

const EMPTY: char = ' ';
const MINE: char = '!';
const COVERED: char = '·';
const MARKED: char = '?';

// characters (spaces) between cells (default "   ")
const SPACE_WIDTH: &'static str = " ";
// number of newlines inbetween lines
const SPACE_HEIGHT: usize = 0;

// uncover everything at the beginning
// use this for debugging
const SHOW_EVERYTHING: bool = false;

fn main() {
    let app = clap::app_from_crate!()
        .arg(clap::arg!(-l --level <LEVEL> "Which level to play (1-3, defaults to 1 if other is specified)").required(false))
        .get_matches();

    let mut game = match Game::new(app.value_of("level")) {
        Ok(game) => game,
        Err(error) => {
            Game::reset_terminal().unwrap();
            eprintln!("Game initialization error: {error}");
            std::process::exit(1);
        }
    };

    loop {
        match game.run() {
            Ok(res) => match res {
                true => continue,
                false => break,
            }
            Err(error) => {
                Game::reset_terminal().unwrap();
                eprintln!("Game runtime error: {error}");
                std::process::exit(1);
            }
        }
    }

    Game::exit_message().unwrap();
}

#[derive(Copy, Clone, PartialEq, Debug)]
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
    // r
    Restart,
}

#[derive(Copy, Clone, PartialEq, Debug)]
struct Cell {
    covered: bool,
    cell_type: CellType,
    marked: bool,
}

#[derive(Debug)]
pub struct Game {
    // we use a single Stdout for simplicity
    out: Stdout,
    // board data: Y<X<Cell>>
    data: Vec<Vec<Cell>>,
    // number of mines on the board
    num_mines: usize,
    // width of the board
    width: usize,
    // height of the board
    height: usize,
    // coordinates of where our cursor is at the moment
    selection: (usize, usize),
    // if the board has been touched
    is_touched: bool,
    // if we should show everything
    show_everything: bool,
}

impl Game {
    pub fn new(level: Option<&str>) -> Result<Self> {
        let mut out = stdout();
        let data = Vec::new();
        let is_touched = false;

        let level = match level {
            Some(level) => level.parse::<u8>().unwrap_or(1),
            None => Self::choose_level(&mut out)?,
        };

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
        let selection = ((width / 2), (height / 2));
        let show_everything = SHOW_EVERYTHING;

        Ok(Self {
            out,
            data,
            num_mines,
            width,
            height,
            selection,
            is_touched,
            show_everything,
        })
    }

    pub fn run(&mut self) -> Result<bool> {
        self.create_blank_board();

        // reset data from last game
        self.selection = ((self.width / 2), (self.height / 2));
        self.show_everything = SHOW_EVERYTHING;
        self.is_touched = false;

        terminal::enable_raw_mode()?;

        // show the cursor
        self.out.execute(Show)?;

        // draw the boards initial state
        self.draw_board()?;
        self.update_cursor()?;

        loop {
            // this event blocks the thread until we get a keypress
            let event = event::read()?;

            // get an Input from the event
            match self.get_input(event) {
                // if we have one...
                Some(input) => match input {
                    // if it's a new direction update the cursor and reload the loop.
                    Input::Direction(next_selection) => {
                        self.selection = next_selection;
                        self.update_cursor()?;
                        continue;
                    }
                    // if the user said to mark the cell
                    Input::Mark => {
                        // if it isn't already marked and uncovered
                        if !self.get_current_cell().marked && self.get_current_cell().covered {
                            // mark it
                            self.data[self.selection.1][self.selection.0].marked = true;
                        } else if self.get_current_cell().covered {
                            // otherwise unmark it if it's covered
                            self.data[self.selection.1][self.selection.0].marked = false;
                        } else {
                            // if it's uncovered already then restart the loop. no need to redraw and fill up the terminal buffer.
                            continue;
                        }
                    }
                    // if the user quit then end the function sending "false" which means don't restart it. Go to the exit message.
                    Input::Quit => {
                        return Ok(false);
                    },
                    // if the user selected the cell...
                    Input::Select => {
                        // if we haven't touched the board yet populate the board so the user doesn't click on a mine their first try
                        if !self.is_touched {
                            self.is_touched = true;
                            self.populate_board();
                        }

                        // if we clicked on an uncovered on restart the loop and don't redraw
                        if !self.get_current_cell().covered {
                            continue;
                        }

                        // uncover the cell
                        self.uncover_cell(self.selection);
                    }
                    Input::Restart => return Ok(true),
                },
                // if the input was not a recognized one then restart the loop and wait for the next input.
                // the less times we redraw the board the better, we don't want to fill up the terminal buffer.
                None => continue,
            }

            // if we clicked on a mine go to the losing screen.
            if self.get_current_cell().cell_type == CellType::Mine
                && self.get_current_cell().covered == false
            {
                return self.end_screen("You lost! press r to try again and q to quit");
            }

            // if we won go to the winning screen
            if self.has_won() {
                return self.end_screen("You won! press r to play again and q to quit");
            }

            // update the board on screen after everything else is done
            self.draw_board()?;
        }
    }

    // the screen that shows up when you lose or win
    fn end_screen(&mut self, message: &str) -> Result<bool> {
        // hide the cursor
        self.out.execute(Hide)?;

        // show everything to the user because they've lost
        // it's nice for them to see how they could've won
        self.show_everything = true;

        // update the board state
        self.draw_board()?;

        // print the message at the bottom of the board
        self.out
            .execute(MoveTo(0, (((SPACE_HEIGHT + 1) * self.height) + 1) as u16))?
            .execute(Print(message.bold()))?;

        // loop through the events.
        loop {
            let event = event::read()?;

            match event {
                Event::Key(key) => match key.code {
                    KeyCode::Char(char) => match char {
                        // return false because we don't want to restart
                        'q' => return Ok(false),
                        // return true because we want to restart
                        'r' => return Ok(true),
                        _ => continue,
                    },
                    _ => continue,
                },
                _ => continue,
            }
        }
    }

    fn has_won(&mut self) -> bool {
        let mut num_uncovered_cells = 0;

        for y in &self.data {
            for cell in y {
                if !cell.covered {
                    if cell.cell_type == CellType::Mine {
                        return false;
                    }

                    num_uncovered_cells += 1;
                }
            }
        }

        if num_uncovered_cells == ((self.width * self.height) - self.num_mines) {
            return true;
        } else {
            return false;
        }
    }

    // uncover the cell the cursor is currently on
    fn uncover_cell(&mut self, cell: (usize, usize)) {
        // clear the current cell
        self.data[cell.1][cell.0].covered = false;
        self.data[cell.1][cell.0].marked = false;

        // clear the empty cells around it if we're empty
        if self.get_current_cell().cell_type == CellType::Empty {
            self.remove_surrounding_empty_cells(cell);
        }
    }

    // recursively remove the surrounding empty cells of a cell
    fn remove_surrounding_empty_cells(&mut self, cell: (usize, usize)) {
        for (x, y, cell_type) in self.get_surrounding_cells(cell) {
            if cell_type == CellType::Empty && self.data[y][x].covered == true {
                self.data[y][x].covered = false;
                self.data[y][x].marked = false;
                self.remove_surrounding_empty_cells((x, y));
            } else {
                self.data[y][x].covered = false;
                self.data[y][x].marked = false;
            }
        }
    }

    // a simple shortcut function that gives us the Cell the cursor is at
    fn get_current_cell(&self) -> Cell {
        return self.data[self.selection.1][self.selection.0];
    }

    // update's the cursor's position on screen from memory. this doesn't take a terminal redraw.
    fn update_cursor(&mut self) -> Result<()> {
        let right = (self.selection.0 * (SPACE_WIDTH.chars().count() + 1)) as u16;
        let up = ((self.height - (self.selection.1 + 1)) * (SPACE_HEIGHT + 1)) as u16;

        self.out.execute(MoveTo(right, up))?;

        Ok(())
    }

    // this was the most technical function in the program, it randomly places mines on the board
    // the hardest part was not placing any mines where the user's cursor is, and not adjacent to the cursor either
    // this is done so that the user's first click is not on a bomb or adjacent square so they can have a chance to win each time
    fn populate_board(&mut self) {
        // Random mine placement indice idea: credit @asuradev99
        let num_cells = self.width * self.height;

        let mut mine_indices: Vec<usize> = (0..num_cells).collect();

        // remove the spot at our cursor from the indices
        mine_indices.remove((self.selection.1 * self.width) + self.selection.0);

        // remove all the spots around our cursor so that we don't click on an adjacent square.
        for (x, y, _cell) in self.get_surrounding_cells(self.selection) {
            // we need to search the indices by value for the correct index to remove using rposition.
            // this is done because each time we remove a cell it skews the positions of all other ones by 1.
            mine_indices.remove(
                mine_indices
                    .iter()
                    .rposition(|&a| a == ((y * self.width) + x))
                    .unwrap(),
            );
        }

        // shuffle mine placement using rand
        mine_indices.shuffle(&mut rand::thread_rng());

        // place mines on board based on indices
        for i in &mine_indices[0..self.num_mines] {
            let x = i % self.width;
            let y = i / self.height;
            self.data[y][x].cell_type = CellType::Mine;
        }

        // add "adjacent" cells based on where the bombs are
        for y in 0..self.height {
            for x in 0..self.width {
                // if the cell is empty
                if self.data[y][x].cell_type == CellType::Empty {
                    // the number of adjacent mines to the cell
                    let mut num_adj_mines = 0;

                    // go through all the surrounding cells and add one to num_adj_mines if the cell is a mine
                    for cell in self.get_surrounding_cells((x, y)) {
                        if cell.2 == CellType::Mine {
                            num_adj_mines += 1;
                        }
                    }

                    // if we have any adjacent mines set our cell's type as adjacent with the number of mines
                    if num_adj_mines > 0 {
                        self.data[y][x].cell_type = CellType::Adjacent(num_adj_mines);
                    }
                }
            }
        }
    }

    // draw the board to the terminal based on the game's internal state
    fn draw_board(&mut self) -> Result<()> {
        // clear the screen and move to 0,0
        self.out
            .execute(Clear(ClearType::All))?
            .execute(MoveTo(0, 0))?;

        // draw all of the cells
        // we reverse the iterator so that we'll show the data from the bottom up (1st quadrant of a Cartesian plane) while still drawing from the top down
        for line in self.data.iter().rev() {
            for cell in line {
                // if the cell is marked we aren't showing everything
                let cell = if cell.marked && !self.show_everything {
                    // print the marked symbol as cyan and bold
                    format!("{}{}", MARKED.cyan().bold(), SPACE_WIDTH)
                // if the cell is covered and we aren't showing everything
                } else if cell.covered && !self.show_everything {
                    // print the covered symbol
                    format!("{COVERED}{SPACE_WIDTH}")
                } else {
                    // else print the symbol from what the data is normally
                    match cell.cell_type {
                        CellType::Empty => format!("{EMPTY}{SPACE_WIDTH}"),
                        CellType::Adjacent(num) => format!(
                            "{}{}",
                            Self::create_stylized_num(num).bold(),
                            SPACE_WIDTH
                        ),
                        CellType::Mine => format!(
                            "{}{}",
                            MINE.red().bold(),
                            SPACE_WIDTH
                        ),
                    }
                };

                self.out.execute(Print(cell))?;
            }

            // move the correct number of lines :)
            self.out
                .execute(MoveToNextLine((SPACE_HEIGHT + 1) as u16))?;
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

    fn choose_level<W: Write>(out: &mut W) -> Result<u8> {
        let mut level = 1;
        let mut draw = true;

        terminal::enable_raw_mode()?;

        // hide the cursor
        out.execute(Hide)?;

        // loop on every keypress
        loop {
            if draw {
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
            }

            // get our event
            // this blocks so the loop doesn't run constantly
            let event = event::read()?;

            // get our next level from the key event
            level = match event {
                Event::Key(key) => match key.code {
                    KeyCode::Up => level - 1,
                    KeyCode::Down => level + 1,
                    KeyCode::Enter => break,
                    KeyCode::Char(char) => match char {
                        ' ' => break,
                        '1' => 1,
                        '2' => 2,
                        '3' => 3,
                        'q' => {
                            Self::exit_message()?;
                            std::process::exit(0);
                        },
                        _ => {
                            draw = false;
                            continue;
                        }
                    },
                    _ => {
                        draw = false;
                        continue;
                    }
                },
                _ => {
                    draw = false;
                    continue;
                }
            };

            // if we try to set level as a level out of bounds set it back in bounds
            if level > 3 {
                level = 3;
            } else if level < 1 {
                level = 1;
            }

            draw = true;
        }

        Self::reset_terminal()?;

        // return our level :)
        Ok(level)
    }

    // make sure the terminal is back to normal
    fn reset_terminal() -> Result<()> {
        terminal::disable_raw_mode()?;
        stdout()
            .execute(Clear(ClearType::All))?
            .execute(MoveTo(0, 0))?
            .execute(Show)?
            .flush()?;

        Ok(())
    }

    // print a nice goodbye message and clear the screen
    fn exit_message() -> Result<()> {
        Self::reset_terminal()?;

        stdout()
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
                    'r' => return Some(Input::Restart),
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

    fn create_stylized_num(num: usize) -> StyledContent<String> {
        match num {
            1 => style::style(num.to_string()).with(Color::Blue),
            2 => style::style(num.to_string()).with(Color::Green),
            3 => style::style(num.to_string()).with(Color::Yellow),
            4 => style::style(num.to_string()).with(Color::DarkBlue),
            5 => style::style(num.to_string()).with(Color::DarkGreen),
            6 => style::style(num.to_string()).with(Color::DarkMagenta),
            7 => style::style(num.to_string()).with(Color::Red),
            8 => style::style(num.to_string()).with(Color::DarkRed),
            _ => style::style(num.to_string()),
        }
    }
}
