use crate::parse::{
    parse_error_message, parse_key, parse_value_boolean, parse_value_list, parse_value_string,
    parse_value_time, ParseError,
};

use crate::error::CustomError;
#[cfg(not(test))]
use log::warn;

#[cfg(test)]
use std::println as warn;

use chrono::NaiveDateTime;
use pulldown_cmark::{html, Options, Parser};
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
};

#[derive(Debug, PartialEq)]
pub struct Config {
    pub layout: String,
    pub base_layout: String,
    pub title: String,
    pub description: String,
    pub permalink: String,
    pub categories: Vec<String>,
    pub tags: Vec<String>,
    pub visible: bool,
    pub date: Option<NaiveDateTime>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            layout: String::from(""),
            base_layout: String::from("default"),
            title: String::from(""),
            description: String::from(""),
            permalink: String::from(""),
            categories: Vec::new(),
            tags: Vec::new(),
            visible: false,
            date: None,
        }
    }
}

impl Config {
    fn is_valid(&self) -> bool {
        !(self.layout.is_empty() || self.title.is_empty())
    }
}

#[derive(Debug)]
pub struct Article {
    pub template: String,
    pub config: Config,
    pub url: String,
    pub config_liquid: liquid::Object,
}

/// BufReader or read_to_string() is the key api choice (mmap alternatively as well)
/// the difficulty getting the rest of the file after parsing the config
/// BufReader<R> can improve the speed of programs that make small and repeated read calls to the same file or network socket.
/// It does not help when reading very large amounts at once, or reading just one or a few times.
/// It also provides no advantage when reading from a source that is already in memory, like a Vec<u8>.
pub fn parse(data: BufReader<File>, path: &PathBuf) -> Result<(Config, String), ParseError> {
    let mut found_config = false;
    let mut line_n = 1;
    let mut config = Config::default();

    // we set the defaults here e.g. default_layout: "default"
    // therefore when we get default_layout: "" then it overwrites the default
    let lines = data.lines();

    let mut body = "".to_string();
    let mut reached_end = false;

    for line in lines {
        let line = match &line {
            Ok(line) => line,
            Err(err) => Err(ParseError::InvalidValue(parse_error_message(
                &err.to_string(),
                path,
                "",
                0,
                10,
                line_n,
            )))?,
        };
        if !found_config && line == "---" {
            found_config = true;
            line_n += 1;
        } else if found_config && line == "---" {
            reached_end = true;
            found_config = false;
            line_n += 1;
        } else if reached_end {
            body += &line;
            body += "\n";
        } else if found_config {
            let (key, rest) = parse_key(&line, path, line, line_n)?;
            match key {
                // match each thing but then need to work out how to map it....
                // maybe look into the from string implementation???
                "layout" => {
                    config.layout = parse_value_string(rest.trim(), path, line, line_n)?.to_string()
                }
                "base_layout" => {
                    config.base_layout =
                        parse_value_string(rest.trim(), path, line, line_n)?.to_string()
                }
                "title" => {
                    config.title = parse_value_string(rest.trim(), path, line, line_n)?.to_string()
                }
                "description" => {
                    config.description =
                        parse_value_string(rest.trim(), path, line, line_n)?.to_string()
                }
                "permalink" => {
                    config.permalink =
                        parse_value_string(rest.trim(), path, line, line_n)?.to_string()
                }
                "categories" => {
                    config.categories = parse_value_list(rest.trim(), path, line, line_n)?
                }
                "tags" => config.tags = parse_value_list(rest.trim(), path, line, line_n)?,
                "titlebar" => {
                    config.visible = parse_value_boolean(rest.trim(), path, line, line_n)?
                }
                "date" => config.date = Some(parse_value_time(rest.trim(), path, line, line_n)?),
                _ => {
                    return Err(ParseError::InvalidKey(parse_error_message(
                        "unknown key",
                        path,
                        line,
                        0,
                        line.len() - 1,
                        line_n,
                    )))
                }
            }
            line_n += 1;
        } else {
            return Err(ParseError::InvalidConfig(parse_error_message(
                "configuration needs to start with '---' for the first line",
                path,
                line,
                0,
                line.len(),
                line_n,
            )));
        }
    }
    if config.is_valid() {
        return Ok((config, body));
    } else if line_n == 2 {
        return Err(ParseError::InvalidConfig(
            format!("empty config no key value pairs found in {}", "test.txt").into(),
        ));
    } else if !reached_end {
        return Err(ParseError::InvalidConfig(
            "no at '---' for the last line of the configuration".into(),
        ));
    } else if config.title.is_empty() {
        return Err(ParseError::InvalidConfig(
            "missing configuration 'title' field".into(),
        ));
    } else {
        return Err(ParseError::InvalidConfig(
            "missing configuration 'layout' field or 'base_layout' to be set to a custom value"
                .into(),
        ));
    }
}

