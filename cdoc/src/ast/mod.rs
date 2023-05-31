mod collectors;

mod visitor;

pub use visitor::*;

use crate::notebook::CellOutput;
use crate::parsers::shortcodes::Argument;
use pulldown_cmark::{HeadingLevel, LinkType, Options, Parser};
use serde_json::Value;
use std::collections::HashMap;

/// Inline elements.
#[derive(Clone, Debug)]
pub enum Inline {
    /// Plain text
    Text(String),
    /// Emphasis
    Emphasis(Vec<Inline>),
    /// Strong
    Strong(Vec<Inline>),
    /// Strikethrough
    Strikethrough(Vec<Inline>),
    /// Inline code
    Code(String),
    SoftBreak,
    HardBreak,
    /// Horizontal rule
    Rule,
    /// An inline image (usually originates from a markdown image spec)
    Image(LinkType, String, String, Vec<Inline>),
    /// An inline link (usually originates from a markdown link spec)
    Link(LinkType, String, String, Vec<Inline>),
    /// Unescaped html.
    Html(String),
    /// Math element (may be inline or display)
    /// The trailing space element is necessary due to the way parsing currently works with
    /// pulldown_cmark.
    Math {
        source: String,
        display_block: bool,
        trailing_space: bool,
    },
    Shortcode(Shortcode),
}

fn vec_inline_to_string(vec: &[Inline]) -> String {
    vec.iter().map(|item| item.to_string()).collect()
}

impl ToString for Inline {
    fn to_string(&self) -> String {
        match self {
            Inline::Text(s) => s.clone(),
            Inline::Emphasis(inner) => vec_inline_to_string(inner),
            Inline::Strong(inner) => vec_inline_to_string(inner),
            Inline::Strikethrough(inner) => vec_inline_to_string(inner),
            Inline::Code(s) => s.clone(),
            Inline::SoftBreak => String::default(),
            Inline::HardBreak => String::default(),
            Inline::Rule => String::default(),
            Inline::Html(s) => s.to_string(),
            Inline::Math { source, .. } => source.to_string(),
            Inline::Shortcode(s) => s.to_string(),
            _ => String::default(),
        }
    }
}

impl ToString for Shortcode {
    fn to_string(&self) -> String {
        match self {
            Shortcode::Inline(base) => base.to_string(),
            Shortcode::Block(base, _) => base.to_string(),
        }
    }
}

impl ToString for ShortcodeBase {
    fn to_string(&self) -> String {
        format!("{}#{}", self.name, self.id.clone().unwrap_or_default(),)
    }
}

/// Wraps Vec<Block> for trait implementations
#[derive(Clone, Debug)]
pub struct Ast(pub Vec<Block>);

/// Code cell attributes. Currently limited but may be extended to arbitrary values.
#[derive(Clone, Debug, Default)]
pub struct CodeAttributes {
    /// Can edit cell
    pub(crate) editable: bool,
    /// Cell is folded by default.
    pub(crate) fold: bool,
}

/// Code cell output (currently always from a notebook). These values are provided to the output_*.yml
/// category of built in templates.  
#[derive(Clone, Debug)]
pub enum CodeOutput {
    /// Base 64 encoded image
    Image(String),
    /// Svg source
    Svg(String),
    /// Json encoded as a map
    Json(HashMap<String, Value>),
    /// Literal html
    Html(String),
    /// Javascript source code
    Javascript(String),
}

/// The base ast component. Mostly corresponds to markdown blocks, but certain elements like Math
/// are represented as Inline even in block display mode. The two enums might be combined in the
/// future.
#[derive(Clone, Debug)]
pub enum Block {
    Heading {
        lvl: HeadingLevel,
        id: Option<String>,
        classes: Vec<String>,
        inner: Vec<Inline>,
    },
    Plain(Vec<Inline>),
    Paragraph(Vec<Inline>),
    BlockQuote(Vec<Inline>),
    /// A code block. May originate from markdown fenced code blocks or notebook code cells.
    CodeBlock {
        /// Code source
        source: String,
        /// Code reference. Currently only used for markdown.
        reference: Option<String>,
        /// Code attributes
        attr: CodeAttributes,
        /// Code tags
        tags: Option<Vec<String>>,
        /// Notebook cell outputs.
        outputs: Vec<CellOutput>,
    },
    /// A list - ordered or unordered.
    List(Option<u64>, Vec<Block>),
    ListItem(Vec<Block>),
}

