pub mod article;
use log::{error, info, warn};
use std::collections::HashMap;
use std::fs::read_to_string;
use std::fs::File;
use std::io::{prelude::*, BufReader};
use std::path::{Path, PathBuf};
pub mod error;
pub mod parse;
mod util;

pub type Partials = liquid::partials::EagerCompiler<liquid::partials::InMemorySource>;

pub struct Build<'a> {
    includes: Partials,
    articles: Vec<article::Article>,
    layouts: Vec<String>,
    output: &'a PathBuf,

    backtrace: bool,
    article_paths: Vec<String>,
    includes_paths: HashMap<String, String>,
}

impl<'a> Build<'a> {
    pub fn new(output: &'a PathBuf, backtrace: bool) -> Self {
        Build {
            includes: Partials::empty(),
            layouts: Vec::new(),
            articles: Vec::new(),
            output,
            backtrace,
            article_paths: Vec::new(),
            includes_paths: HashMap::new(),
        }
    }

    /// note: includes are hard-coded as .html files
    /// in util:search_dir and util::path_file_name_to_string
    pub fn includes(mut self, dir: &'a PathBuf, layout: bool) -> Self {
        if dir.exists() && dir.is_dir() {
            for file_path in util::search_dir(dir, "html", false) {
                if let Ok(content) = util::read_file(&file_path) {
                    match util::path_file_name_to_string(&file_path) {
                        Ok(rel_path) => {
                            if layout {
                                info!("new layout {:?}", rel_path);
                            } else {
                                info!("new include {:?}", rel_path);
                            }

                            // only including error information when backtrace enabled otherwise we just ignore it
                            if self.backtrace {
                                self.includes_paths
                                    .insert(rel_path.clone(), format!("{:?}", file_path));
                            }

                            // layouts and includes both liquid templates
                            if self.includes.add(&rel_path, content) {
                                if layout {
                                    error!("\"{:?}\" already exists as a layout, note: layouts and includes share the same name", rel_path);
                                } else {
                                    error!("\"{:?}\" already exists as a includes, note: layouts and includes share the same name", rel_path);
                                }
                            }

                            if layout {
                                // this is used to check that articles have a valid layout
                                self.layouts.push(rel_path);
                            }
                        }
                        Err(e) => error!("{:?}", e),
                    }
                } else {
                    error!("unable to read file {:?}", file_path);
                }
            }
        } else {
            error!("{:?} is not a path or directory", &dir);
        }
        self
    }

    pub fn articles(mut self, temp: &'a Vec<&'a PathBuf>) -> Self {
        for dir in temp {
            info!("looking for markdown articles in {:?}", dir);
            if dir.exists() && dir.is_dir() {
                if self.layouts.is_empty() {
                    panic!(
                "empty layout list, please load in layout template files before parsing articles"
            );
                } else {
                    for f in util::search_dir(&dir, "md", true) {
                        if let Ok(cat) = File::open(&f) {
                            match article::Article::parse(BufReader::new(cat), &f) {
                                Ok(art) => {
                                    self.articles.push(art);
                                    self.article_paths.push(format!("{:?}", &f));
                                }
                                Err(e) => error!("{:?}", e),
                            }
                        } else {
                            error!("Could not read {:?}", &f);
                        }
                    }
                }
            } else {
                error!("{:?} is not a path or directory", dir);
            }
        }

        self
    }

    pub fn sass(self, dir: &'a PathBuf, load_paths: &Vec<&Path>) -> Self {
        if dir.exists() && dir.is_dir() {
            for f in util::search_dir(dir, "scss", true) {
                if let Ok(data) = read_to_string(&f) {
                    match grass::from_string(
                        data,
                        &grass::Options::default().load_paths(load_paths),
                    ) {
                        Ok(css) => {
                            let mut output_path = self.output.clone();
                            output_path
                                .push(Path::new(&util::path_file_name_to_string(&f).unwrap()));
                            info!("writing css to {:?}", output_path);

                            let mut file = File::create(output_path).unwrap();
                            file.write_all(css.as_bytes()).unwrap();
                        }
                        Err(e) => warn!("parsing sccs {:?} caused {:?}", &f, e),
                    }
                } else {
                    warn!("soemthing went wrong");
                }
            }
        } else {
            warn!("{:?} is not a path or directory, .css files will be copied across but no .sccs compiling will happen", dir);
        }
        self
    }

