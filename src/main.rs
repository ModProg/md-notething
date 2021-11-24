use std::{
    collections::HashSet,
    ops::{BitOr, Range},
};

use gloo_console::log;
use pulldown_cmark::{Parser, Tag};
use web_sys::{Element, HtmlInputElement, Node};
use yew::{prelude::*, utils::document};

enum Msg {
    CursorMove(i32, i32),
}

struct Keypress {
    key: String,
    alt: bool,
    ctrl: bool,
    shift: bool,
}

impl From<KeyboardEvent> for Keypress {
    fn from(ke: web_sys::KeyboardEvent) -> Self {
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
    Cursor,
}

impl TextStyle {
    fn as_class(&self) -> &'static [&'static str] {
        match self {
            TextStyle::Italic => &["italic"],
            TextStyle::Bold => &["font-bold"],
            TextStyle::Code => &["bg-gray-600", "rounded"],
            TextStyle::Cursor => todo!(),
        }
    }
}

struct Model {
    // `ComponentLink` is like a reference to a component.
    // It can be used to send messages to the component
    link: ComponentLink<Self>,
    text: String,
    cursor_position: (usize, usize, usize),
    node_ref: NodeRef,
    line_lengths: Vec<usize>,
    line_refs: Vec<NodeRef>,
    highlighting: Vec<(HashSet<TextStyle>, Range<usize>)>,
    lines: Vec<(String, usize, NodeRef)>,
}

impl Model {
    fn handle_key(event: KeyboardEvent) -> Option<<Model as Component>::Message> {
        Some(match Keypress::from(event).as_ref() {
            key if key == "h" => Msg::CursorMove(-1, 0),
            key if key == "j" => Msg::CursorMove(0, 1),
            key if key == "k" => Msg::CursorMove(0, -1),
            key if key == "l" => Msg::CursorMove(1, 0),
            a => {
                log!(a.key);
                return None;
            }
        })
    }
    fn parse_md(&mut self) {
        let parser = Parser::new(&self.text);

        let mut highlights: HashSet<TextStyle> = HashSet::new();

        self.highlighting = parser
            .into_offset_iter()
            .filter_map(|(elem, range)| {
                use pulldown_cmark::Event;
                if match elem {
                    Event::Start(Tag::Emphasis) => {
                        highlights.insert(TextStyle::Italic);
                        true
                    }
                    Event::Start(Tag::Strong) => {
                        highlights.insert(TextStyle::Bold);
                        true
                    }

                    Event::End(Tag::Emphasis) => {
                        highlights.remove(&TextStyle::Italic);
                        false
                    }
                    Event::End(Tag::Strong) => {
                        highlights.remove(&TextStyle::Bold);
                        false
                    }

                    Event::Code(_) => return Some((HashSet::from([TextStyle::Code]), range)),
                    _ => false,
                } {
                    Some((highlights.clone(), range))
                } else {
                    None
                }
            })
            .collect();

        for i in 0..self.highlighting.len() - 1 {
            // One highlighting group can only be fully inside another one
            if self.highlighting[i].1.end > self.highlighting[i + 1].1.end {}
        }

        let mut offset = 0;
        let mut line_refs = vec![];
        let mut line_lengths = vec![];
        // TODO normalize input or handle \n\r
        self.lines = self
            .text
            .lines()
            .map(|l| {
                let node_ref = NodeRef::default();
                line_refs.push(node_ref.clone());
                line_lengths.push(l.len());
                let ret = (l.to_owned(), offset, node_ref);
                offset += l.len() + 1;
                ret
            })
            .collect();
        self.line_refs = line_refs;
        self.line_lengths = line_lengths;
    }
}

impl Component for Model {
    type Message = Msg;
    type Properties = ();

    fn create(_props: Self::Properties, link: ComponentLink<Self>) -> Self {
        let mut s = Self {
            link,
            cursor_position: (0, 0, 0),
            text: String::from(
                "This: _is some pretty **Markdown**_ **xD\nnew** line go *brr* `idk what I am doing`\n\n\nnew paragraph",
            ),
            node_ref: NodeRef::default(),
            line_lengths: vec![],
            line_refs: vec![],
            lines: vec![],
            highlighting: vec![],
        };
        s.parse_md();
        s
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            Msg::CursorMove(x, y) => {
                self.cursor_position.1 = ((self.cursor_position.1 as i32 + y).max(0) as usize)
                    .min(self.line_lengths.len() - 1);
                self.cursor_position.0 = ((self.cursor_position.0 as i32 + x).max(0) as usize)
                    .min(self.line_lengths[self.cursor_position.1].max(1) - 1);
                // TODO This could be more precise
                true
            }
        }
    }

    fn change(&mut self, _props: Self::Properties) -> ShouldRender {
        // Should only return "true" if new properties are different to
        // previously received properties.
        // This component has no properties so we will always return "false".
        false
    }

    fn rendered(&mut self, first_render: bool) {
        if first_render {
            let smth = self.node_ref.cast::<HtmlInputElement>().unwrap();
            smth.focus().unwrap();
        }
    }

    fn view(&self) -> Html {
        let keyhandler = self.link.batch_callback(Self::handle_key);

        // let
        html! {
            <div class=classes!("dark") style="font-family: Hack, monospace; font-size: 20px" >
                <div ref=self.node_ref.clone() class=classes!("bg-gray-200", "text-gray-800", "dark:bg-gray-900", "dark:text-gray-300", "h-screen") onkeypress=keyhandler /*onfocus={self.link.callback(|_| Msg::Update)}*/ tabindex="0">
                    <div class=classes!("mix-blend-difference", "absolute", "z-10") id="body">
                        {for self.lines.iter().map(|(line,offset, node_ref)| html!{
                            <Line line=line.clone() offset=*offset ref=node_ref.clone() highlighting=self.highlighting.clone()/>
                        })}
                    </div>
                    <Cursor x={self.cursor_position.0} y={self.cursor_position.1} style=CursorStyle::Box lines=self.line_refs.clone()/>
                    // <p id="body"><span>{"This "}</span> <span class=classes!("italic")>{
                    //     {"_is just a random text_"}
                    // }</span> </p>
                    // <div class=classes!("bg-red-300") style ={
                    //     document().create_range().unwrap();
                    //     ""
                    // }></div>
                </div>
            </div>
        }
    }
}

