use std::fmt::{Display, Formatter};
use std::ops::Range;
use std::vec::IntoIter;

use pulldown_cmark::CodeBlockKind::Fenced;
use pulldown_cmark::Tag::CodeBlock;
use pulldown_cmark::{CowStr, Event, OffsetIter, Options, Parser};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ast::{AEvent, Ast, Block, CodeAttributes};
use crate::config::OutputFormat;
use crate::notebook::{Cell, CellOutput, Notebook};
use crate::processors::shortcodes::ShortCodeProcessError;
use crate::processors::MarkdownPreprocessor;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentMetadata {
    pub title: Option<String>,
    #[serde(default = "default_true")]
    pub exercises: bool,
    #[serde(default)]
    pub notebook_output: bool,
    #[serde(default)]
    pub code_solutions: bool,
    #[serde(default = "default_true")]
    pub cell_outputs: bool,
    #[serde(default)]
    pub interactive: bool,
    #[serde(default)]
    pub editable: bool,
    #[serde(default)]
    pub layout: LayoutSettings,

    #[serde(default = "default_outputs")]
    pub outputs: Vec<OutputFormat>,
}

fn default_true() -> bool {
    true
}

fn default_outputs() -> Vec<OutputFormat> {
    vec![
        OutputFormat::Notebook,
        OutputFormat::Html,
        OutputFormat::Info,
        OutputFormat::LaTeX,
    ]
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LayoutSettings {
    pub hide_sidebar: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Document<C> {
    pub content: C,
    pub metadata: DocumentMetadata,
    pub variables: DocumentVariables,
}

pub type RawContent = Vec<Element>;
pub type EventContent = Vec<AEvent>;

#[derive(Debug, Clone, Default)]
pub enum Element {
    Markdown {
        content: String,
    },
    Code {
        cell_number: usize,
        content: String,
        output: Option<Vec<CellOutput>>,
    },
    Raw {
        content: String,
    },
    #[default]
    Default,
}

impl From<Element> for Vec<Block> {
    fn from(value: Element) -> Self {
        match value {
            Element::Markdown { content } => {
                let ast: Ast = Parser::new_ext(&content, Options::all()).collect();
                ast.0
            }
            Element::Code {
                content, output, ..
            } => {
                vec![Block::CodeBlock {
                    source: content,
                    reference: None,
                    attr: CodeAttributes {
                        editable: true,
                        fold: false,
                    },
                    outputs: output.unwrap_or(Vec::default()),
                }]
            }
            Element::Raw { .. } => {
                vec![]
            }
            Element::Default => {
                vec![]
            }
        }
    }
}

impl From<RawContent> for Ast {
    fn from(value: RawContent) -> Self {
        Ast(value
            .into_iter()
            .flat_map(|c| -> Vec<Block> { c.into() })
            .collect())
    }
}

#[derive(Debug, Clone)]
pub struct DocPos {
    cell_number: Option<usize>,
    #[allow(unused)]
    global_offset: usize,
    line: usize,
    #[allow(unused)]
    local_position: Range<usize>,
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct DocumentVariables {
    pub first_heading: Option<String>,
}

#[derive(Error, Debug)]
pub enum PreprocessError {
    #[error(transparent)]
    Shortcode(#[from] ShortCodeProcessError),
}

impl Display for DocPos {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.cell_number {
            None => write!(f, "line: {}", self.line),
            Some(n) => write!(f, "cell: {}, local position: {}", n, self.line),
        }
    }
}

impl DocPos {
    pub fn new(
        cell_number: Option<usize>,
        global_offset: usize,
        line: usize,
        local_position: Range<usize>,
    ) -> Self {
        DocPos {
            cell_number,
            global_offset,
            line,
            local_position,
        }
    }
}

impl<T> Document<T> {
    pub fn map<O, F: Fn(T) -> O>(self, f: F) -> Document<O> {
        Document {
            content: f(self.content),
            metadata: self.metadata,
            variables: self.variables,
        }
    }
}

impl Document<RawContent> {
    pub fn preprocess(
        self,
        processor: &dyn MarkdownPreprocessor,
        ctx: &tera::Context,
    ) -> Result<Document<RawContent>, anyhow::Error> {
        let elements = self
            .content
            .iter()
            .map(|e| match e {
                Element::Markdown { content } => Ok(Element::Markdown {
                    content: processor.process(content, ctx)?,
                }),
                _ => Ok(e.clone()),
            })
            .collect::<Result<Vec<Element>, anyhow::Error>>()?;
        Ok(Document {
            content: elements,
            metadata: self.metadata,
            variables: DocumentVariables::default(),
        })
    }

    pub(crate) fn new<C: IntoRawContent>(content: C, metadata: DocumentMetadata) -> Self {
        Document {
            metadata,
            variables: DocumentVariables::default(),
            content: content.into(),
        }
    }

    pub fn to_events(&self, config: IteratorConfig) -> Document<EventContent> {
        let content = self.configure_iterator(config).map(|e| e.into());
        Document {
            metadata: self.metadata.clone(),
            variables: DocumentVariables::default(),
            content: content.collect(),
        }
    }
}

impl Document<Ast> {
    pub fn to_events(&self) -> Document<EventContent> {
        let content = self.content.clone().into_iter().collect();
        Document {
            metadata: self.metadata.clone(),
            variables: DocumentVariables::default(),
            content,
        }
    }
}

impl Document<EventContent> {
    pub fn to_events(&self) -> impl Iterator<Item = Event> {
        self.content.iter().map(|e| e.into())
    }

    // pub fn to_events_with_pos<'a>(&'a self) -> impl Iterator<Item = (Event<'a>, DocPos)> {
    //     self.content.iter().map(|(e, p)| (e.into(), p.clone()))
    // }
}

pub trait IntoRawContent {
    fn into(self) -> RawContent;
}

impl IntoRawContent for String {
    fn into(self) -> RawContent {
        vec![Element::Markdown { content: self }]
    }
}

impl IntoRawContent for Notebook {
    fn into(self) -> RawContent {
        self.cells
            .into_iter()
            .fold((1, Vec::new()), |(num, mut acc), cell| {
                let next = match &cell {
                    Cell::Code { .. } => num + 1,
                    _ => num,
                };
                acc.push((next, cell));
                (next, acc)
            })
            .1
            .into_iter()
            .map(|(i, cell)| match cell {
                Cell::Markdown { common } => Element::Markdown {
                    content: common.source,
                },
                Cell::Code {
                    common, outputs, ..
                } => Element::Code {
                    cell_number: i,
                    content: common.source,
                    output: Some(outputs),
                },
                Cell::Raw { common } => Element::Raw {
                    content: common.source,
                },
            })
            .collect()
    }
}

pub struct ElementIterator<'a, 'b> {
    cell_iter: ElementIteratorCell<'a, 'b>,
}

pub enum ElementIteratorCell<'a, 'b> {
    Markdown {
        parser: Box<OffsetIter<'a, 'b>>,
    },
    Code {
        cell_number: usize,
        events: Box<IntoIter<(Event<'a>, Range<usize>)>>,
    },
    Raw {},
}

impl<'a, 'b> ElementIterator<'a, 'b> {
    fn map_doc_pos(&self, elem: (Event<'a>, Range<usize>)) -> Event<'a> {
        // let cell_num = match &self.cell_iter {
        //     ElementIteratorCell::Code { cell_number, .. } => Some(*cell_number),
        //     _ => None,
        // };
        // let line = &self.source[elem.1.start..elem.1.end].lines().count();

        elem.0
    }
}

impl<'a, 'b> Iterator for ElementIterator<'a, 'b> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.cell_iter {
            ElementIteratorCell::Markdown { parser, .. } => {
                parser.next().map(|e| self.map_doc_pos(e))
            }
            ElementIteratorCell::Code { events, .. } => events.next().map(|e| self.map_doc_pos(e)),
            ElementIteratorCell::Raw { .. } => None,
        }
    }
}

#[derive(Default, Copy, Clone)]
pub struct IteratorConfig {
    pub include_output: bool,
    pub include_solutions: bool,
}

impl IteratorConfig {
    #[allow(unused)]
    pub fn include_output(self) -> Self {
        IteratorConfig {
            include_output: true,
            include_solutions: self.include_solutions,
        }
    }

    #[allow(unused)]
    pub fn include_solutions(self) -> Self {
        IteratorConfig {
            include_output: self.include_output,
            include_solutions: true,
        }
    }
}

pub trait ConfigureCollector {
    type Item;
    type IntoIter;

    fn configure_iterator(self, config: IteratorConfig) -> Self::IntoIter;
}

pub trait ConfigureElemIterator {
    type Item;
    type IntoIter;

    fn configure_iterator(self, cell_number: usize, config: IteratorConfig) -> Self::IntoIter;
}

impl<'a> ConfigureCollector for &'a Element {
    type Item = Event<'a>;
    type IntoIter = ElementIterator<'a, 'a>;

    fn configure_iterator(self, config: IteratorConfig) -> Self::IntoIter {
        let (cell, _content) = match self {
            Element::Markdown { content } => (
                ElementIteratorCell::Markdown {
                    parser: Box::new(Parser::new_ext(content, Options::all()).into_offset_iter()),
                },
                content.clone(),
            ),

            Element::Code {
                cell_number,
                content,
                output: outputs,
            } => {
                let cblock = CodeBlock(Fenced(CowStr::Boxed("python".into())));
                let mut events = vec![
                    (Event::Start(cblock.clone()), (0..0)),
                    (Event::Text(CowStr::Borrowed(content)), (0..content.len())),
                    (Event::End(cblock), (content.len()..content.len())),
                ];
                if config.include_output {
                    if let Some(os) = outputs {
                        for o in os {
                            events.append(&mut o.to_events());
                        }
                    }
                }

                (
                    ElementIteratorCell::Code {
                        cell_number: *cell_number,
                        events: Box::new(events.into_iter()),
                    },
                    content.clone(),
                )
            }
            Element::Raw { content } => (ElementIteratorCell::Raw {}, content.clone()),
            _ => (ElementIteratorCell::Raw {}, "".to_string()),
        };
        ElementIterator { cell_iter: cell }
    }
}

impl<'a> ConfigureCollector for &'a Document<RawContent> {
    type Item = Event<'a>;
    type IntoIter = Box<dyn Iterator<Item = Self::Item> + 'a>;

    fn configure_iterator(self, config: IteratorConfig) -> Self::IntoIter {
        Box::new(
            self.content
                .iter()
                .flat_map(move |elem: &Element| elem.configure_iterator(config)),
        )
    }
}
