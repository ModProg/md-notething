use std::collections::HashMap;

use derive_more::Deref;
use gloo_console::console_dbg as dbg;
use pulldown_cmark::{Event, Tag};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;
use yew::{classes, html, Classes, Html};
use Command::*;

use crate::ApplicationState;

pub trait Markdown<'a> {
    fn parse_from_md<T>(md: &mut T) -> Self
    where
        T: Iterator<Item = Event<'a>>;

    fn to_md(self) -> String;
}

pub trait Commandee {
    type Command = Command;
    type Response = bool;
    fn command(&mut self, command: &Self::Command) -> Self::Response;
}

pub trait Render {
    fn render(&self, state: &ApplicationState) -> Html;
}

pub struct Document {
    pub elements: Vec<Element>,
    pub active_element: usize,
}

impl Render for Document {
    fn render(&self, state: &ApplicationState) -> Html {
        html! {
            {for self.elements.iter().map(|e|e.render(state))}
        }
    }
}

impl Commandee for Document {
    fn command(&mut self, command: &Self::Command) -> Self::Response {
        let len = self.elements.len();
        let element = &mut self.elements[self.active_element];
        match (command, element.command(command)) {
            (Command::Up | Command::Left, false) => {
                if self.active_element > 0 {
                    element.command(&CursorLeave);
                    self.active_element -= 1;
                    self.elements[self.active_element].command(&CursorEnterH(true));
                }
            }
            (Command::Down | Command::Right, false) => {
                if self.active_element < len - 1 {
                    element.command(&CursorLeave);
                    self.active_element += 1;
                    self.elements[self.active_element].command(&CursorEnterH(false));
                }
            }
            _ => return false,
        };
        true
    }
}

impl<'a> Markdown<'a> for Document {
    fn parse_from_md<T>(md: &mut T) -> Self
    where
        T: Iterator<Item = Event<'a>>,
    {
        let mut md = md.peekable();
        let mut document = Self {
            active_element: 0,
            elements: vec![],
        };

        while md.peek().is_some() {
            if let Some(element) = Option::<Element>::parse_from_md(&mut md) {
                document.elements.push(element)
            }
        }

        document
    }

    fn to_md(self) -> String {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Deref)]
pub struct Characters(Vec<String>);

impl<S> From<S> for Characters
where
    S: AsRef<str>,
{
    fn from(s: S) -> Self {
        Self(s.as_ref().graphemes(true).map(String::from).collect())
    }
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum Motion {
    Up,
    Left,
    Down,
    Right,
}

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Up,
    Left,
    Down,
    Right,
    CursorEnterH(bool),
    CursorEnterV(usize, bool),
    CursorLeave,
    Insert(Characters),
    Delete(Motion),
}

impl Command {
    fn horizontal(&self) -> bool {
        matches!(self, Left | Right | CursorEnterH(_))
    }
}

pub enum Element {
    Table(Table),
}

impl Render for Element {
    fn render(&self, state: &ApplicationState) -> Html {
        match self {
            Element::Table(table) => table.render(state),
        }
    }
}

impl Commandee for Element {
    type Command = Command;

    type Response = bool;

    fn command(&mut self, command: &Self::Command) -> Self::Response {
        match self {
            Element::Table(table) => table.command(command),
        }
    }
}

impl<'a> Markdown<'a> for Option<Element> {
    fn parse_from_md<T>(md: &mut T) -> Self
    where
        T: Iterator<Item = Event<'a>>,
    {
        let mut md = md.peekable();

        if let Some(event) = md.peek() {
            match event {
                Event::Start(Tag::Table(_)) => Some(Element::Table(Table::parse_from_md(&mut md))),
                _ => None,
                // Event::End(_) => todo!(),
                // Event::Text(_) => todo!(),
                // Event::Code(_) => todo!(),
                // Event::Html(_) => todo!(),
                // Event::FootnoteReference(_) => todo!(),
                // Event::SoftBreak => todo!(),
                // Event::HardBreak => todo!(),
                // Event::Rule => todo!(),
                // Event::TaskListMarker(_) => todo!(),
            }
        } else {
            None
        }

        // while md.peek().is_some() {
        //     if let Some(element) = Element::parse_from_md(md) {
        //         document.elements.push(element)
        //     }
        // }
        //
        // document
    }

    fn to_md(self) -> String {
        todo!()
    }
}

