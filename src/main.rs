use std::{
    collections::HashSet,
    iter,
    ops::{BitOr, Range},
};

use gloo_console::log;
use pulldown_cmark::{Parser, Tag};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;
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
    fn forground_classes(&self) -> &'static [&'static str] {
        match self {
            TextStyle::Italic => &["italic"],
            TextStyle::Bold => &["font-bold"],
            TextStyle::Cursor => &["text-gray-900", "rounded", "bg-red-300"],
            _ => &[],
        }
    }
    fn background_classes(&self) -> &'static [&'static str] {
        match self {
            // These are mirrored to help with non monospace spacing
            TextStyle::Italic => &["italic"],
            TextStyle::Bold => &["font-bold"],
            TextStyle::Code => &["bg-gray-600"],
            _ => &[],
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
    highlighting: Vec<(TextStyle, Range<usize>)>,
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
                log!("Unknown keypress", a.key);
                return None;
            }
        })
    }
    fn parse_md(&mut self) {
        let parser = Parser::new(&self.text);

        // let mut highlights: HashSet<TextStyle> = HashSet::new();

        self.highlighting = parser
            .into_offset_iter()
            .filter_map(|(elem, range)| {
                use pulldown_cmark::Event;
                Some((
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
                ))
                // {
                //     Some((highlights.clone(), range))
                // } else {
                //     None
                // }
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
                line_lengths.push(UnicodeSegmentation::graphemes(l, true).count());
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
                "This: _is some pretty ừn ự đ ở **Markdown**_ **xD\nnew** line go *brr* `idk what I am doing`\n\n\nnew paragr🌷🎁💩😜👍🏳️‍🌈aph",
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
                    .min(self.line_lengths[self.cursor_position.1]);
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
                    <div class=classes!("absolute", "text-transparent") id="body">
                        {for self.lines.iter().enumerate().map(|(i,(line,offset, node_ref))| html!{
                            <Line line=line.clone() offset=*offset ref=node_ref.clone() highlighting=self.highlighting.clone() background=true cursor=None/>
                        })}
                    </div>
                    <div class=classes!("absolute", "z-10") id="body">
                        {for self.lines.iter().enumerate().map(|(i,(line,offset, node_ref))| html!{
                            <Line line=line.clone()+" " offset=*offset ref=node_ref.clone() highlighting=self.highlighting.clone() cursor=(i==self.cursor_position.1).then(|| self.cursor_position.0)/>
                        })}
                    </div>
                    // <Cursor x={self.cursor_position.0} y={self.cursor_position.1} style=CursorStyle::Box lines=self.line_refs.clone() text=self.text.lines().map(String::from).collect::<Vec<_>>()/>
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
    highlighting: Vec<(TextStyle, Range<usize>)>,
    offset: usize,
    #[prop_or_default]
    cursor: Option<usize>,
    #[prop_or_default]
    background: bool,
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

    fn change(&mut self, props: Self::Properties) -> ShouldRender {
        if props != self.0 {
            self.0 = props;
            true
        } else {
            false
        }
    }

    fn view(&self) -> Html {
        let mut spans = vec![];
        // let mut his = self
        //     .0
        //     .highlighting
        //     .iter()
        //     .filter_map(|(h, r)| {
        //         if r.end > self.0.offset && r.start < self.0.offset + self.0.line.len() {
        //             let mut r = r.clone();
        //             r.end = (r.end as i32 - self.0.offset as i32).min(self.0.line.len() as i32)
        //                 as usize;
        //             r.start = (r.start as i32 - self.0.offset as i32).max(0) as usize;
        //             Some((h, r))
        //         } else {
        //             None
        //         }
        //     })
        //     .peekable();
        // if his.peek().is_none() {
        //     spans.push(html!(<span>{&self.0.line}</span>));
        // }
        // /

        let mut idk = self
            .0
            .line
            .grapheme_indices(true)
            .enumerate()
            .map(|(idx, (grapheme_offset, grapheme))| {
                let classes: HashSet<_> = self
                    .0
                    .highlighting
                    .iter()
                    .filter_map(|(hi, range)| {
                        if range.start <= grapheme_offset + self.0.offset
                            && range.end > grapheme_offset + self.0.offset
                        {
                            Some(hi)
                        } else {
                            None
                        }
                    })
                    .copied()
                    .chain((Some(idx) == self.0.cursor).then(|| TextStyle::Cursor))
                    // .flatten()
                    .collect();
                (classes, grapheme)
            })
            .peekable();

        if self.0.background {
            let mut was_code = false;
            while let (Some((highlights, grapheme)), will_code) = (
                idk.next(),
                idk.peek()
                    .map(|(highlights, _)| highlights.contains(&TextStyle::Code))
                    .unwrap_or_default(),
            ) {
                let mut classes: Vec<_> = highlights
                    .iter()
                    .map(TextStyle::background_classes)
                    .flatten()
                    .copied()
                    .collect();
                if !will_code {
                    classes.push("rounded-r")
                }
                if !was_code {
                    classes.push("rounded-l")
                }
                was_code = highlights.contains(&TextStyle::Code);

                spans.push(html! {
                    <span class=classes!(classes)>{grapheme}</span>
                });
            }
        } else {
            for (highlights, grapheme) in idk {
                let mut classes: Vec<_> = highlights
                    .iter()
                    .map(TextStyle::forground_classes)
                    .flatten()
                    .copied()
                    .collect();

                spans.push(html! {
                    <span class=classes!(classes)>{grapheme}</span>
                });
            }
        }

        // while let (Some((hi, range)), peek) = (his.next(), his.peek()) {
        //     if range.start > shown_idx {
        //         spans.push(html!(<span>{&self.0.line[shown_idx..range.start]}</span>));
        //     }
        //     let end = if let Some((_, peek_range)) = peek {
        //         peek_range.start.min(range.end)
        //     } else {
        //         range.end
        //     };
        //     spans.push(html! {
        //             <span class=classes!(hi.iter().map(TextStyle::as_class).flatten().copied().collect::<Vec<&str>>())>
        //                 {&self.0.line[range.start..end]}
        //             </span>
        //         });
        //     shown_idx = end;

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
        let line = self.props.lines[self.props.y].cast::<Element>().unwrap();
        let text_nodes = line.child_nodes();
        if text_nodes.get(0).is_some() {
            let range: web_sys::Range = document().create_range().unwrap();
            let text = &self.props.text[self.props.y];
            // let body = body.child_nodes().item(0).unwrap();
            let mut rect = None;

            let mut graphemes = UnicodeSegmentation::graphemes(text.as_str(), true)
                .chain(iter::once(" "))
                .take(self.props.x + 1)
                .peekable();

            let mut text_node_idx = 0;
            let mut inner_idx = 0;

            log!(self.props.x);
            while let Some(&grapheme) = graphemes.peek() {
                log!(grapheme, grapheme.len());
                let text_node: Node = text_nodes.get(text_node_idx).unwrap();
                let text_node = text_node.child_nodes().item(0).unwrap();

                let grapheme_width = grapheme.width();
                let grapheme_width = if grapheme_width == 3 {
                    6
                } else {
                    grapheme_width
                };

                if range.set_start(&text_node, inner_idx as u32).is_ok()
                    && range
                        .set_end(&text_node, (inner_idx + grapheme_width) as u32)
                        .is_ok()
                {
                    rect = Some(range.get_bounding_client_rect());
                    inner_idx += grapheme_width;
                    graphemes.next();
                } else {
                    text_node_idx += 1;
                    inner_idx = 0;
                }
            }

            // for i in 0..text_nodes.length() {
            //     let text_node: Node = text_nodes.get(i).unwrap();
            //     let text_node = text_node.child_nodes().item(0).unwrap();
            //     loop {
            //         if range.set_start(&text_node, x as u32).is_ok()
            //             && range.set_end(&text_node, x as u32 + 1).is_ok()
            //         {
            //             rect = Some(range.get_bounding_client_rect());
            //             content = Some(range.clone_contents().unwrap().text_content().unwrap());
            //             break;
            //         }
            //     }
            //     x -= UnicodeSegmentation::graphemes(text_node.text_content().unwrap().as_str(), true)
            //         .count();
            // }
            let rect = rect.unwrap();
            // let content = content.unwrap();
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
