---
title: Project organisation and configuration
---

# Project organisation and configuration

All Courses projects have the following elements:
- `build/` contains the project outputs (webpage, notebooks, etc.)
- `resources/` contains all static resources used in the templates or content.
- `content/` contains all source files for generating content, whether they are documents or scripts.
- `templates/` contains layout templates as well as shortcode templates.
- `config.yml` is the global project configuration. This is the only explicit configuration file.

Once the site layouts and shortcodes have been completed, the `content/` folder is where most further customization happens. The organization of `content/` directly determines the organisation of the final webpage and other outputs.

Here's an example of what a project's directory may look like:
```plain
- config.yml
- build/
- resources/
- content/
    - part-a/
        - index.md
        - chapter1/
            - index.md
            - section-01.md
        - chapter2/
            - index.ipynb
            - section-01.ipynb
            - section-02.md
            - myscript.py
    - part-b/
        - index.md
        - chapter1/
            - index.md
        - chapter2/
            - index.md
- templates/
    - index.tera.html
    - section.tera.html
    - shortcodes/
        - html/
            - image.tera.html
        - md/
            - image.md.html
```

## Content organisation

Courses projects are currently limited to four levels of documents: *the project*, *parts*, *chapters*, and *sections* (this may change in the future). Each level has a corresponding document. In the case of parts, chapters, or an entire project, these documents are always named `index` (and then either the `.md` or `.ipynb` extension) inside the corresponding level folder. Since *sections* do not have children, they are placed on the same level as chapter documents but with arbitrary names. The above example have folders named after their respective levels to exemplify how this works in practice. 

{% message(title=Note, color=info) %}
The name `index` is used because these documents are often used as overview pages for the next document level. 
{% end %}

## Configuring content
Courses has only as single global configuration file, `config.yml`, that only contains globally relevant information. Content configuration is instead specified in the individual content files using the `yaml` language. In markdown  files, this is done using the *frontmatter syntax*. Example:

```plain
---
this: is
yaml:
    - item
---

# Regular markdown
Some text...
```

In notebooks (`.ipynb` files) it is done by adding a `raw` cell to the very top of the document with the `yaml`-configuration inside.

### Configuration options 
Document configurations consist of a number of possible fields, most of which have default values. This means you can usually leave out most options. The full set of options currently are:
```yaml
title: # String (required)
code_split: true # boolean
notebook_output: true # boolean
layout:
  hide_sidebar: true # boolean
output:
  web: true # boolean
  source: true # boolean
```
with only the `title` being required.

- `code_split`: Enable/disable parsing of the exercise placeholder/solution syntax in the document. This option is only useful for showing the actual syntax instead of parsing it, as is done on the page for its documentation.
- `notebook_output`: Toggle the notebook cell outputs for the whole document. It is useful for exercise-like documents with outputs created during testing that should not be included in the outputs.
- `layout`: Options for changing the webpage layout. Currently only supports hiding the sidebar.
- `output`: Enable/disable output generation for web and/or notebooks (called `source` because script files are also included).

## Global configuration
The `config.yml` is used for changing _settings related to the project as a whole. The default configuration is:
```yaml
url_prefix: ""
build:
  dev:
    katex_output: false
  release:
    katex_output: true
```
*As with the file configurations, the defaults are selected if your configuration file does not define the given property.*

The `build` element defines different build profiles, similar to many build tools such as Maven, Cargo, Cmake, and many more. The reason for having multiple configurations is that it allows the final deployment _settings to differ from what is used for local development. In the default case, the `dev` profile does not precompile LaTeX math expressions (using the KaTeX library) - instead they will be rendered by the browser. The `release` profile invokes KaTeX in the build step which is slower when building but faster when showing the webpage. 

Right now, there are very few meaningful options to warrant this multi-profile setup, but more will be added in the future. One very obvious use case is to output some form of helpful information for development in the `dev` profile.


## Build process and outputs
When you build a courses project, the tool generates a webpage as well as a directory of processed notebooks and other source files. This makes using Courses for course content very easy, since the generated notebooks are optimized for distribution. The notebooks are subjected to the same processing pipeline which parses the placeholder/solution syntax and renders shortcode templates. The only difference is that the output are `.ipynb` files instead of web-pages.

### Web process
The generated web-pages are rendered using the layout files in `templates/`. The result is a folder `build/web/` which contains everything necessary for deploying the site, including the content of the `resources/` folder. You can therefore upload the output directly to any static-site host provider such as GitHub Pages or Amazon S3. 

### Notebook process
Notebooks are generated by applying the placeholder/solution syntax to all code cells and then rendering shortcodes using the markdown templates (the ones in `templates/shortcodes/md/`). Having separate templates for `html` and `markdown` outputs makes it easy to write documents with complex elements such as *images* and *admonitions* on the webpage without ending up with a notebook filled with `html`. 


### Other files

It is often useful to include additional code files or data files for use in the actual content. Courses therefore copies all files not ending in `.md` or `.ipynb` directly from the *content* folder to the `build/source` output folder.


