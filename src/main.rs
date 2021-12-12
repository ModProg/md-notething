#![feature(derive_default_enum, bool_to_option, associated_type_defaults)]
use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    iter::FromIterator,
    ops::{Deref, DerefMut},
};

use gloo_console::console_dbg;
use pulldown_cmark::{Options, Parser, Tag};
use unicode_segmentation::UnicodeSegmentation;
use uuid::Uuid;
use web_sys::{window, HtmlInputElement};
use yew::prelude::*;

use crate::document::{Document, Element, Paragraph, Table, TableCell};

mod document;

#[derive(PartialEq, Eq, Clone, Copy)]
enum Mode {
    Insert,
    Normal,
    Command,
}

#[allow(dead_code)]
impl Mode {
    fn is_insert(&self) -> bool {
        matches!(self, Self::Insert)
    }
    fn is_command(&self) -> bool {
        matches!(self, Self::Command)
    }
}

enum Msg {
    CursorMove(i32, i32),
    CursorPos(Option<usize>, Option<usize>),
    Write(String),
    Mode(Mode),
    ExecuteCommand,
}

struct Keypress {
    key: String,
    alt: bool,
    ctrl: bool,
    shift: bool,
}

impl From<&KeyboardEvent> for Keypress {
    fn from(ke: &KeyboardEvent) -> Self {
        Self {
            key: ke.key(),
            alt: ke.alt_key(),
            ctrl: ke.ctrl_key(),
            shift: ke.shift_key(),
        }
    }
}

#[allow(dead_code)]
struct KeyRef<'a> {
    key: &'a str,
    alt: bool,
    ctrl: bool,
    shift: bool,
}

impl PartialEq<&str> for KeyRef<'_> {
    fn eq(&self, other: &&str) -> bool {
        self.key == *other && !self.alt && !self.ctrl
    }
}

impl KeyRef<'_> {
    fn insertable(&self) -> bool {
        self.key.graphemes(true).count() == 1 && !self.alt && !self.ctrl
    }
}