#[derive(Properties, Clone, PartialEq)]
struct LineProps {
    line: String,
    highlighting: Vec<(HashSet<TextStyle>, Range<usize>)>,
    offset: usize,
}

struct Line(LineProps);

impl Component for Line {
    type Message = ();
    type Properties = LineProps;

    fn create(props: Self::Properties, _link: ComponentLink<Self>) -> Self {
        Self(props)
    }

    fn update(&mut self, _msg: Self::Message) -> ShouldRender {
        false
    }

    fn change(&mut self, _props: Self::Properties) -> ShouldRender {
        // Should only return "true" if new properties are different to
        // previously received properties.
        // This component has no properties so we will always return "false".
        false
    }

    fn view(&self) -> Html {
        let mut spans = vec![];
        if self.0.line.is_empty() {
            spans.push(html!(<span>{" "}</span>));
        } else {
            let mut his = self
                .0
                .highlighting
                .iter()
                .filter_map(|(h, r)| {
                    if r.end > self.0.offset && r.start < self.0.offset + self.0.line.len() {
                        let mut r = r.clone();
                        r.end = (r.end as i32 - self.0.offset as i32).min(self.0.line.len() as i32)
                            as usize;
                        r.start = (r.start as i32 - self.0.offset as i32).max(0) as usize;
                        Some((h, r))
                    } else {
                        None
                    }
                })
                .peekable();
            if his.peek().is_none() {
                spans.push(html!(<span>{&self.0.line}</span>));
            }
            let mut shown_idx = 0;
            while let (Some((hi, range)), peek) = (his.next(), his.peek()) {
                if range.start > shown_idx {
                    spans.push(html!(<span>{&self.0.line[shown_idx..range.start]}</span>));
                }
                let end = if let Some((_, peek_range)) = peek {
                    peek_range.start.min(range.end)
                } else {
                    range.end
                };
                spans.push(html! {
                    <span class=classes!(hi.iter().map(TextStyle::as_class).flatten().copied().collect::<Vec<&str>>())>
                        {&self.0.line[range.start..end]}
                    </span>
                });
                shown_idx = end - 1;
            }
        };
        html! {
            <p>
                {for spans}
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
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CursorStyle {
    Box,
    EmtyBox,
}

struct Cursor {
    props: CursorProps,
    link: ComponentLink<Self>,
}

impl Component for Cursor {
    type Message = ();
    type Properties = CursorProps;

    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
        Self { props, link }
    }

    fn update(&mut self, _msg: Self::Message) -> ShouldRender {
        true
    }

    fn change(&mut self, props: Self::Properties) -> ShouldRender {
        let changed = props != self.props;
        self.props = props;
        changed
    }

    fn rendered(&mut self, first_render: bool) {
        if first_render {
            let cb = self.link.callback(|_| {});
            cb.emit(());
        }
    }

    fn view(&self) -> Html {
        // Do something after the one second timeout is up!
        let range: web_sys::Range = document().create_range().unwrap();
        let line = self.props.lines[self.props.y as usize]
            .cast::<Element>()
            .unwrap();
        // let body = body.child_nodes().item(0).unwrap();
        let mut x = self.props.x;
        let mut rect = None;
        let text_nodes = line.child_nodes();
        let mut class = "";
        let mut content = None;
        for i in 0..text_nodes.length() {
            let text_node: Node = text_nodes.get(i).unwrap();
            let text_node = text_node.child_nodes().item(0).unwrap();
            if range.set_start(&text_node, x as u32).is_ok()
                && range.set_end(&text_node, x as u32 + 1).is_ok()
            {
                rect = Some(range.get_bounding_client_rect());
                let elem = text_node.parent_element().unwrap();
                if elem.class_list().contains("italic") {
                    class = "italic";
                }
                content = Some(range.clone_contents().unwrap().text_content().unwrap());
                break;
            }
            x -= text_node.text_content().unwrap().len();
        }
        if let Some(rect) = rect {
            let content = content.unwrap();
            let classes = match self.props.style {
                CursorStyle::Box => vec!["bg-red-300", "text-gray-900", "rounded"],
                CursorStyle::EmtyBox => vec![
                    "border-red-300",
                    "text-transparent",
                    "bg-transparent",
                    "border-2",
                    "rounded",
                ],
            };
            html! {
                <div class=classes!(classes) style = {format!("position: absolute; width:{}px; height:{}px;left:{}px;top:{}px; padding-", rect.width(), rect.height(), rect.x(), rect.y())}>
                    // <span class = classes!(class) style="line-height: 1.25; display: block;">{content}</span>
                </div>
            }
        } else {
            html! {}
        }
    }
}

fn main() {
    yew::start_app::<Model>();
}
