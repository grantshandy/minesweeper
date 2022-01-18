use std::io::Write;

use crossterm::{
    cursor::{MoveTo, MoveToNextLine, Hide},
    event::{self, Event, KeyCode},
    style::{Print, Stylize},
    terminal::{self, Clear, ClearType},
    ExecutableCommand, Result,
};

const MENU: &'static str = r#"Welcome to Minesweeper.
Press q at any time to quit.

1. Beginner – 9 * 9 Board and 10 Mines
2. Intermediate – 16 * 16 Board and 40 Mines
3. Advanced – 24 * 24 Board and 99 Mines"#;

pub fn choose_level<W: Write>(out: &mut W) -> Result<u8> {
    let mut out = out;

    terminal::enable_raw_mode()?;

    let mut level = 1;

    loop {
        out.execute(Clear(ClearType::All))?.execute(MoveTo(0, 0))?;

        for line in MENU.split("\n") {

            if line.contains(&format!("{}. ", level)) {
                out.execute(Print(line.bold()))?;
            } else {
                out.execute(Print(line))?;
            }

            out.execute(MoveToNextLine(1))?;
        }

        out.flush()?;
        out.execute(Hide)?;

        let event = event::read()?;

        let mut next_level = match event {
            Event::Key(key) => match key.code {
                KeyCode::Up => level - 1,
                KeyCode::Down => level + 1,
                KeyCode::Enter => break,
                KeyCode::Char(char) => match char {
                    'q' => {
                        crate::exit_message(&mut out)?;
                        std::process::exit(0);
                    }
                    _ => continue,
                },
                _ => continue,
            },
            _ => continue,
        };

        if next_level > 3 {
            next_level = 3;
        } else if next_level < 1 {
            next_level = 1;
        }

        level = next_level;
    }

    out.flush()?;
    out.execute(Clear(ClearType::All))?;

    terminal::disable_raw_mode()?;

    Ok(level)
}