    pub fn run(self) {
        info!("run");
        let mut global_articles: Vec<&liquid::Object> = Vec::new();
        let mut global_tags: HashMap<&str, Vec<&str>> = HashMap::new();
        let mut global_cats: HashMap<&str, Vec<&str>> = HashMap::new();

        let parser = liquid::ParserBuilder::with_stdlib()
            .partials(self.includes)
            .build()
            .unwrap();

        for obj in &self.articles {
            global_articles.push(&obj.config_liquid);
            for tag in &obj.config.tags {
                global_tags.entry(tag).or_insert(Vec::new()).push(&obj.url);
            }

            for cat in &obj.config.categories {
                global_cats.entry(cat).or_insert(Vec::new()).push(&obj.url);
            }
        }

        // One of the key things here is that articles is the raw content, that means it's nothing rendered yet
        // otherwise you would get weird things if you try to depend on something being already being renedered.
        // Although the cost of that is that we have to do the pre_render() step twice.
        let global = liquid::object!({
            "articles": global_articles,
            "tags": global_tags,
            "cats": global_cats,
        });

        if self.articles.is_empty() {
            error!("no articles found");
        }

        info!("layouts: {:?}", self.layouts);

        let mut errors: HashMap<String, Vec<String>> = HashMap::new();
        let mut i = 0;
        for art in self.articles {
            //TODO: make this be the url
            let mut output_path = self.output.clone();
            output_path.push(PathBuf::from(&art.url));
            info!("writing to {:?}", output_path);

            match &art.true_render(&global, &parser) {
                Ok(output) => {
                    info!("success");
                    let mut file = File::create(output_path).unwrap();
                    file.write_all(output.as_bytes()).unwrap();
                }
                Err(e) => match e {
                    error::CustomError::LiquidError(error) => {
                        if !error.contains("from: {% include") {
                            error!("{}file:\n   {}\n", error, self.article_paths[i]);
                        } else {
                            errors
                                .entry(format!("Template {}", error))
                                .or_insert(Vec::new())
                                .push(self.article_paths[i].clone());
                        }
                    }

                    error::CustomError::IOError(_) => {}
                },
            }

            i += 1;
        }

        if !errors.is_empty() {
            for (error, affected) in errors {
                if self.backtrace {
                    error!(
                        "{}files that use this template:\n   {}\n",
                        parse_backtrace(&error, &self.includes_paths),
                        affected.join(", ")
                    );
                } else {
                    error!(
                        "{}files that use this template:\n   {}\n",
                        error,
                        affected.join(", ")
                    );
                }
            }
        }
    }
}


/// provides file path for liquid include errors
/// note: getting location of the include error in files will be even more messy
fn parse_backtrace<'a>(error: &str, templates: &HashMap<String, String>) -> String {
    let mut inside_include = false;
    let mut msg = "".to_string();
    for line in error.split("\n") {
        if line.starts_with("from: {% include") {
            inside_include = true;
            msg += line;
        } else if inside_include && line.starts_with("    \"") {

            if let Some(index) = line[5..].find('"') {
                if let Some(path) = templates.get(&line[5..5+index]) {
                    msg += &format!("{}\n    {} = {}", line,&line[5..5+index], path);
                }
            } else {
                msg += line;
            }
        } else if !inside_include && line == "\twith:" {
            inside_include = false;
            msg += line;
        }else{
            msg += line;
        } 
        msg += "\n";
    }

    msg
}