#[derive(PartialEq)]
pub struct Table {
    // cells: Vec<Vec<TableCell>>,
    pub cells: HashMap<(usize, usize), Paragraph>,
    pub active_cell: Option<(usize, usize)>,
    pub height: usize,
    pub width: usize,
}

impl Table {
    fn neighbor(&mut self, direction: &Command) -> Option<(usize, usize)> {
        if let Some(active_cell) = self.active_cell {
            Some(match direction {
                Command::Up => (active_cell.0, active_cell.1 - 1),
                Command::Left => (active_cell.0 - 1, active_cell.1),
                Command::Down => (active_cell.0, active_cell.1 + 1),
                Command::Right => (active_cell.0 + 1, active_cell.1),
                _ => return None,
            })
        } else {
            None
        }
    }
    fn cell(&self, x: usize, y: usize) -> Option<&Paragraph> {
        self.cells.get(&(x, y))
    }
}

impl Render for Table {
    fn render(&self, state: &ApplicationState) -> Html {
        html! {
            <table class={classes!("table-auto")}>
            {
                for (0..self.height).map(|y| {
                    html!{
                        <tr>
                        {
                            for (0..self.width).map(|x| {
                                html!{
                                    <td class="border px-2 h-10">
                                    {self.cell(x,y).map(|c|c.render(state)).unwrap_or_default()}
                                    </td>
                                }
                            })
                        }
                        </tr>
                    }
                })
            }
            </table>
        }
    }
}

impl Commandee for Table {
    fn command(&mut self, command: &Self::Command) -> Self::Response {
        if let Some(active_cell) = self.active_cell {
            let neighbor = self.neighbor(command);
            let cell = self
                .cells
                .get_mut(&active_cell)
                .expect("I made sure of the existense somewhere else I hope");

            match (&command, cell.command(command), neighbor) {
                (
                    Command::Up | Command::Down | Command::Left | Command::Right,
                    false,
                    Some(neighbor),
                ) => {
                    let cursor = cell.get_normalized_cursor().unwrap();
                    cell.command(&CursorLeave);
                    self.cells
                        .get_mut(&neighbor)
                        .unwrap()
                        .command(&if command.horizontal() {
                            CursorEnterH(command == &Left)
                        } else {
                            CursorEnterV(cursor, command == &Up)
                        });
                    self.active_cell = Some(neighbor);
                    true
                }
                (Command::Left | Command::Right, true, _) => true,
                (Command::Left | Command::Right, false, None) => false,
                (_, true, _) => true,
                (Delete(Motion::Left), ..) => true,
                _ => todo!(),
            }
        } else {
            match command {
                CursorEnterH(false) | CursorEnterV(_, false) => {
                    self.active_cell = Some((0, 0));
                    self.cells
                        .get_mut(&self.active_cell.unwrap())
                        .unwrap()
                        .command(command);
                }
                _ => return false,
            }
            true
        }
    }
}

impl<'a> Markdown<'a> for Table {
    fn parse_from_md<T>(md: &mut T) -> Self
    where
        T: Iterator<Item = Event<'a>>,
    {
        let mut md = md.peekable();
        let mut table = Table {
            cells: HashMap::new(),
            active_cell: None,
            height: 0,
            width: 0,
        };
        loop {
            if matches!(md.peek(), Some(Event::End(Tag::Table(_)))) {
                md.next();
                break;
            }
            match md.next().unwrap() {
                Event::Start(Tag::TableRow) => table.width = 0,
                Event::End(Tag::TableCell) => table.width += 1,
                Event::End(Tag::TableRow | Tag::TableHead) => {
                    table.height += 1;
                    dbg!(table.height);
                }
                Event::Text(text) => table
                    .cells
                    .entry((table.width, table.height))
                    .or_default()
                    .text
                    .extend(text.graphemes(true).map(String::from)),
                e => {
                    dbg!(e);
                }
            }
        }
        table
    }

    fn to_md(self) -> String {
        todo!()
    }
}

// #[derive(PartialEq)]
// pub struct TableCell {
//     pub content: Paragraph,
// }
//
// impl Deref for TableCell {
//     type Target = Paragraph;
//
//     fn deref(&self) -> &Self::Target {
//         &self.content
//     }
// }
//
// impl DerefMut for TableCell {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         &mut self.content
//     }
// }
//
// impl Commandee for TableCell {
//     fn command(&mut self, command: &Self::Command) -> Self::Response {
//         self.content.command(command)
//     }
// }

