#![feature(derive_default_enum)]
use std::{
    collections::HashSet,
    fmt::format,
    iter::{self, FromIterator},
    ops::{Deref, DerefMut},
};

use gloo_console::log;
use pulldown_cmark::{Parser, Tag};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;
use uuid::Uuid;
use web_sys::{window, Element, HtmlInputElement, Node};
use yew::prelude::*;

#[derive(PartialEq, Eq, Clone, Copy)]
enum Mode {
    Insert,
    Normal,
}

enum Msg {
    CursorMove(i32, i32),
    CursorPos(Option<usize>, Option<usize>),
    Write(String),
    Mode(Mode),
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

struct KeyRef<'a> {
    key: &'a str,
    alt: bool,
    ctrl: bool,
    shift: bool,
}

impl PartialEq<&str> for KeyRef<'_> {
    fn eq(&self, other: &&str) -> bool {
        self.key == *other && !self.alt && !self.ctrl && !self.shift
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
}

impl TextStyle {
    fn forground_classes(self) -> &'static [&'static str] {
        match self {
            TextStyle::Italic => &["italic"],
            TextStyle::Bold => &["font-bold"],
            TextStyle::Cursor(cursor_style) => cursor_style.classes(), //&["text-gray-900", "rounded", "bg-red-300"],
            _ => &[],
        }
    }
    fn background_classes(self) -> &'static [&'static str] {
        match self {
            // These are mirrored to help with non monospace spacing
            TextStyle::Italic => &["italic"],
            TextStyle::Bold => &["font-bold"],
            TextStyle::Code => &["bg-gray-600"],
            _ => &[],
        }
    }
}

struct TextLine {
    // content: String,
    key: uuid::Uuid,
    characters: Vec<(String, HashSet<TextStyle>, usize)>,
}