impl Article {
    /// header is in a --- --- block with new lines
    /// the rest of the doc is template in markdown
    pub fn parse(md: BufReader<File>, path: &PathBuf) -> Result<Article, ParseError> {
        // markdown parsing NOTE: we are assuming that we are dealing with markdown hear!!!
        let (config, content) = parse(md, path)?;

        let template = content.trim().to_string();

        let url: String = if config.permalink.is_empty() {
            format!("{}.html", config.title)
        } else {
            config.permalink.clone() // messy.... argh!!!
        }
        .replace(" ", "%20");

        let config_liquid = liquid::object!({
            "content": template,
            "config": liquid::object!({
                "title": config.title,
                "description": config.description,
                "tags": config.tags,
                "categories": config.categories,
                "visible": config.visible,
                "layout": config.layout,
            }),
            "url":url,
        });

        return Ok(Article {
            template,
            config,
            url,
            config_liquid,
        });
    }

    fn pre_render(
        mut self,
        globals: &liquid::Object,
        liquid_parser: &liquid::Parser,
        md: bool,
    ) -> Result<Self, CustomError> {
        // hack do proper error handling!!!

        let template = liquid_parser
            .parse(&self.template)?
            .render(&liquid::object!({
                "global": globals,
                "page": self.config_liquid,
                "layout": self.config.layout
            }))?;

        self.template = if md {
            let parser = Parser::new_ext(&template, Options::empty());

            // Write to String buffer.
            let mut template = String::new();
            html::push_html(&mut template, parser);
            template
        } else {
            template
        };

        self.config_liquid = liquid::object!({
            "content": self.template,
            "config": liquid::object!({
                "title": self.config.title,
                "description": self.config.description,
                "tags": self.config.tags,
                "categories": self.config.categories,
                "visible": self.config.visible,
                "layout": self.config.layout,
            }),
            "url":self.url,
        });

        Ok(self)
    }

    fn render(
        &self,
        globals: &liquid::Object,
        parser: &liquid::Parser,
    ) -> Result<String, CustomError> {
        let template = if self.config.base_layout.is_empty() {
            if self.config.layout.is_empty() {
                warn!("no base layout found");
                if !self.template.contains("{{page.content}}")
                    || !self.template.contains("{{ page.content }}")
                {
                    //TODO: is this warning necessary and accurate????
                    warn!("potentailly missing out {{{{page.content}}}} in layout so none of the articles text will be displayed");
                }
                parser.parse(&format!("{{%- include '{0}' -%}}", self.config.layout))?
            } else {
                warn!("no base layout found and no layout found");
                parser.parse(&self.template)?
            }
        } else {
            warn!("using baselayout: {:?}", self.config.base_layout);
            if !self.template.contains("{{page.content}}")
                || !self.template.contains("{{ page.content }}")
            {
                //TODO: is this warning necessary and accurate????
                warn!("potentailly missing out {{{{page.content}}}} in layout so none of the  articles text will be displayed");
            }
            parser.parse(&format!("{{%- include '{0}' -%}}", self.config.base_layout))?
        };

        Ok(template.render(&liquid::object!({
            "global": globals,
            "page": self.config_liquid,
            "layout": self.config.layout
        }))?)
    }

