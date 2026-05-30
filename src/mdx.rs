use leptos::{
    component, IntoView,
    prelude::*
};
use leptos::attr::custom::custom_attribute;
use leptos::attr::any_attribute::IntoAnyAttribute;
use regex::Regex;
use std::collections::HashMap;
use tl::{HTMLTag, Node};

use crate::markdown::parse;

#[component]
/// Renders a markdown source into a Leptos component.
/// Custom components can be used in the markdown source.
pub fn Mdx(source: String, components: Components) -> impl IntoView {
    let (_fm, html) = parse(&source).expect("invalid mdx");
    // TODO: we could expose frontmatter in the context so components can use its value

    let dom = tl::parse(&html, tl::ParserOptions::default()).expect("invalid html");

    let mut root_views = vec![];
    for node_handle in dom.children() {
        let node = node_handle.get(dom.parser()).expect("not a node");
        root_views.push(process_element(node, dom.parser(), &components, true));
    }

    AnyView::from(Fragment::new(root_views))
}

/// Props passed to a custom component.
pub struct MdxComponentProps {
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub attributes: HashMap<String, Option<String>>,
    pub children: Children,
}

/// A collection of custom components.
pub struct Components {
    components: HashMap<String, Box<dyn Fn(MdxComponentProps) -> AnyView>>,
}

impl Components {
    pub fn new() -> Self {
        Self {
            components: HashMap::new(),
        }
    }

    /// Register a new custom component that won't receive any props.
    pub fn add<F, IV>(&mut self, name: String, component: F)
    where
        F: Fn() -> IV + 'static,
        IV: IntoView + 'static,
    {
        self.components
            .insert(name, Box::new(move |_| component().into_any()));
    }

    /// Register a new custom component that will receive props. The standardized
    /// MdxComponentsProps are converted to the props type of the component using the provided
    /// adapter.
    pub fn add_props<F, IV, Props, PropsFn>(
        &mut self,
        name: String,
        component: F,
        props_adapter: PropsFn,
    ) where
        F: Fn(Props) -> IV + 'static,
        IV: IntoView + 'static,
        PropsFn: Fn(MdxComponentProps) -> Props + 'static,
    {
        self.components.insert(
            name,
            Box::new(move |props| component(props_adapter(props)).into_any()),
        );
    }

    fn get(&self, name: &str) -> Option<&Box<dyn Fn(MdxComponentProps) -> AnyView>> {
        self.components.get(name)
    }
}

pub fn process_element(
    el: &Node,
    parser: &tl::Parser,
    components: &Components,
    parse_new_lines: bool,
) -> AnyView {
    match el {
        Node::Comment(_comment) => return ().into_any(),
        Node::Raw(raw) => {
            let text = String::from_utf8(raw.as_bytes().to_vec());

            if let Ok(t) = text {
                if parse_new_lines {
                    /*
                     * Replace new lines with <br /> only if they are preceded and followed by text.
                     * to avoid adding <br /> to empty lines.
                     */
                    let reg = Regex::new(r"(.+)\n(.+)").unwrap();

                    let t = reg.replace_all(&t, |caps: &regex::Captures| {
                        format!("{} <br /> {}", &caps[1], &caps[2])
                    }).to_string();

                    return t.into_any();
                }

                return t.into_any();
            } else {
                println!("error parsing raw text: {:?}", text);
                return ().into_any();
            }
        }
        Node::Tag(tag) => {
            let mut child_views = vec![];

            let nodes = tag.children();

            // Process children
            nodes.top().iter().for_each(|node_handle| {
                let node = node_handle.get(parser).expect("not a node");

                /*
                 * Inside code blocks we want to keep the new lines as they are.
                 */
                if tag.name().as_utf8_str() == "code" || tag.name().as_utf8_str() == "pre" {
                    child_views.push(process_element(node, parser, components, false));
                } else {
                    child_views.push(process_element(node, parser, components, parse_new_lines));
                }
            });

            let name_ref = tag.name().as_utf8_str();
            let name = name_ref.as_ref();

            // Custom elements
            if let Some(component) = components.get(name) {
                let attributes = tag.attributes();

                let classes = attributes.class_iter().map_or(Vec::new(), |class_list| {
                    class_list.map(|c| c.to_string()).collect()
                });

                let attributes_map = attributes
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.map(|v| v.to_string())))
                    .collect();

                return component(MdxComponentProps {
                    id: attributes.id().map(|id| id.as_utf8_str().to_string()),
                    classes,
                    attributes: attributes_map,
                    children: Box::new(move || AnyView::from(Fragment::new(child_views))),
                });
            }

            // HTML elements
            html_element(&tag.clone(), child_views)
        }
    }
}

fn html_element(element: &HTMLTag, children: Vec<AnyView>) -> AnyView {
    let attributes = element.attributes();

    let mut attrs = Vec::new();

    let classes: Vec<String> = attributes.class_iter().map_or(Vec::new(), |class_list| {
        class_list.map(|c| c.to_string()).collect()
    });

    for (k, v) in attributes.iter() {
        let key = k.to_string();
        if key == "class" {
            continue;
        }
        if let Some(v) = v {
            attrs.push(custom_attribute(key, v.to_string()).into_any_attr());
        } else {
            attrs.push(custom_attribute(key, true).into_any_attr());
        }
    }

    if !classes.is_empty() {
        attrs.push(custom_attribute("class", classes.join(" ")).into_any_attr());
    }

    let tag_name = element.name().as_utf8_str().to_string();
    let fragment = AnyView::from(Fragment::new(children));

    leptos::html::custom(tag_name)
        .add_any_attr(attrs)
        .child(fragment)
        .into_any()
}
