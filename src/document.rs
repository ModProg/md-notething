use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};

use pulldown_cmark::Event;
use yew::{classes, html, Html};
use Command::*;

pub struct Document {
    pub elements: Vec<Element>,
    pub active_element: usize,
}

impl Document {
    pub fn render(&self) -> Html {
        html! {
            {for self.elements.iter().map(Element::render)}
        }
    }
}

impl Commandee for Document {
    fn command(&mut self, command: &Self::Command) -> Self::Response {
        let len = self.elements.len();
        let cell = &mut self.elements[self.active_element];
        match (command, cell.command(command)) {
            (Command::Up | Command::Left, false) => {
                if self.active_element > 0 {
                    cell.command(&CursorLeave);
                    self.active_element -= 1;
                    self.elements[self.active_element]
                        .command(&CursorEnter(Option::<Side>::from(command).unwrap()));
                }
            }
            (Command::Down | Command::Right, false) => {
                if self.active_element < len - 1 {
                    cell.command(&CursorLeave);
                    self.active_element += 1;
                    self.elements[self.active_element]
                        .command(&CursorEnter(Option::<Side>::from(command).unwrap()));
                }
            }
            _ => return false,
        };
        true
    }
}

pub trait Markdown<'a> {
    fn parse_from_md<T>(md: &mut T) -> Self
    where
        T: Iterator<Item = Event<'a>>;

    fn to_md(self) -> String;
}

#[derive(Clone, Copy)]
pub enum Side {
    Top,
    Left,
    Right,
    Bottom,
}

impl From<&Command> for Option<Side> {
    fn from(value: &Command) -> Self {
        match value {
            Up => Some(Side::Bottom),
            Left => Some(Side::Right),
            Down => Some(Side::Top),
            Right => Some(Side::Left),
            _ => None,
        }
    }
}

#[non_exhaustive]
#[derive(Clone)]
pub enum Command {
    Up,
    Left,
    Down,
    Right,
    CursorEnter(Side),
    CursorLeave,
}
pub trait Commandee {
    type Command = Command;
    type Response = bool;
    fn command(&mut self, command: &Self::Command) -> Self::Response;
}

pub enum Element {
    Table(Table),
}

impl Element {
    fn render(&self) -> Html {
        match self {
            Element::Table(table) => table.render(),
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

#[derive(PartialEq)]
pub struct Table {
    // cells: Vec<Vec<TableCell>>,
    pub(crate) cells: HashMap<(usize, usize), TableCell>,
    pub(crate) active_cell: (usize, usize),
}

impl Table {
    fn neighbor(&mut self, direction: &Command) -> Option<&mut TableCell> {
        match direction {
            Command::Up => self
                .cells
                .get_mut(&(self.active_cell.0, self.active_cell.1 - 1)),
            Command::Left => self
                .cells
                .get_mut(&(self.active_cell.0 - 1, self.active_cell.1)),
            Command::Down => self
                .cells
                .get_mut(&(self.active_cell.0, self.active_cell.1 + 1)),
            Command::Right => self
                .cells
                .get_mut(&(self.active_cell.0 + 1, self.active_cell.1)),
            _ => None,
        }
    }

    fn render(&self) -> Html {
        html! {
            <table class={classes!("table-auto")}>
            </table>
        }
    }
}


impl Commandee for Table {
    fn command(&mut self, command: &Self::Command) -> Self::Response {
        let mut cell = self
            .cells
            .remove(&self.active_cell)
            .expect("I made sure of the existense somewhere else I hope");

        let this = self.neighbor(command);
        let res = match (&command, cell.command(command), this) {
            (
                Command::Up | Command::Down | Command::Left | Command::Right,
                false,
                Some(neighbor),
            ) => {
                neighbor.content.set_cursor(0);
                cell.content.remove_cursor();
                true
            }
            (Command::Left | Command::Right, false, None) => true,
            _ => todo!(),
        };
        self.cells.insert(self.active_cell, cell);
        res
    }
}

#[derive(PartialEq)]
pub struct TableCell {
    pub content: Paragraph,
}

impl Deref for TableCell {
    type Target = Paragraph;

    fn deref(&self) -> &Self::Target {
        &self.content
    }
}

impl DerefMut for TableCell {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.content
    }
}

impl Commandee for TableCell {
    fn command(&mut self, command: &Self::Command) -> Self::Response {
        self.content.command(command)
    }
}

#[derive(PartialEq)]
pub struct Paragraph {
    pub text: Vec<String>,
    pub cursor: Option<usize>,
}

impl Paragraph {
    fn set_cursor(&mut self, pos: usize) {
        self.cursor = Some(pos.min(self.text.len()));
    }
    fn remove_cursor(&mut self) {
        self.cursor = None;
    }
}

impl Commandee for Paragraph {
    fn command(&mut self, command: &Self::Command) -> Self::Response {
        if let Some(cursor) = &mut self.cursor {
            match command {
                Command::Left if *cursor != 0 => {
                    *cursor -= 1;
                    true
                }
                Command::Down if *cursor != self.text.len() - 1 => {
                    *cursor -= 1;
                    true
                }
                _ => false,
            }
        } else {
            false
        }
    }
}
