use std::io::{stdout, Write};

use crossterm::{
    style::{Attribute, SetAttribute, Print},
    ExecutableCommand, Result, terminal::{Clear, ClearType, self}, cursor::{MoveTo, MoveToNextLine, Show},
};

mod game;
mod level_selection;

fn main() {
    // we run application in a different function so I can handle any errors it might come across better
    match run() {
        Ok(_) => {}
        Err(error) => eprintln!("Error! {}", error.to_string()),
    }
}

fn run() -> Result<()> {
    let mut out = stdout();

    // ask the user for their level
    let level = level_selection::choose_level(&mut out)?;

    // start the game
    game::run_game(&mut out, level)?;

    stdout().execute(SetAttribute(Attribute::Reset))?;

    Ok(())
}

pub fn exit_message<W: Write>(out: &mut W) -> Result<()> {
    out.flush()?;
    terminal::disable_raw_mode()?;

    out
        .execute(Clear(ClearType::All))?
        .execute(MoveTo(0, 0))?
        .execute(Print("Thanks for playing!"))?
        .execute(MoveToNextLine(2))?
        .execute(Show)?;

    Ok(())
}