    pub fn true_render(
        self,
        global: &liquid::Object,
        parser: &liquid::Parser,
    ) -> Result<String, CustomError> {
        Ok(self
            .pre_render(&global, parser, false)?
            .pre_render(&global, parser, true)?
            .render(&global, parser)?)
    }
}

#[cfg(test)]
mod render {

    use super::{Article, BufReader, CustomError, File, ParseError};
    use std::io::Write;
    use tempfile;

    // lazy didn't know how best to grab the type
    type Partials = liquid::partials::EagerCompiler<liquid::partials::InMemorySource>;

    fn create_article(md: &str, path: &str) -> Result<Article, ParseError> {
        // create a temp file
        let mut f = tempfile::Builder::new()
            .rand_bytes(0)
            .prefix("")
            .suffix(path)
            .tempfile_in("")
            .unwrap();

        write!(f, "{}", md).unwrap();

        Ok(Article::parse(
            BufReader::new(File::open(path).unwrap()),
            &std::path::PathBuf::from(path),
        )?)
    }

    fn gen_render_mocks(
        md: &str,
        path: &str,
        mocks: Vec<(String, String)>,
        global: &liquid::Object,
    ) -> Result<String, CustomError> {
        let a = create_article(md, path).unwrap();
        // create partials
        let mut source = Partials::empty();
        for (k, v) in mocks {
            source.add(k, v);
        }
        let parser = liquid::ParserBuilder::with_stdlib()
            .partials(source)
            .build()
            .unwrap();

        a.true_render(global, &parser)
    }

    mod parse_tests {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn empty_content() {
            assert_eq!(
                Some(ParseError::InvalidConfig(
                    "no at '---' for the last line of the configuration".into()
                )),
                create_article("", "empty_content").err()
            );
        }

        #[test]
        fn test_empty_template() {
            let a: Article = create_article(
                "---\nlayout:page\ntitle:cats and dogs\n---",
                "test_empty_template",
            )
            .unwrap();
            assert_eq!("", a.template);
        }

        #[test]
        fn parse() {
            let a: Article =
                create_article("---\nlayout:page\ntitle:cats and dogs\n---\ncat", "parse").unwrap();
            assert_eq!("cat", a.template);
            assert_eq!("page", a.config.layout);
        }

        #[test]
        fn parse_template_muli_line() {
            let a: Article = create_article(
                "---\nlayout:page\ntitle:cats and dogs\n---\ncat\ncat\ncat\ncat\ncat",
                "parse_template_multi_line",
            )
            .unwrap();
            assert_eq!("cat\ncat\ncat\ncat\ncat", a.template);
            assert_eq!("page", a.config.layout);
        }

        #[test]
        fn template_md_line() {
            let a: Article = create_article(
                "---\nlayout:page\ntitle:cats and dogs\n---\ncat---dog",
                "template_md_line",
            )
            .unwrap();
            assert_eq!("cat---dog", a.template);
            assert_eq!("page", a.config.layout);
        }

        #[test]
        fn parse_with_real() {
            let a: Article = create_article(
                "---\nlayout: page\ntitle:cats and dogs\n---\ncat",
                "parse_with_real",
            )
            .unwrap();
            assert_eq!("cat", a.template);
            assert_eq!("page", a.config.layout);
        }

        #[test]
        fn more_than_three_dashes() {
            let e = create_article(
                "----\nlayout:page\ntitle:cats and dogs\n-------\ncat",
                "more_than_three_seconds",
            )
            .err();
            assert!(e != None, "no error found");
            match e {
                Some(ParseError::InvalidConfig(config)) => {
                    assert!(config.contains("configuration needs to start with '---' for the first line"), "expected string to end with 'configuration needs to start with '---' for the first line' found {}", config)
                }
                _ => assert!(false, "looking for ParseError::InvalidConfig found {:?}", e)
            }
        }
    }