/// Shortcode source. Can contain recursive ast elements.
#[derive(Debug, Clone)]
pub enum Shortcode {
    /// Inline code using the {{ name(param) }} syntax.
    Inline(ShortcodeBase),
    /// Block code using the {% name(param) %} body {% end_name %} syntax. The body can contain any
    /// valid ast elements.
    Block(ShortcodeBase, Vec<Block>),
}

pub(crate) fn str_to_blocks(input: &str) -> Vec<Block> {
    let ast: Ast = Parser::new_ext(input, Options::all()).collect();
    ast.0
}

pub(crate) fn math_block_md(src: &str, display_block: bool, trailing_space: bool) -> String {
    let delim = if display_block { "$$" } else { "$" };
    let trail = if trailing_space { " " } else { "" };
    format!("{}{}{}{}", delim, src, delim, trail)
}

/// Shortcode call and argument specification
#[derive(Debug, Clone)]
pub struct ShortcodeBase {
    /// Shortcode name (currently equivalent to the filename of the corresponding template)
    pub(crate) name: String,
    /// Shortcode reference - used to build a shortcode reference map for use in links.
    pub(crate) id: Option<String>,
    /// Shortcode number by type. Useful for enumeration - available for use in the shortcode
    /// template.
    pub(crate) num: usize,
    /// List of shortcode parameters.
    pub(crate) parameters: Vec<Argument<Vec<Block>>>,
}

pub(crate) enum ShortcodeIdx {
    Inline(usize, usize),
    Block {
        def: (usize, usize),
        end: (usize, usize),
    },
}

fn extract_block(start: usize, input: &str) -> Option<ShortcodeIdx> {
    let end = start + input[start..].find("%}")?;

    let mut level = 1;
    let mut cur_start = end;

    let mut end_block = end;

    while level > 0 {
        let new_start = input[(cur_start + 2)..]
            .find("{%")
            .map(|s| s + cur_start + 2);
        end_block = cur_start + 2 + input[cur_start + 2..].find("{% end")?;
        match new_start {
            Some(s) => {
                if s < end_block {
                    level += 1;
                    cur_start = s + 2;
                } else {
                    level -= 1;
                    cur_start = end_block + 7;
                }
            }
            None => {
                level -= 1;
                cur_start = end_block + 7;
            }
        }
    }

    let end_of_block = end_block + 6 + input[end_block + 6..].find("%}")?;

    Some(ShortcodeIdx::Block {
        def: (start, end),
        end: (end_block, end_of_block),
    })
}

fn extract_inline(start: usize, input: &str) -> Option<ShortcodeIdx> {
    let mut level = 1;
    let mut end = start;
    let mut cur_start = start;
    while level > 0 {
        let new_start = input[(cur_start + 2)..]
            .find("{{")
            .map(|s| s + cur_start + 2);
        end = cur_start + 2 + input[(cur_start + 2)..].find("}}")?;
        match new_start {
            Some(s) => {
                if s < end {
                    level += 1;
                    cur_start = s + 2;
                } else {
                    level -= 1;
                    cur_start = end + 2;
                }
            }
            None => {
                level -= 1;
                cur_start = end + 2;
            }
        }
    }
    Some(ShortcodeIdx::Inline(start, end))
}

pub(crate) fn find_all_blocks(input: &str) -> Vec<(usize, usize)> {
    let mut rest = input;
    let mut offset = 0;

    let mut res = Vec::new();
    loop {
        let next = find_next_block(rest);
        match next {
            None => return res,
            Some((start, end)) => {
                res.push((offset + start, offset + end));
                rest = &rest[(end)..];
                offset += end;
            }
        }
    }
}

fn find_next_block(input: &str) -> Option<(usize, usize)> {
    let start = input.find('`')?;
    let end_delim = if input[(start + 1)..].len() > 2 && &input[(start + 1)..(start + 3)] == "``" {
        "```"
    } else {
        "`"
    };

    let end = start + 1 + input[(start + 1)..].find(end_delim)? + end_delim.len();
    Some((start, end))
}

pub(crate) fn find_shortcode(input: &str) -> Option<ShortcodeIdx> {
    let start_inline = input.find("{{");
    let start_block = input.find("{%");

    match start_inline {
        None => start_block.and_then(|start| extract_block(start, input)),
        Some(inline_start_idx) => match start_block {
            None => extract_inline(inline_start_idx, input),
            Some(block_start_idx) => {
                if inline_start_idx < block_start_idx {
                    extract_inline(inline_start_idx, input)
                } else {
                    extract_block(block_start_idx, input)
                }
            }
        },
    }
}
