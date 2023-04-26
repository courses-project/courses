use std::fs;
use std::ops::Deref;

use indicatif::{ProgressBar, ProgressStyle};
use tera::Tera;

use cdoc::document::Document;
use cdoc::renderers::RenderResult;

use crate::generators::{Generator, GeneratorContext};
use crate::project::ItemDescriptor;

pub struct MarkdownGenerator {
    #[allow(unused)]
    tera: Tera,
}

impl MarkdownGenerator {
    pub fn new(tera: Tera) -> Self {
        MarkdownGenerator { tera }
    }
}

impl Generator for MarkdownGenerator {
    fn generate(&self, ctx: GeneratorContext) -> anyhow::Result<()> {
        let spinner = ProgressStyle::with_template("{prefix:.bold.dim} {spinner} {wide_msg}")
            .unwrap()
            .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ");
        let pb = ProgressBar::new(0);
        pb.set_style(spinner);

        for item in ctx.project {
            if let Some(c) = item.doc.content.deref() {
                pb.set_message(format!("{}", item.doc.path.display()));
                pb.inc(1);

                let mut markdown_build_dir = ctx.build_dir.as_path().join(&item.doc.path);
                markdown_build_dir.pop(); // Pop filename
                let markdown_build_path = markdown_build_dir.join(format!("{}.md", item.doc.id));

                fs::create_dir_all(markdown_build_dir)?;
                fs::write(markdown_build_path, &c.content)?;
            }
        }

        // pb.finish_with_message(format!("notebook rendering {}", style("success").green()));
        pb.finish_and_clear();

        Ok(())
    }

    fn generate_single(
        &self,
        content: Document<RenderResult>,
        doc_info: ItemDescriptor<()>,
        ctx: GeneratorContext,
    ) -> anyhow::Result<()> {
        let mut markdown_build_dir = ctx.build_dir.as_path().join(&doc_info.doc.path);
        markdown_build_dir.pop(); // Pop filename
        let markdown_build_path = markdown_build_dir.join(format!("{}.md", doc_info.doc.id));

        fs::create_dir_all(markdown_build_dir)?;
        fs::write(markdown_build_path, content.content)?;

        Ok(())
    }
}