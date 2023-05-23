use crate::ast::{Block, Inline, Shortcode};
use crate::document::Document;
use crate::notebook::{CellOutput, OutputValue, StreamType};

use anyhow::{anyhow, Result};
use pulldown_cmark::HeadingLevel;

use crate::parsers::shortcodes::{Argument, ShortCodeDef};
use std::collections::HashMap;
use std::io::{Cursor, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use tera::Context;

use crate::renderers::{DocumentRenderer, RenderContext, RenderElement, RenderResult};
use crate::templates::{TemplateDefinition, TemplateType};

fn write_bytes(source: &str, mut buf: impl Write) -> Result<()> {
    let bytes = source.as_bytes();
    let l = buf.write(bytes)?;
    (l == bytes.len())
        .then_some(())
        .ok_or(anyhow!("did not write correct number of bytes"))
}

pub struct GenericRenderer;

impl DocumentRenderer for GenericRenderer {
    fn render_doc(&mut self, ctx: &RenderContext) -> Result<Document<RenderResult>> {
        // let doc = doc.to_events();
        // let dd = doc.to_events();
        //
        // let mut output = String::new();
        // html::push_html(&mut output, dd);
        let buf = Vec::new();
        let mut cursor = Cursor::new(buf);
        self.render(&ctx.doc.content.0, ctx, &mut cursor)?;

        let content = String::from_utf8(cursor.get_ref().clone())?;
        Ok(Document {
            content,
            metadata: ctx.doc.metadata.clone(),
            variables: ctx.doc.variables.clone(),
            ids: ctx.doc.ids.clone(),
            id_map: ctx.doc.id_map.clone(),
        })
    }
}

impl GenericRenderer {
    fn render_params(
        &mut self,
        parameters: Vec<Argument<Vec<Block>>>,
        ctx: &RenderContext,
    ) -> Result<Vec<Argument<String>>> {
        parameters
            .into_iter()
            .map(|p| p.try_map(|v| v.try_map(|i| self.render_inner(&i, ctx))))
            .collect()
    }

    fn render_shortcode_template(
        &mut self,
        ctx: &RenderContext,
        shortcode: &Shortcode,
        buf: impl Write,
    ) -> Result<()> {
        let mut args = ctx.extra_args.clone();
        args.insert("defs", &ctx.templates.definitions);

        let name = match shortcode {
            Shortcode::Inline(def) => {
                let rendered = self.render_params(def.parameters.clone(), ctx)?;
                let r: anyhow::Result<Vec<()>> = ctx
                    .templates
                    .validate_args_for_template(&def.name, &rendered)?
                    .into_iter()
                    .collect();
                r?;
                let tdef = ctx
                    .templates
                    .get_template(&def.name, TemplateType::Shortcode)?;
                add_args(
                    &tdef,
                    &mut args,
                    &def.id,
                    def.num,
                    ctx.ids,
                    ctx.ids_map,
                    rendered,
                )?;
                def.name.clone()
            }
            Shortcode::Block(def, body) => {
                let rendered = self.render_params(def.parameters.clone(), ctx)?;
                let r: anyhow::Result<Vec<()>> = ctx
                    .templates
                    .validate_args_for_template(&def.name, &rendered)?
                    .into_iter()
                    .collect();
                r?;
                let tdef = ctx
                    .templates
                    .get_template(&def.name, TemplateType::Shortcode)?;
                add_args(
                    &tdef,
                    &mut args,
                    &def.id,
                    def.num,
                    ctx.ids,
                    ctx.ids_map,
                    rendered,
                )?;
                let body = self.render_inner(body, ctx)?;
                args.insert("body", &body);
                def.name.clone()
            }
        };
        ctx.templates
            .render(&name, ctx.format, TemplateType::Shortcode, &args, buf)
    }
}

impl RenderElement<Inline> for GenericRenderer {
    fn render(&mut self, elem: &Inline, ctx: &RenderContext, mut buf: impl Write) -> Result<()> {
        match elem {
            Inline::Text(s) => {
                let _ = buf.write(s.as_bytes())?;
                Ok(())
            }
            Inline::Emphasis(inner) => render_value_template(
                "emphasis",
                TemplateType::Builtin,
                &self.render_inner(inner, ctx)?,
                ctx,
                buf,
            ),
            Inline::Strong(inner) => render_value_template(
                "strong",
                TemplateType::Builtin,
                &self.render_inner(inner, ctx)?,
                ctx,
                buf,
            ),
            Inline::Strikethrough(inner) => render_value_template(
                "strikethrough",
                TemplateType::Builtin,
                &self.render_inner(inner, ctx)?,
                ctx,
                buf,
            ),
            Inline::Code(s) => {
                render_value_template("inline_code", TemplateType::Builtin, s, ctx, buf)
            }
            Inline::SoftBreak => {
                render_basic_template("soft_break", TemplateType::Builtin, ctx, buf)
            }
            Inline::HardBreak => {
                render_basic_template("hard_break", TemplateType::Builtin, ctx, buf)
            }
            Inline::Rule => {
                render_basic_template("horizontal_rule", TemplateType::Builtin, ctx, buf)
            }
            Inline::Image(_tp, url, alt, inner) => {
                let inner = self.render_inner(inner, ctx)?;
                render_image(url, alt, &inner, ctx, buf)
            }
            Inline::Link(_tp, url, alt, inner) => {
                let inner = self.render_inner(inner, ctx)?;
                render_link(url, alt, &inner, ctx, buf)
            }
            Inline::Html(s) => write_bytes(s, buf),
            Inline::Math {
                source,
                display_block,
                trailing_space,
            } => render_math(*display_block, *trailing_space, source, ctx, buf),
            Inline::Shortcode(s) => Ok(self.render_shortcode_template(ctx, s, buf)?),
        }
    }
}

impl RenderElement<OutputValue> for GenericRenderer {
    fn render(&mut self, elem: &OutputValue, ctx: &RenderContext, buf: impl Write) -> Result<()> {
        match elem {
            OutputValue::Plain(s) => {
                render_value_template("output_text", TemplateType::Builtin, &s.join(""), ctx, buf)
            }
            OutputValue::Image(s) => {
                render_value_template("output_img", TemplateType::Builtin, s, ctx, buf)
            }
            OutputValue::Svg(s) => {
                render_value_template("output_svg", TemplateType::Builtin, s, ctx, buf)
            }
            OutputValue::Json(s) => write_bytes(&serde_json::to_string(s)?, buf),
            OutputValue::Html(s) => write_bytes(s, buf),
            OutputValue::Javascript(_) => Ok(()),
        }
    }
}

impl RenderElement<CellOutput> for GenericRenderer {
    fn render(
        &mut self,
        elem: &CellOutput,
        ctx: &RenderContext,
        mut buf: impl Write,
    ) -> Result<()> {
        match elem {
            CellOutput::Stream { text, name } => match name {
                StreamType::StdOut => {
                    render_value_template("output_text", TemplateType::Builtin, text, ctx, buf)
                }
                StreamType::StdErr => {
                    render_value_template("output_error", TemplateType::Builtin, text, ctx, buf)
                }
            },
            CellOutput::Data { data, .. } => {
                for v in data {
                    self.render(v, ctx, &mut buf)?;
                }
                Ok(())
            }
            CellOutput::Error { evalue, .. } => {
                render_value_template("output_error", TemplateType::Builtin, evalue, ctx, buf)
            }
        }
    }
}

pub fn header_lvl_to_int(lvl: &HeadingLevel) -> usize {
    match lvl {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

impl RenderElement<Block> for GenericRenderer {
    fn render(&mut self, elem: &Block, ctx: &RenderContext, buf: impl Write) -> Result<()> {
        match elem {
            Block::Heading { lvl, inner, .. } => {
                let mut args = Context::default();
                // println!("{}", );
                args.insert("level", &header_lvl_to_int(lvl));
                args.insert("inner", &self.render_inner(inner, ctx)?);
                Ok(ctx
                    .templates
                    .render("header", ctx.format, TemplateType::Builtin, &args, buf)?)
            }
            Block::Plain(inner) => self.render(inner, ctx, buf),
            Block::Paragraph(inner) | Block::BlockQuote(inner) => render_value_template(
                "paragraph",
                TemplateType::Builtin,
                &self.render_inner(inner, ctx)?,
                ctx,
                buf,
            ),
            Block::CodeBlock {
                source,
                outputs,
                tags,
                ..
            } => {
                let id = get_id();

                let highlighted = syntect::html::highlighted_html_for_string(
                    source,
                    &ctx.syntax_set,
                    ctx.syntax_set.find_syntax_by_extension("py").unwrap(),
                    &ctx.theme,
                )?;

                let mut args = Context::default();
                args.insert("interactive", &ctx.doc.metadata.interactive);
                args.insert("cell_outputs", &ctx.doc.metadata.cell_outputs);
                args.insert("editable", &ctx.doc.metadata.editable);
                args.insert("source", &source);
                args.insert("highlighted", &highlighted);
                args.insert("id", &id);
                args.insert("tags", &tags);
                args.insert("outputs", &self.render_inner(outputs, ctx)?);

                Ok(ctx
                    .templates
                    .render("cell", ctx.format, TemplateType::Builtin, &args, buf)?)
            }
            Block::List(idx, items) => {
                let inner = self.render_inner(items, ctx)?;
                // let inner: Result<String> = items.iter().map(|b| self.render(b, ctx)).collect();
                // let inner = inner?;

                match idx {
                    None => render_value_template(
                        "list_unordered",
                        TemplateType::Builtin,
                        &inner,
                        ctx,
                        buf,
                    )?,
                    Some(start) => {
                        let mut args = Context::default();
                        args.insert("start", &start);
                        args.insert("value", &inner);
                        ctx.templates.render(
                            "list_ordered",
                            ctx.format,
                            TemplateType::Builtin,
                            &args,
                            buf,
                        )?
                    }
                };
                Ok(())
            }
            Block::ListItem(inner) => render_value_template(
                "list_item",
                TemplateType::Builtin,
                &self.render_inner(inner, ctx)?,
                ctx,
                buf,
            ),
        }
    }
}

fn render_basic_template(
    name: &str,
    type_: TemplateType,
    ctx: &RenderContext,
    buf: impl Write,
) -> Result<()> {
    ctx.templates
        .render(name, ctx.format, type_, &Context::default(), buf)
}

fn render_value_template(
    name: &str,
    type_: TemplateType,
    value: &str,
    ctx: &RenderContext,
    buf: impl Write,
) -> Result<()> {
    let mut args = Context::default();
    args.insert("value", value);
    ctx.templates.render(name, ctx.format, type_, &args, buf)
}

static COUNTER: AtomicUsize = AtomicUsize::new(1);

fn get_id() -> usize {
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

fn add_args(
    def: &TemplateDefinition,
    args: &mut Context,
    id: &Option<String>,
    num: usize,
    ids: &HashMap<String, (usize, Vec<ShortCodeDef>)>,
    id_map: &HashMap<String, (usize, ShortCodeDef)>,
    arguments: Vec<Argument<String>>,
) -> Result<()> {
    if let Some(id) = id {
        args.insert("id", &id);
    }
    args.insert("num", &num);
    args.insert("ids", &ids);
    args.insert("id_map", &id_map);
    for (i, p) in arguments.into_iter().enumerate() {
        match p {
            Argument::Positional(val) => args.insert(
                def.shortcode.as_ref().unwrap().parameters[i].name.clone(),
                val.inner(),
            ),
            Argument::Keyword(k, v) => args.insert(k, v.inner()),
        }
    }
    Ok(())
}

fn render_image(
    url: &str,
    alt: &str,
    inner: &str,
    ctx: &RenderContext,
    buf: impl Write,
) -> Result<()> {
    let mut args = Context::default();
    args.insert("url", url);
    args.insert("alt", alt);
    args.insert("inner", inner);
    ctx.templates
        .render("image", ctx.format, TemplateType::Builtin, &args, buf)
}

fn render_link(
    url: &str,
    alt: &str,
    inner: &str,
    ctx: &RenderContext,
    buf: impl Write,
) -> Result<()> {
    let mut args = Context::default();
    args.insert("url", url);
    args.insert("alt", alt);
    args.insert("inner", inner);
    ctx.templates
        .render("link", ctx.format, TemplateType::Builtin, &args, buf)
}

fn render_math(
    display_mode: bool,
    trailing_space: bool,
    inner: &str,
    ctx: &RenderContext,
    buf: impl Write,
) -> Result<()> {
    let mut args = Context::default();
    args.insert("display_mode", &display_mode);
    args.insert("trailing_space", &trailing_space);
    args.insert("value", inner);
    ctx.templates
        .render("math", ctx.format, TemplateType::Builtin, &args, buf)
}
