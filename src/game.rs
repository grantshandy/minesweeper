use core::num;
use std::io::Write;

use crossterm::{
    cursor::{MoveTo, MoveToNextLine, Show},
    style::{Print, Stylize},
    terminal::{self, Clear, ClearType},
    ExecutableCommand, Result, event::{Event, KeyCode, self},
};
use rand::seq::SliceRandom;

#[derive(Copy, Clone, PartialEq)]
enum Cell {
    Empty,
    Adjacent(usize),
    Mine,
}

pub fn run_game<W: Write>(out: &mut W, level: u8) -> Result<()> {
    let mut out = out;

    out.execute(Show)?;

    // get width and height
    let (width, height): (usize, usize) = match level {
        1 => (9, 9),
        2 => (16, 16),
        3 => (24, 24),
        _ => (9, 9),
    };

    // set the number of mines in the game from the level
    let num_mines: usize = match level {
        1 => 10,
        2 => 40,
        3 => 99,
        _ => 10,
    };

    // X left-right
    // Y top-bottom

    // Y<C<Cell>>
    let mut data = create_blank_data(width, height);
    // this starts at 0
    let mut selection: (usize, usize) = (0, 0);

    // this turns on when we first click on a square.
    // it lets us place the mines after the user has interacted with it so they don't lose on their first time.
    let mut is_touched = false;

    terminal::enable_raw_mode()?;

    loop {
        // draw the board
        draw_board(out, width, height, &mut data)?;

        // move our cursor to the selection
        out.execute(MoveTo((selection.0 * 4) as u16, (selection.1 * 2) as u16))?;

        // get our next event
        let event = event::read()?;

        // set our selection for the next cycle
        selection = match get_next_selection(event, selection, width, height) {
            Some(next_selection) => next_selection,
            None => selection,
        };

        // if we pressed enter or space (or if we pressed q quit the game)
        if match event {
            Event::Key(key) => match key.code {
                KeyCode::Enter => true,
                KeyCode::Char(char) => match char {
                    ' ' => true,
                    'q' => {
                        crate::exit_message(&mut out)?;
                        break;
                    }
                    _ => false,
                }
                _ => false,
            }
            _ => false,
        } {
            if !is_touched {
                populate_data(width, height, selection, &mut data, num_mines);

                is_touched = true;
            }

            data[selection.1][selection.0].0 = true;
        }
    }

    terminal::disable_raw_mode()?;
 
    Ok(())
}

fn get_next_selection(event: Event, selection: (usize, usize), width: usize, height: usize) -> Option<(usize, usize)> {
    let width = width - 1;
    let height = height - 1;

    match event {
        Event::Key(key) => match key.code {
            KeyCode::Up => {
                if selection.1 == 0 {
                    Some(selection)
                } else {
                    Some((selection.0, selection.1 - 1))
                }
            },
            KeyCode::Down => {
                // why do I have to subtract 2 for it to work??
                if selection.1 < height {
                    Some((selection.0, selection.1 + 1))
                } else {
                    Some(selection)
                }
            },
            KeyCode::Right => {
                if selection.0 == width && selection.1 < height{
                    Some((0, selection.1 + 1))
                } else if selection.1 <= height && selection.0 != width {
                    Some((selection.0 + 1, selection.1))
                } else {
                    Some(selection)
                }
            },
            KeyCode::Left => {
                if selection.0 == 0 && selection.1 != 0 {
                    Some((width, selection.1 - 1))
                } else if selection.0 == 0 && selection.1 == 0 {
                    Some(selection)
                } else {
                    Some((selection.0 - 1, selection.1))
                }
            },
            _ => None,
        },
        _ => None,
    }
}