    mod layouts {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn render_default_layout() {
            assert_eq!(
                "cats",
                gen_render_mocks(
                    "---\r\nlayout: page\r\ntitle:cats and dogs\n---\r\ncat",
                    "render_default_layout",
                    vec![("default".to_string(), "cats".to_string())],
                    &liquid::object!({})
                )
                .unwrap()
            );
        }

        #[test]
        fn render_globals() {
            assert_eq!(
                "test1 <p>cat</p>\n",
                gen_render_mocks(
                    "---\r\nlayout: page\r\ntitle:cats and dogs\n---\r\ncat",
                    "render_globals",
                    vec![(
                        "default".to_string(),
                        "{{global}} {{page.content}}".to_string()
                    )],
                    &liquid::object!({
                        "test": 1
                    })
                )
                .unwrap()
            );
        }

        #[test]
        fn render_globals_scope() {
            assert_eq!(
                "1".to_string(),
                gen_render_mocks(
                    "---\r\nlayout: page\r\ntitle:cats and dogs\n---\r\ncat",
                    "render_globals_scope",
                    vec![("default".to_string(), "{{global.test}}".to_string())],
                    &liquid::object!({
                        "test": 1
                    })
                )
                .unwrap()
            );
        }

        #[test]
        fn render_content() {
            assert_eq!(
                "<h1>cats and dogs</h1><p>cat</p>\n".to_string(),
                gen_render_mocks(
                    "---\r\nlayout: page\r\ntitle:cats and dogs\n---\r\ncat",
                    "render_content",
                    vec![(
                        "default".to_string(),
                        "<h1>{{page.config.title}}</h1>{{page.content}}".to_string()
                    )],
                    &liquid::object!({
                        "test": 1
                    })
                )
                .unwrap()
            );
        }

        #[test]
        fn render_content_with_html_in_md() {
            assert_eq!(
                "<h1>cats and dogs</h1><p>cat<span>hello world</span></p>\n".to_string(),
                gen_render_mocks(
                    "---\r\nlayout: page\r\ntitle:cats and dogs\n---\r\ncat<span>hello world</span>",
                    "render_content_with_html_in_md",
                    vec![(
                        "default".to_string(),
                        "<h1>{{page.config.title}}</h1>{{page.content}}".to_string()
                    )],
                    &liquid::object!({
                        "test": 1
                    })
                )
                .unwrap()
            );
        }

        #[test]
        fn render_chained_includes() {
            assert_eq!(
                "I am a header<p>cat</p>\n".to_string(),
                gen_render_mocks(
                    "---\r\nlayout: page\r\ntitle:cats and dogs\n---\r\ncat",
                    "render_chained_includes",
                    vec![
                        (
                            "default".to_string(),
                            "{% include 'header' %}{% include layout %}".to_string()
                        ),
                        ("header".to_string(), "I am a header".to_string()),
                        ("page2".to_string(), "1".to_string()),
                        ("page3".to_string(), "2".to_string()),
                        ("page".to_string(), "{{page.content}}".to_string()),
                        ("page4".to_string(), "3".to_string())
                    ],
                    &liquid::object!({
                        "test": 1
                    })
                )
                .unwrap()
            );
        }

        #[test]
        fn render_template_jekyll() {
            assert_eq!(
                "<h1>mole</h1><p>cat mole</p>\n".to_string(),
                gen_render_mocks(
                    "---\r\nlayout: page\r\ntitle:mole\n---\r\ncat {{page.config.title}}",
                    "render_template_jekyll",
                    vec![
                        (
                            "default".to_string(),
                            "<h1>{{page.config.title}}</h1>{% include layout %}".to_string()
                        ),
                        ("page".to_string(), "{{page.content}}".to_string())
                    ],
                    &liquid::object!({
                        "test": 1
                    })
                )
                .unwrap()
            );
        }
    }
}