impl Keypress {
    fn as_ref(&self) -> KeyRef {
        KeyRef {
            key: &self.key,
            alt: self.alt,
            ctrl: self.ctrl,
            shift: self.shift,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum TextStyle {
    Italic,
    Bold,
    Code,
    Cursor(CursorStyle),
    Table,
    TableCell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Position {
    First,
    Last,
    Sandwitched,
    Single,
}

impl Position {
    fn is_first(&self) -> bool {
        matches!(self, Self::First | Self::Single)
    }
    fn is_last(&self) -> bool {
        matches!(self, Self::Last | Self::Single)
    }
}

impl TextStyle {
    fn forground_classes(self, position: Position) -> Classes {
        console_dbg!((self, position));
        match self {
            TextStyle::Italic => classes!["italic"],
            TextStyle::Bold => classes!["font-bold"],
            TextStyle::Cursor(cursor_style) => cursor_style.classes(), //&["text-gray-900", "rounded", "bg-red-300"],
            TextStyle::Table => classes!["hidden", "whitespace-normal"],
            TextStyle::TableCell => classes![
                "unhidden",
                "border-t-2",
                "border-b-2",
                position
                    .is_first()
                    .then_some(classes!["border-l-2", "-ml-px"]),
                position
                    .is_last()
                    .then_some(classes!["border-r-2", "-mr-px"])
            ],
            _ => classes![],
        }
    }
    fn background_classes(self, position: Position) -> Classes {
        match self {
            // These are mirrored to help with non monospace spacing
            TextStyle::Italic => classes!["italic"],
            TextStyle::Bold => classes!["font-bold"],
            TextStyle::Code => classes![
                "bg-gray-600",
                position.is_first().then_some("rounded-l"),
                position.is_last().then_some("rounded-r")
            ],
            TextStyle::Table => classes!["hidden", "whitespace-normal"],
            TextStyle::TableCell => classes!["unhidden"],
            _ => classes![],
        }
    }

    fn positioned(
        &self,
        was_style: &HashSet<TextStyle>,
        will_style: &HashSet<TextStyle>,
    ) -> Position {
        match (was_style.contains(self), will_style.contains(self)) {
            (true, true) => Position::Sandwitched,
            (true, false) => Position::Last,
            (false, true) => Position::First,
            (false, false) => Position::Single,
        }
    }
}

#[derive(Default)]
struct TextLine {
    // content: String,
    key: uuid::Uuid,
    characters: Vec<(String, HashSet<TextStyle>, usize)>,
}

impl TextLine {
    fn len(&self) -> usize {
        self.characters.len()
    }
    fn clear(&mut self) {
        self.characters.clear()
    }
    fn char_len(&self) -> usize {
        self.characters.iter().map(|(s, ..)| s.len()).sum()
    }

    /// position must be in 0..=line.len()
    fn insert(&mut self, position: usize, value: &str) -> (Vec<Msg>, Vec<TextLine>) {
        let mut lines = value.split('\n');
        let graphemes: Vec<_> = lines
            .next()
            .expect("There should be a first item")
            .grapheme_indices(true)
            .collect();
        // self.characters.reserve(graphemes.len());
        let start_offset = if position == 0 {
            0
        } else {
            self.characters[position - 1].2 + self.characters[position - 1].0.len()
        };
        let remainder = self.characters.split_off(position);

        self.characters.extend(
            graphemes
                .iter()
                .map(|&(i, v)| (v.to_owned(), HashSet::new(), i + start_offset)),
        );
        let mut new_lines: Vec<_> = lines.map(TextLine::from).collect();
        let mut move_action = vec![];

        let last_line_len;
        if !new_lines.is_empty() {
            move_action.push(Msg::CursorMove(0, new_lines.len() as i32));
            let last_line = new_lines.last_mut().unwrap();
            move_action.push(Msg::CursorPos(Some(last_line.len()), None));
            last_line_len = last_line.len();
            &mut last_line.characters
        } else {
            move_action.push(Msg::CursorMove(graphemes.len() as i32, 0));
            last_line_len = self.len();
            &mut self.characters
        }
        .extend(
            remainder
                .into_iter()
                .enumerate()
                .map(|(i, (c, s, _))| (c, s, i + last_line_len)),
        );

        (move_action, new_lines)
    }
}

impl<S: AsRef<str>> From<S> for TextLine {
    fn from(s: S) -> Self {
        Self {
            key: Uuid::new_v4(),
            characters: s
                .as_ref()
                .grapheme_indices(true)
                .map(|(offset, value)| (value.to_owned(), HashSet::new(), offset))
                .collect(),
        }
    }
}

impl<'a> FromIterator<&'a TextLine> for String {
    fn from_iter<T: IntoIterator<Item = &'a TextLine>>(iter: T) -> Self {
        iter.into_iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl ToString for TextLine {
    fn to_string(&self) -> String {
        self.characters
            .iter()
            .map(|(v, ..)| v.to_string())
            .collect()
    }
}

impl DerefMut for TextLine {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.characters
    }
}
impl Deref for TextLine {
    type Target = [(String, HashSet<TextStyle>, usize)];

    fn deref(&self) -> &Self::Target {
        &self.characters
    }
}

struct Model {
    // `ComponentLink` is like a reference to a component.
    // It can be used to send messages to the component
    /// x 0..=lines[y].len(), y in 0..lines.len()
    cursor_position: (usize, usize),
    node_ref: NodeRef,
    cursor_ref: Cell<NodeRef>,
    // highlighting: Vec<(TextStyle, Range<usize>)>,
    // lines: Vec<(String, usize, NodeRef, Vec<(TextStyle, Range<usize>)>)>,
    lines: Vec<TextLine>,
    command: TextLine,
    mode: Mode,
    font: String,
}

impl Model {
    fn handle_key_press(event: KeyboardEvent, mode: Mode) -> Option<<Model as Component>::Message> {
        let ret = Some({
            let key = Keypress::from(&event);
            match mode {
                Mode::Insert => match key.as_ref() {
                    key if key == "Escape" => vec![Msg::Mode(Mode::Normal)],
                    key if key == "Enter" => vec![Msg::Write("\n".to_owned())],
                    key if key == "ArrowLeft" => vec![Msg::CursorMove(-1, 0)],
                    key if key == "ArrowDown" => vec![Msg::CursorMove(0, 1)],
                    key if key == "ArrowUp" => vec![Msg::CursorMove(0, -1)],
                    key if key == "ArrowRight" => vec![Msg::CursorMove(1, 0)],
                    key if key == "Backspace" => todo!(),
                    key if key.insertable() => vec![Msg::Write(key.key.to_owned())],
                    a => {
                        console_dbg!("Unknown keypress (insert)", a.key);
                        return None;
                    }
                },
                Mode::Normal => match key.as_ref() {
                    key if key == "i" => vec![Msg::Mode(Mode::Insert)],
                    key if key == ":" => vec![Msg::Mode(Mode::Command)],
                    key if key == "h" => vec![Msg::CursorMove(-1, 0)],
                    key if key == "j" => vec![Msg::CursorMove(0, 1)],
                    key if key == "k" => vec![Msg::CursorMove(0, -1)],
                    key if key == "l" => vec![Msg::CursorMove(1, 0)],
                    a => {
                        console_dbg!("Unknown keypress (normal)", a.key);
                        return None;
                    }
                },
                Mode::Command => match key.as_ref() {
                    key if key == "Escape" => vec![Msg::Mode(Mode::Normal)],
                    key if key == "Enter" => vec![Msg::ExecuteCommand, Msg::Mode(Mode::Normal)],
                    key if key == "ArrowLeft" => vec![Msg::CursorMove(-1, 0)],
                    key if key == "ArrowDown" => vec![Msg::CursorMove(0, 1)],
                    key if key == "ArrowUp" => vec![Msg::CursorMove(0, -1)],
                    key if key == "ArrowRight" => vec![Msg::CursorMove(1, 0)],
                    key if key.insertable() => vec![Msg::Write(key.key.to_owned())],
                    a => {
                        console_dbg!("Unknown keypress (command)", a.key);
                        return None;
                    }
                },
            }
        });
        event.prevent_default();
        ret
    }
    fn parse_md(&mut self) {
        let text = &self.lines.iter().collect::<String>();
        let options = Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS;
        let parser = Parser::new_ext(text, options);

        // let mut highlights: HashSet<TextStyle> = HashSet::new();

        let highlighting: Vec<_> = parser
            .into_offset_iter()
            .filter_map(|(elem, range)| {
                use pulldown_cmark::Event;
                Some((
                    match elem {
                        Event::Start(Tag::Emphasis) => TextStyle::Italic,
                        Event::Start(Tag::Strong) => TextStyle::Bold,
                        Event::Code(_) => TextStyle::Code,
                        Event::Start(Tag::Table(_)) => TextStyle::Table,
                        Event::Start(Tag::TableHead) => TextStyle::Bold,
                        Event::Start(Tag::TableCell) => TextStyle::TableCell,

                        _ => return None,
                    },
                    range,
                ))
                // {
                //     Some((highlights.clone(), range))
                // } else {
                //     None
                // }
            })
            .collect();

        let mut offset = 0;
        for line in self.lines.iter_mut() {
            for character in line.iter_mut() {
                character.1 = highlighting
                    .iter()
                    .filter_map(|(hi, range)| {
                        if range.start <= character.2 + offset && range.end > character.2 + offset {
                            Some(*hi)
                        } else {
                            None
                        }
                    })
                    .collect();
            }
            // +1 for linebreak
            offset += line.char_len() + 1;
        }
    }

    fn execute(&mut self, command: String) {
        for command in command.split_whitespace() {
            if let Some((name, value)) = command.split_once('=') {
                match name {
                    "font" => self.font = value.to_owned(),
                    _ => todo!(),
                }
            }
        }
    }
}

impl Component for Model {
    type Message = Vec<Msg>;
    type Properties = ();

    fn create(_props: &yew::Context<Model>) -> Self {
        let mut s = Self {
            cursor_position: (0, 0),
            node_ref: NodeRef::default(),
            cursor_ref: Cell::new(NodeRef::default()),
            lines: //"**aa**"
                "T_a_ **a** his: _is some pretty ừn ự đ ở **Markdown**_ **xD**\nThis: _is some pretty ừn ự đ ở **Markdown**_ **xD\nnew** line go *brr* `idk what I am doing`\n\n\nnew paragr🌷🎁💩😜👍🏳️‍🌈ap

| Hello | xD |
| ----- | -- |
| test  | 1  |

| | hi |

h\nThissiaodajdnkajbdsklajbdkajbdkjlasbdlkjabdwhpdajnlvnoampmönöaiofoaödnlaksdjpaokdjwoaudlsdoahdkjdbjakldb\n\n\n\nadasd asdad asdwuh asdjh aksjd ajdh lkndjadno aodhoa a aodha aodhadawo waaodsjhda kjsdh alsd asdjh alsdk jasd asd skj d akjsdh a"
                .repeat(10)
                .lines()
                .map(|s| s.into())
                .collect(),
                command: TextLine::default(),
            mode: Mode::Normal,
            font: "mononoki".to_string(), 
        };
        s.parse_md();
        s
    }

    fn update(&mut self, ctx: &Context<Self>, msgs: Self::Message) -> bool {
        let mut ret = false;
        for msg in msgs {
            match msg {
                Msg::CursorMove(x, y) => {
                    let last = self.cursor_position;
                    self.cursor_position.1 = ((self.cursor_position.1 as i32 + y).max(0) as usize)
                        .min(self.lines.len() - 1);
                    if x != 0 {
                        let max_x = if self.mode == Mode::Insert {
                            self.lines[self.cursor_position.1].len()
                        } else {
                            self.lines[self.cursor_position.1].len() - 1
                        };
                        self.cursor_position.0 = ((self.cursor_position.0.min(max_x) as i32 + x)
                            .max(0) as usize)
                            .min(max_x);
                    }
                    // TODO This could be more precise
                    ret |= last != self.cursor_position;
                }
                Msg::Write(text) if self.mode.is_command() => {
                    let (cursor_movement, lines) = self
                        .command
                        .insert(self.cursor_position.0.min(self.command.len()), &text);
                    assert!(lines.is_empty());
                    // Maybe optimized
                    // for line in new_lines.into_iter().rev() {
                    //     self.lines.insert(self.cursor_position.1 + 1, line);
                    // }
                    self.update(ctx, cursor_movement);
                    // self.cursor_position.0 += text.graphemes(true).count();
                    self.parse_md();
                    ret = true;
                }
                Msg::Write(text) => {
                    let line = &mut self.lines[self.cursor_position.1];
                    let (cursor_movement, new_lines) =
                        line.insert(self.cursor_position.0.min(line.len()), &text);
                    // Maybe optimized
                    for line in new_lines.into_iter().rev() {
                        self.lines.insert(self.cursor_position.1 + 1, line);
                    }
                    self.update(ctx, cursor_movement);
                    // self.cursor_position.0 += text.graphemes(true).count();
                    self.parse_md();
                    ret = true;
                }
                Msg::Mode(mode) => {
                    if mode != self.mode {
                        if self.mode == Mode::Insert {
                            self.cursor_position.0 = self
                                .cursor_position
                                .0
                                .min(self.lines[self.cursor_position.1].len() - 1);
                        }
                        self.mode = mode;
                        ret = true;
                    }
                }
                Msg::CursorPos(x, y) => {
                    if let Some(x) = x {
                        self.cursor_position.0 = x;
                    }
                    if let Some(y) = y {
                        self.cursor_position.1 = y;
                    }
                }
                Msg::ExecuteCommand => {
                    self.execute(self.command.to_string());
                    self.command.clear();
                    ret = true
                }
            }
        }
        ret
    }

    fn rendered(&mut self, _: &Context<Self>, first_render: bool) {
        // focus the text at page load to be able to accept keyboard input
        if first_render {
            let smth = self.node_ref.cast::<HtmlInputElement>().unwrap();
            smth.focus().unwrap();
        }

        // scroll to cursor if out of view
        // TODO add support for cursor_margins
        // behavior: 'smooth'
        if let Some(elem) = self.cursor_ref.take().cast::<web_sys::Element>() {
            let window = window().unwrap();
            let bounds = elem.get_bounding_client_rect();
            if bounds.y() < 0. {
                let y = window.scroll_y().unwrap() + bounds.y();
                window.scroll_to_with_x_and_y(0., y);
            } else if bounds.bottom() > window.inner_height().unwrap().as_f64().unwrap() {
                let y = window.scroll_y().unwrap() + bounds.bottom()
                    - window.inner_height().unwrap().as_f64().unwrap();
                window.scroll_to_with_x_and_y(0., y);
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let mode = self.mode;
        let keypress = ctx
            .link()
            .batch_callback(move |e| Self::handle_key_press(e, mode));

        let cursor_ref = NodeRef::default();
        self.cursor_ref.set(cursor_ref.clone());

        let document = Document {
            elements: vec![Element::Table(Table {
                cells: HashMap::from_iter(
                    vec![(
                        (0, 0),
                        TableCell {
                            content: Paragraph {
                                text: vec!["H".to_owned()],
                                cursor: Some(0),
                            },
                        },
                    )]
                    .into_iter(),
                ),
                active_cell: (0, 0),
            })],
            active_element: 0,
        };
        html! {
            <div class={classes!("dark")} style={format!("font-family: {}, Hack, Noto, monospace; font-size: 20px; line-height: 30px", self.font)}>
                <div ref={self.node_ref.clone()} style="min-height:100vh" class={classes!("bg-gray-200", "text-gray-800", "dark:bg-gray-900", "dark:text-gray-300", "wrap", "p-2")} onkeydown={keypress} tabindex="0">
                        <div class={classes!("fixed", "flex", "items-center", "justify-center", "h-1/3", "w-screen")}>
                            <div class={classes!("w-10/12", "object-center", "bg-gray-700", "rounded", "ring-2", "ring-gray-400", "p-2",(self.mode != Mode::Command).then(|| "hidden"))}>

                                <Line line={self.command.characters.clone()} cursor={(self.mode == Mode::Command).then(|| (self.cursor_position.0, CursorStyle::Insert, cursor_ref.clone()))}>
                                    <span class={classes!("font-bold")}>
                                        {":"}
                                    </span>
                                </Line>
                            </div>
                        </div>
                        {document.render()}
                    // <div style="height:0" class={classes!("text-transparent")}>
                    //     {for self.lines.iter().map(|line| html!{
                    //         <Line key={line.key.to_string()} line={line.characters.clone()} background=true cursor={None}/>
                    //     })}
                    // </div>
                    // <div>
                    //     {for self.lines.iter().enumerate().map(|(i, line)| html!{
                    //         <Line key={line.key.to_string()} line={line.characters.clone()} cursor = {(i == self.cursor_position.1 && self.mode != Mode::Command)
                    //             .then(|| (self.cursor_position.0.min(if self.mode == Mode::Insert {
                    //                 line.len()
                    //             } else {
                    //                 line.len().max(1) - 1
                    //             }),if self.mode == Mode::Insert {
                    //                 CursorStyle::Insert
                    //             }else{
                    //                 CursorStyle::Box
                    //             },cursor_ref.clone()))}
                    //         />
                    //     })}
                    // </div>
                </div>
            </div>
        }
    }
}

#[derive(Properties, Clone, PartialEq, Debug)]
struct LineProps {
    line: Vec<(String, HashSet<TextStyle>, usize)>,
    #[prop_or_default]
    cursor: Option<(usize, CursorStyle, NodeRef)>,
    #[prop_or_default]
    background: bool,
    #[prop_or_default]
    children: Children,
}

struct Line(LineProps);

impl Component for Line {
    type Message = ();
    type Properties = LineProps;

    fn create(ctx: &yew::Context<Line>) -> Self {
        Self(ctx.props().to_owned())
    }

    fn changed(&mut self, ctx: &Context<Self>) -> bool {
        // TODO
        if ctx.props() != &self.0 {
            self.0 = ctx.props().to_owned();
            true
        } else {
            false
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let mut spans = vec![];
        let props = ctx.props();

        if props.background {
            let mut was_style: HashSet<TextStyle> = HashSet::new();
            let mut peekable_line = props.line.iter().peekable();
            while let (Some((character, style, _)), will_style) = (
                peekable_line.next(),
                peekable_line
                    .peek()
                    .map(|(_, style, _)| style.clone())
                    .unwrap_or_default(),
            ) {
                let classes: Classes = style
                    .iter()
                    .copied()
                    .flat_map(|style| {
                        style.background_classes(style.positioned(&was_style, &will_style))
                    })
                    .collect();
                was_style = style.clone();

                spans.push(html! {
                    <span class={classes}>{character}</span>
                });
            }
        } else {
            let mut was_style: HashSet<TextStyle> = HashSet::new();
            let mut peekable_line = props.line.iter().enumerate().peekable();
            while let (Some((i, (character, style, _))), will_style) = (
                peekable_line.next(),
                peekable_line
                    .peek()
                    .map(|(_, (_, style, _))| style.clone())
                    .unwrap_or_default(),
            ) {
                let classes: Classes = style
                    .iter()
                    .copied()
                    .chain(props.cursor.iter().find_map(|x| {
                        if x.0 == i {
                            Some(TextStyle::Cursor(x.1))
                        } else {
                            None
                        }
                    }))
                    .flat_map(|style| {
                        style.forground_classes(style.positioned(&was_style, &will_style))
                    })
                    .collect();
                was_style = style.clone();
                spans.push(html! {
                    if props.cursor.is_some() && props.cursor.as_ref().unwrap().0 == i {
                        <span ref={props.cursor.iter().cloned().next().unwrap().2} class={classes}>{character}</span>
                    } else {
                        <span class={classes!(classes)}>{character}</span>
                    }
                });
            }
        }
        if props
            .cursor
            .as_ref()
            .map(|c| c.0 >= props.line.len())
            .unwrap_or_default()
            && !props.background
        {
            spans.push(html! {
                <span ref={props.cursor.iter().cloned().next().unwrap().2} class={classes!(TextStyle::Cursor(props.cursor.as_ref().unwrap().1).forground_classes(Position::Single))}>{" "}</span>
            });
        }

        html! {
            <p>
                {props.children.clone()}
                {for spans}
                <span>{" "}</span>
            </p>
        }
    }
}

#[derive(Properties, Clone, PartialEq)]
struct CursorProps {
    x: usize,
    y: usize,
    style: CursorStyle,
    lines: Vec<NodeRef>,
    text: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
enum CursorStyle {
    #[default]
    Box,
    #[allow(dead_code)]
    EmtyBox,
    Insert,
}

impl CursorStyle {
    fn classes(&self) -> Classes {
        match self {
            CursorStyle::Box => classes!["bg-red-300", "text-gray-900", "rounded"],
            CursorStyle::EmtyBox => classes![
                "border-red-300",
                "text-transparent",
                "bg-transparent",
                "border-2",
                "rounded",
            ],
            CursorStyle::Insert => classes!["cursor-line"],
        }
    }
}

fn main() {
    yew::start_app::<Model>();
}