#[derive(PartialEq, Debug, Default)]
pub struct Paragraph {
    pub text: Vec<String>,
    pub cursor: Option<usize>,
}

impl Paragraph {
    fn get_normalized_cursor(&self) -> Option<usize> {
        self.cursor.map(|cursor| {
            self.text
                .iter()
                .take(cursor)
                .map(|s| s.width().min(2))
                .sum()
        })
    }
    fn set_normalized_cursor(&mut self, mut normalized_cursor: usize) {
        let mut actual_cursor = 0;
        let mut widths = self.text.iter().map(|s| s.width().min(2));
        while normalized_cursor > 0 {
            if let Some(width) = widths.next() {
                if normalized_cursor >= width {
                    normalized_cursor -= width;
                    actual_cursor += 1;
                    continue;
                }
            }
            break;
        }
        self.cursor = Some(actual_cursor);
    }
}

impl Commandee for Paragraph {
    fn command(&mut self, command: &Self::Command) -> Self::Response {
        match (command, &mut self.cursor) {
            (Left, Some(cursor)) if *cursor != 0 => *cursor -= 1,
            (Right, Some(cursor)) if *cursor != self.text.len() - 1 => *cursor += 1,
            (CursorLeave, Some(_)) => self.cursor = None,
            (CursorEnterH(false), _) => self.cursor = Some(0),
            (CursorEnterH(true), _) => self.cursor = Some(self.text.len() - 1),
            (CursorEnterV(cursor, _), _) => self.set_normalized_cursor(*cursor), // self.cursor = Some((*cursor).min(self.text.len() - 1)),
            (Delete(Motion::Left), Some(cursor)) if *cursor > 0 => {
                self.text.remove(*cursor - 1);
                *cursor -= 1;
            }
            (Insert(chars), Some(cursor)) => {
                if chars.0.contains(&"\n".to_string()) {
                    todo!("line breaking")
                }

                let remainder = self.text.split_off(*cursor);

                self.text.extend(chars.iter().map(String::from));
                // let mut new_lines: Vec<_> = lines.map(TextLine::from).collect();
                // let mut move_action = vec![];

                // let last_line_len;
                // if !new_lines.is_empty() {
                //     move_action.push(Msg::CursorMove(0, new_lines.len() as i32));
                //     let last_line = new_lines.last_mut().unwrap();
                //     move_action.push(Msg::CursorPos(Some(last_line.len()), None));
                //     last_line_len = last_line.len();
                //     &mut last_line.characters
                // } else {
                // move_action.push(Msg::CursorMove(graphemes.len() as i32, 0));
                // last_line_len = self.len();
                // &mut self.characters
                // }
                self.text.extend(
                    remainder
                        .into_iter()
                        // .enumerate()
                        // .map(|(i, (c, s, _))| (c, s, i + last_line_len)),
                );
                *cursor += chars.len();

                // (move_action, new_lines)
            }
            _ => {
                dbg!(&self, command);
                return false;
            }
        }
        true
    }
}

fn char_span(c: &str, mut classes: Classes) -> Html {
    if c.width() > 1 {
        // dbg!(c);
        classes.push("inline-flex");
        classes.push("w-[2ch]");
        // classes.push("flex");
        classes.push("justify-center");
        // html!{
        //     <div class="relative w-[2ch] h-[1em] inline-block">
        //         <svg viewbox="0 0 100 100" class="absolute truncate">
        //          // <ellipse cx="173" cy="2" rx="100" ry="58" />
        //             // <text textLength="100" dominant-baseline="hanging" lengthAdjust="spacingAndGlyphs">{c}</text>
        //         </svg>
        //     </div>
        // }
    } //else {
    html! {
        <span class={classes}>{c}</span>
    }
    //}
}

impl Render for Paragraph {
    fn render(&self, state: &ApplicationState) -> Html {
        html! {
            {for self.text.iter().enumerate().map(|(i, character)|
                 html!{
                     if self.cursor == Some(i) {
                     <span class={classes!(
                             (self.cursor == Some(i)).then_some(state.cursor_style.classes()))}>{char_span(character, classes!("relative", "z-10"))}</span>
                     } else {
                         {char_span(character,classes!())}
                     }
                 }
            )}
        }
    }
}