fn draw_board<W: Write>(out: &mut W, width: usize, height: usize, data: &Vec<Vec<(bool, Cell)>>) -> Result<()> {
    out.execute(Clear(ClearType::All))?.execute(MoveTo(0, 0))?;

    for y in 0..height {
        for x in 0..width {
            match data[y][x].0 {
                true => match data[y][x].1 {
                    Cell::Adjacent(num) => out.execute(Print(format!("{}   ", num)))?,
                    Cell::Mine => out.execute(Print("!   ".red().bold()))?,
                    Cell::Empty => out.execute(Print("    "))?,
                },
                false => out.execute(Print("x   "))?,
            };
        }

        out.execute(MoveToNextLine(2))?;
    }

    Ok(())
}

// Y<X<is_uncovered, Cell>>
fn create_blank_data(width: usize, height: usize) -> Vec<Vec<(bool, Cell)>> {
    let mut data: Vec<Vec<(bool, Cell)>> = Vec::new();

    for _y in 0..height {
        let mut row_data: Vec<(bool, Cell)> = Vec::new();

        for _x in 0..width {
            row_data.push((false, Cell::Empty));
        }

        data.push(row_data);
    }

    return data;
}

fn populate_data(width: usize, height: usize, selected: (usize, usize), data: &mut Vec<Vec<(bool, Cell)>>, num_mines: usize) {
    // the number of cells we need to fill
    let length = width * height;

    // Add our mines to the list
    let mut mine_locations: Vec<bool> = Vec::with_capacity(length);
    for _n in 0..num_mines {
        mine_locations.push(true);
    };
    for _n in 0..(length - num_mines) {
        mine_locations.push(false);
    }

    let mut rng = rand::thread_rng();
    mine_locations.shuffle(&mut rng);


    // go through the data and assign every cell a mine or not depending on whether or not they are in the mine locations vec
    let mut current_num = 0;
    for y in 0..height {
        for x in 0..width {
            data[y][x].1 = match mine_locations[current_num] {
                true => Cell::Mine,
                false => Cell::Empty,
            };
            current_num += 1;
        }
    }

    // make sure that our currently selected one is empty so the user doesn't fail on their first try
    // if it's not we need to shuffle again until it is
    loop {
        if data[selected.1][selected.0].1 == Cell::Empty {
            break;
        } else {
            mine_locations.shuffle(&mut rng);
        }
    }

    // go through the cells and calculate which are adjacent
    for y in 0..height {
        for x in 0..width {
            match data[y][x].1 {
                Cell::Empty => {
                    let mut num_adjacent_mines: usize = 0;

                    // WEST
                    if x > 0 {
                        if data[y][x - 1].1 == Cell::Mine {
                            num_adjacent_mines += 1;
                        }
                    }

                    // EAST
                    if x < width - 1 {
                        if data[y][x + 1].1 == Cell::Mine {
                            num_adjacent_mines += 1;
                        }
                    }

                    // SOUTH
                    if y < height - 1 {
                        if data[y + 1][x].1 == Cell::Mine {
                            num_adjacent_mines += 1;
                        }
                    }

                    // SOUTH WEST
                    if y < height - 1 && x > 0 {
                        if data[y + 1][x - 1].1 == Cell::Mine {
                            num_adjacent_mines += 1;
                        }
                    }

                    // SOUTH EAST
                    if y < height - 1 && x < width - 1{
                        if data[y + 1][x + 1].1 == Cell::Mine {
                            num_adjacent_mines += 1;
                        }
                    }


                    // NORTH
                    if y > 0 {
                        if data[y - 1][x].1 == Cell::Mine {
                            num_adjacent_mines += 1;
                        }
                    }

                    // NORTH EAST
                    if y > 0 && x < width - 1 {
                        if data[y - 1][x + 1].1 == Cell::Mine {
                            num_adjacent_mines += 1;
                        }
                    }

                    // NORTH WEST
                    if y > 0 && x > 0 {
                        if data[y - 1][x - 1].1 == Cell::Mine {
                            num_adjacent_mines += 1;
                        }
                    }


                    if num_adjacent_mines > 0 {
                        data[y][x].1 = Cell::Adjacent(num_adjacent_mines);
                    }
                },
                _ => continue,
            }
        }
    }
}