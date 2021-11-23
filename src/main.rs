use std::{char, convert::TryInto};

use gloo_console::{console, log};
use web_sys::{Node, Range};
use yew::{
    prelude::*,
    utils::{document, window},
};

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
    fn as_ref<'a>(&'a self) -> KeyRef<'a> {
        KeyRef {
            key: &self.key,
            alt: self.alt,
            ctrl: self.ctrl,
            shift: self.shift,
        }
    }
}

struct Model {
    // `ComponentLink` is like a reference to a component.
    // It can be used to send messages to the component
    link: ComponentLink<Self>,
    value: i64,
    cursor_position: (u32, u32),
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
}

impl Component for Model {
    type Message = Msg;
    type Properties = ();

    fn create(_props: Self::Properties, link: ComponentLink<Self>) -> Self {
        Self {
            link,
            value: 0,
            cursor_position: (2, 0),
        }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            Msg::CursorMove(x, y) => {
                self.cursor_position.0 = (self.cursor_position.0 as i32 + x).max(0) as u32;
                self.cursor_position.1 = (self.cursor_position.0 as i32 + y).max(0) as u32;
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

    fn view(&self) -> Html {
        let keyhandler = self.link.batch_callback(Self::handle_key);
        html! {
            <div class=classes!("dark") style="font-family: Hack, monospace; font-size: 50px" >
                <div class=classes!("bg-gray-200", "text-gray-800", "dark:bg-gray-900", "dark:text-gray-300", "h-screen") onkeypress=keyhandler tabindex="0">
                    <Cursor x={self.cursor_position.0} y={self.cursor_position.1} style=CursorStyle::Box/>
                    <p id="body"><span>{"This "}</span> <span class=classes!("italic")>{"is just a random text"}</span> </p>
                    // <div class=classes!("bg-red-300") style ={
                    //     document().create_range().unwrap();
                    //     ""
                    // }></div>
                </div>
            </div>
        }
    }
}

#[derive(Properties, Clone, Copy, PartialEq, Eq)]
struct CursorProps {
    x: u32,
    y: u32,
    style: CursorStyle,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CursorStyle {
    Box,
    EmtyBox,
}

struct Cursor(CursorProps);

impl Component for Cursor {
    type Message = ();
    type Properties = CursorProps;

    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
        Self(props)
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        // match msg {
        //     Msg::AddOne => {
        //         self.value += 1;
        //         // the value has changed so we need to
        //         // re-render for it to appear on the page
        //         true
        //     }
        // }
        false
    }

    fn change(&mut self, props: Self::Properties) -> ShouldRender {
        let changed = props != self.0;
        self.0 = props;
        changed
    }

    fn view(&self) -> Html {
        let range: Range = document().create_range().unwrap();
        let body = document().get_element_by_id("body").unwrap();
        // let body = body.child_nodes().item(0).unwrap();
        let mut x = self.0.x;
        let mut rect = None;
        let text_nodes = body.child_nodes();
        let mut class = "";
        let mut content = None;
        for i in 0..text_nodes.length() {
            let text_node: Node = text_nodes.get(i).unwrap();
            let text_node = text_node.child_nodes().item(0).unwrap();
            if range.set_start(&text_node, x).is_ok() && range.set_end(&text_node, x + 1).is_ok() {
                rect = Some(range.get_bounding_client_rect());
                let elem = text_node.parent_element().unwrap();
                if elem.class_list().contains("italic") {
                    class = "italic";
                }
                content = Some(range.clone_contents().unwrap().text_content().unwrap());
                break;
            }
            x -= text_node.text_content().unwrap().len() as u32;
        }
        let rect = rect.unwrap();
        let content = content.unwrap();
        let classes = match self.0.style {
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
                <span class = classes!(class) style= {format!("margin-top:-{}px;display: block", rect.y())}>{content}</span>
            </div>
        }
    }
}

fn main() {
    yew::start_app::<Model>();
}
