use std::str::Chars;

#[derive(Copy, Clone)]
enum Escape {
    Foreground(u8),
    Background(u8),
    Reset,
}

#[derive(Copy, Clone)]
struct Section {
    fg: u8,
    bg: u8,
}

#[derive(Copy, Clone)]
pub enum Shell {
    Any,
    Zsh,
    Bash,
}

const OPEN_BRACE: char = '{';
const CLOSE_BRACE: char = '}';

pub fn generate(template: &str, shell: Shell) -> Result<String, String> {
    let mut buffer = String::new();

    let mut sections: Vec<Section> = Vec::new();
    let mut active_section: Option<Section> = None;
    let mut last_brace: Option<char> = None;

    let mut chars = template.chars();

    while let Some(c) = chars.next() {
        match last_brace {
            Some(brace) if brace != c => {
                push_brace(
                    &mut buffer,
                    brace,
                    active_section.as_ref(),
                    sections.last(),
                    shell,
                );
                active_section = sections.last().copied();
            }
            _ => (),
        }

        last_brace = match c {
            OPEN_BRACE => {
                sections.push(read_meta(&mut chars)?);
                Some(OPEN_BRACE)
            }
            CLOSE_BRACE => {
                sections.pop();
                Some(CLOSE_BRACE)
            }
            _ => {
                buffer.push(c);
                None
            }
        };
    }

    if let Some(last_brace) = last_brace {
        push_brace(
            &mut buffer,
            last_brace,
            active_section.as_ref(),
            sections.last(),
            shell,
        );
    }

    Ok(buffer)
}

fn read_meta(chars: &mut Chars) -> Result<Section, String> {
    let mut buffer = String::new();

    let meta: Vec<&str> = {
        for c in chars.take_while(|c| *c != ':') {
            buffer.push(c);
        }
        buffer.split(',').collect()
    };

    if meta.len() != 2 {
        return Err("Both fg and bg should be specified".to_string());
    }

    let fg: u8 = match meta[0].parse::<u8>() {
        Ok(fg) => fg,
        Err(e) => return Err(format!("Invalid fg: {}", e.to_string())),
    };

    let bg: u8 = match meta[1].parse::<u8>() {
        Ok(bg) => bg,
        Err(e) => return Err(format!("Invalid bg: {}", e.to_string())),
    };

    Ok(Section { fg, bg })
}

fn push_brace(
    buffer: &mut String,
    brace: char,
    current: Option<&Section>,
    next: Option<&Section>,
    shell: Shell,
) {
    if brace == OPEN_BRACE {
        if let Some(next) = next {
            push_escape_code(buffer, Escape::Foreground(next.bg), shell);
            buffer.push('');
            push_escape_code(buffer, Escape::Foreground(next.fg), shell);
            push_escape_code(buffer, Escape::Background(next.bg), shell);
        }
    } else if brace == CLOSE_BRACE {
        let escape = match next {
            Some(next) => Escape::Background(next.bg),
            None => Escape::Reset,
        };

        push_escape_code(buffer, escape, shell);

        if let Some(current) = current {
            push_escape_code(buffer, Escape::Foreground(current.bg), shell);
            buffer.push('');
        }

        let escape = match next {
            Some(next) => Escape::Foreground(next.fg),
            None => Escape::Reset,
        };

        push_escape_code(buffer, escape, shell);
    }
}

fn push_escape_code(buffer: &mut String, escape: Escape, shell: Shell) {
    let escape = match escape {
        Escape::Foreground(color) => format!("38;5;{}", color),
        Escape::Background(color) => format!("48;5;{}", color),
        Escape::Reset => "0".to_string(),
    };

    match shell {
        Shell::Zsh => buffer.push_str("%{"),
        Shell::Bash => buffer.push_str("\\["),
        _ => (),
    }

    buffer.push_str("\x1b[");
    buffer.push_str(&escape);
    buffer.push('m');

    match shell {
        Shell::Zsh => buffer.push_str("%}"),
        Shell::Bash => buffer.push_str("\\]"),
        _ => (),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_section() {
        assert_eq!(
            generate("{0,1:xxx}", Shell::Any),
            Ok("\x1b[38;5;1m\x1b[38;5;0m\x1b[48;5;1mxxx\x1b[0m\x1b[38;5;1m\x1b[0m".to_string())
        );
    }

    #[test]
    fn one_section_zsh() {
        assert_eq!(
            generate("{0,1:xxx}", Shell::Zsh),
            Ok("%{\x1b[38;5;1m%}%{\x1b[38;5;0m%}%{\x1b[48;5;1m%}xxx%{\x1b[0m%}%{\x1b[38;5;1m%}%{\x1b[0m%}".to_string())
        );
    }

    #[test]
    fn one_section_bash() {
        assert_eq!(
            generate("{0,1:xxx}", Shell::Bash),
            Ok("\\[\x1b[38;5;1m\\]\\[\x1b[38;5;0m\\]\\[\x1b[48;5;1m\\]xxx\\[\x1b[0m\\]\\[\x1b[38;5;1m\\]\\[\x1b[0m\\]".to_string())
        );
    }

    #[test]
    fn sequential_sections() {
        assert_eq!(
            generate("{0,1:xxx} {100,200:yyy}", Shell::Any),
            Ok("\x1b[38;5;1m\x1b[38;5;0m\x1b[48;5;1mxxx\x1b[0m\x1b[38;5;1m\x1b[0m \x1b[38;5;200m\x1b[38;5;100m\x1b[48;5;200myyy\x1b[0m\x1b[38;5;200m\x1b[0m".to_string())
        );
    }

    #[test]
    fn overlap_left() {
        assert_eq!(
            generate("{0,1:xxx {100,200:yyy}}", Shell::Any),
            Ok("\x1b[38;5;1m\x1b[38;5;0m\x1b[48;5;1mxxx \x1b[38;5;200m\x1b[38;5;100m\x1b[48;5;200myyy\x1b[0m\x1b[38;5;200m\x1b[0m".to_string())
        );
    }

    #[test]
    fn overlap_right() {
        assert_eq!(
            generate("{0,1:{100,200:yyy} xxx}", Shell::Any),
            Ok("\x1b[38;5;200m\x1b[38;5;100m\x1b[48;5;200myyy\x1b[48;5;1m\x1b[38;5;200m\x1b[38;5;0m xxx\x1b[0m\x1b[38;5;1m\x1b[0m".to_string())
        );
    }

    #[test]
    fn bad_fg() {
        assert_eq!(
            generate("{999,1:xxx}", Shell::Any),
            Err("Invalid fg: number too large to fit in target type".to_string())
        );
    }

    #[test]
    fn bad_bg() {
        assert_eq!(
            generate("{1,-9:xxx}", Shell::Any),
            Err("Invalid bg: invalid digit found in string".to_string())
        );
    }

    #[test]
    fn incomplete_meta() {
        assert_eq!(
            generate("{1:xxx}", Shell::Any),
            Err("Both fg and bg should be specified".to_string())
        );
    }
}