impl TextLine {
    fn len(&self) -> usize {
        self.characters.len()
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
    // highlighting: Vec<(TextStyle, Range<usize>)>,
    // lines: Vec<(String, usize, NodeRef, Vec<(TextStyle, Range<usize>)>)>,
    lines: Vec<TextLine>,
    mode: Mode,
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
                    // blacklist correctly handled keys
                    key if ["Shift"].contains(&key.key) => return None,
                    key if key == "Backspace" => todo!(),
                    KeyRef {
                        key,
                        ctrl: false,
                        alt: false,
                        ..
                    } if mode == Mode::Insert => vec![Msg::Write(key.to_owned())],
                    a => {
                        log!("Unknown keypress", a.key);
                        return None;
                    }
                },
                Mode::Normal => match key.as_ref() {
                    key if key == "i" => vec![Msg::Mode(Mode::Insert)],
                    key if key == "h" => vec![Msg::CursorMove(-1, 0)],
                    key if key == "j" => vec![Msg::CursorMove(0, 1)],
                    key if key == "k" => vec![Msg::CursorMove(0, -1)],
                    key if key == "l" => vec![Msg::CursorMove(1, 0)],
                    a => {
                        log!("Unknown keypress", a.key);
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
        let parser = Parser::new(text);

        // let mut highlights: HashSet<TextStyle> = HashSet::new();

        let highlighting: Vec<_> = parser
            .into_offset_iter()
            .filter_map(|(elem, range)| {
                use pulldown_cmark::Event;
                Some({
                    let a = (
                        match elem {
                            Event::Start(Tag::Emphasis) => {
                                TextStyle::Italic
                                // highlights.insert(TextStyle::Italic);
                                // true
                            }
                            Event::Start(Tag::Strong) => {
                                TextStyle::Bold
                                // highlights.insert(TextStyle::Bold);
                                // true
                            }

                            // Event::End(Tag::Emphasis) => {
                            //     highlights.remove(&TextStyle::Italic);
                            //     false
                            // }
                            // Event::End(Tag::Strong) => {
                            //     highlights.remove(&TextStyle::Bold);
                            //     false
                            // }
                            Event::Code(_) => TextStyle::Code,
                            // return Some((HashSet::from([TextStyle::Code]), range)),
                            _ => return None,
                        },
                        range,
                    );
                    log!(format!("{:?}", a));
                    a
                })
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
}

impl Component for Model {
    type Message = Vec<Msg>;
    type Properties = ();

    fn create(_props: &yew::Context<Model>) -> Self {
        let mut s = Self {
            cursor_position: (0, 0),
            node_ref: NodeRef::default(),
            lines: //"**aa**"
                "T_a_ **a** his: _is some pretty ừn ự đ ở **Markdown**_ **xD**\nThis: _is some pretty ừn ự đ ở **Markdown**_ **xD\nnew** line go *brr* `idk what I am doing`\n\n\nnew paragr🌷🎁💩😜👍🏳️‍🌈aph\nThissiaodajdnkajbdsklajbdkajbdkjlasbdlkjabdwhpdajnlvnoampmönöaiofoaödnlaksdjpaokdjwoaudlsdoahdkjdbjakldb\n\n\n\nadasd asdad asdwuh asdjh aksjd ajdh lkndjadno aodhoa a aodha aodhadawo waaodsjhda kjsdh alsd asdjh alsdk jasd asd skj d akjsdh a"
                .repeat(10)
                .lines()
                .map(|s| s.into())
                .collect(),
            mode: Mode::Normal,
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
                Msg::Write(text) => {
                    let line = &mut self.lines[self.cursor_position.1];
                    let (cursor_movement, new_lines) =
                        line.insert(self.cursor_position.0.min(line.len()), &text);
                    // Maybe optimized
                    let window = window().unwrap();
                    let performance = window.performance().unwrap();
                    let time = performance.now();
                    for line in new_lines.into_iter().rev() {
                        self.lines.insert(self.cursor_position.1 + 1, line);
                    }
                    let time = performance.now() - time;
                    log!(format!("{:?}", time));
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
            }
        }
        ret
    }

    fn changed(&mut self, _props: &yew::Context<Model>) -> bool {
        // Should only return "true" if new properties are different to
        // previously received properties.
        // This component has no properties so we will always return "false".
        false
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            let smth = self.node_ref.cast::<HtmlInputElement>().unwrap();
            smth.focus().unwrap();
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let mode = self.mode;
        let keypress = ctx
            .link()
            .batch_callback(move |e| Self::handle_key_press(e, mode));

        // let
        html! {
            <div class={classes!("dark")} style="font-family: Hack, monospace; font-size: 20px; line-height: 30px" >
                <div ref={self.node_ref.clone()} style="min-height:100vh" class={classes!("bg-gray-200", "text-gray-800", "dark:bg-gray-900", "dark:text-gray-300", "wrap")} onkeydown={keypress} /*onfocus={self.link.callback(|_| Msg::Update)}*/ tabindex="0">
                    <div style="height:0" class={classes!("text-transparent")}>
                        {for self.lines.iter().map(|line| html!{
                            <Line key={line.key.to_string()} line={line.characters.clone()} background=true cursor={None}/>
                        })}
                    </div>
                    <div>
                        {for self.lines.iter().enumerate().map(|(i, line)| html!{
                            <Line key={line.key.to_string()} line={line.characters.clone()} cursor = {(i == self.cursor_position.1)
                                .then(|| (self.cursor_position.0.min(if self.mode == Mode::Insert {
                                    line.len()
                                } else {
                                    line.len().max(1) - 1
                                }),if self.mode == Mode::Insert {
                                    CursorStyle::Insert
                                }else{
                                    CursorStyle::Box
                                }))}
                            />
                        })}
                    </div>
                </div>
            </div>
        }
    }
}

#[derive(Properties, Clone, PartialEq, Debug)]
struct LineProps {
    line: Vec<(String, HashSet<TextStyle>, usize)>,
    #[prop_or_default]
    cursor: Option<(usize, CursorStyle)>,
    #[prop_or_default]
    background: bool,
}

struct Line(LineProps);

impl Component for Line {
    type Message = ();
    type Properties = LineProps;

    fn create(ctx: &yew::Context<Line>) -> Self {
        Self(ctx.props().to_owned())
    }

    fn changed(&mut self, ctx: &Context<Self>) -> bool {
        // ctx.props() != ctx.
        // TODO
        if ctx.props() != &self.0 {
            // log!(format!("{:?}", self.0));
            // log!(format!("{:?}", ctx.props()));
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
            let mut was_code = false;
            let mut peekable_line = props.line.iter().peekable();
            while let (Some((character, style, _)), will_code) = (
                peekable_line.next(),
                peekable_line
                    .peek()
                    .map(|(_, style, _)| style.contains(&TextStyle::Code))
                    .unwrap_or_default(),
            ) {
                let mut classes: Vec<_> = style
                    .iter()
                    .copied()
                    .map(TextStyle::background_classes)
                    .flatten()
                    .copied()
                    .collect();
                if style.contains(&TextStyle::Code) {
                    if !will_code {
                        classes.push("rounded-r")
                    }
                    if !was_code {
                        classes.push("rounded-l")
                    }
                    was_code = true
                } else {
                    was_code = false
                }

                spans.push(html! {
                    <span class={classes!(classes)}>{character}</span>
                });
            }
        } else {
            for (i, (character, style, _)) in props.line.iter().enumerate() {
                let classes: Vec<_> = style
                    .iter()
                    .copied()
                    .chain(props.cursor.iter().find_map(|&x| {
                        if x.0 == i {
                            Some(TextStyle::Cursor(x.1))
                        } else {
                            None
                        }
                    }))
                    .map(TextStyle::forground_classes)
                    .flatten()
                    .copied()
                    .collect();
                spans.push(html! {
                    <span class={classes!(classes)}>{character}</span>
                });
            }
        }
        if props
            .cursor
            .map(|c| c.0 == props.line.len())
            .unwrap_or_default()
            && !props.background
        {
            spans.push(html! {
                <span class={classes!(TextStyle::Cursor(props.cursor.unwrap().1).forground_classes())}>{" "}</span>
            });
        }

        html! {
            <p>
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
    EmtyBox,
    Insert,
}

impl CursorStyle {
    fn classes(&self) -> &'static [&'static str] {
        match self {
            CursorStyle::Box => &["bg-red-300", "text-gray-900", "rounded"],
            CursorStyle::EmtyBox => &[
                "border-red-300",
                "text-transparent",
                "bg-transparent",
                "border-2",
                "rounded",
            ],
            CursorStyle::Insert => &["cursor-line"],
        }
    }
}

fn main() {
    yew::start_app::<Model>();
}
