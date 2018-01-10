extern crate env_logger;
extern crate glob;
#[macro_use]
extern crate log;
#[macro_use]
extern crate tera;
extern crate csv;
extern crate regex;

#[macro_use]
extern crate serde_derive;
extern crate serde_yaml;

#[macro_use]
extern crate failure;
#[macro_use]
extern crate failure_derive;

use std::env;
use tera::Tera;
use tera::Context;
use std::io::prelude::*;
use std::fs::{self, File, OpenOptions};
use glob::glob;
use serde_yaml::Value;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::iter::FromIterator;
use regex::Regex;

pub static DEFAULT_PATH: &str = "";
pub static DEFAULT_GLOB: &str = "**/*";
pub static DEFAULT_TPL_EXTENSION: &str = ".dmt.tpl";
pub static DEFAULT_MPTPL_EXTENSION: &str = ".dmt.mtpl";
pub static DEFAULT_CTX_EXTENSION: &str = ".dmt.ctx";
pub static DEFAULT_CSV_EXTENSION: &str = ".dmt.csv";

pub static DEFAULT_ENV_PREFIX: &str = "";
pub static DEFAULT_CTX_PREFIX: &str = "";
pub static DEFAULT_CSV_PREFIX: &str = "";
pub static DEFAULT_LCL_PREFIX: &str = "";
pub static DEFAULT_DEF_PREFIX: &str = "";

pub static DEFAULT_VAR_FILE: &str = "default.yml";
pub static LOCAL_VAR_FILE: &str = "local.yml";

use std::str::FromStr;
use failure::Error;
use failure::ResultExt;
use failure::err_msg;

#[derive(Copy, Clone)]
enum VariableMode {
    MiniMode,
    DMTMode,
}

#[derive(Debug, Deserialize)]
pub struct MultipartTemplate {
    preamble: String,
    glob: String,
    postfix: String,
    indent: u8,
}

pub struct TemplateRenderer<'ren> {
    mode: VariableMode,
    context: Box<Context>,
    target_tpl_glob: &'ren str,
    base_path: &'ren str,
    target_extension: &'ren str,
    target_mp_extension: &'ren str,
    data_sources: Vec<Box<DataSource>>,
}

trait DataSource {
    fn load(&self) -> Result<Context, Error>;
}

struct EnvironmentDatasource<'res> {
    prefix: &'res str,
}

impl<'res> DataSource for EnvironmentDatasource<'res> {
    fn load(&self) -> Result<Context, Error> {
        let mut new_context = Context::new();

        //Load environment variables
        debug!("adding environment variables to context");

        let mut evars = Vec::new();

        for (key, value) in env::vars() {
            let value = Value::String(value);
            new_context.add(&[self.prefix, &key].concat(), &value);

            evars.push(key);
        }

        debug!("added env variables  : {}", evars.join(","));

        Ok(new_context)
    }
}

struct YamlFileDatasource<'res> {
    mode: VariableMode,
    target: PathBuf,
    prefix: &'res str,
}

impl<'res> DataSource for YamlFileDatasource<'res> {
    fn load(&self) -> Result<Context, Error> {
        let mut file = match File::open(&self.target) {
            Ok(file) => file,
            Err(e) => {
                return {
                    let err = format_err!("could not open: {}", self.target.to_str().unwrap());
                    Err(err)
                }
            }
        };
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        let deserialized_data: Value = serde_yaml::from_str(&contents)?;

        let mut new_context = Context::new();

        let mapped = deserialized_data.as_mapping().map(|hm| {
            let root = match self.mode {
                VariableMode::DMTMode => {
                    debug!("dmt mode'");
                    Some(hm)
                }
                VariableMode::MiniMode => {
                    debug!("legacy mode'");
                    debug!("setting new root, subkeyed to 'variables'");
                    hm[&Value::String(String::from("variables"))].as_mapping()
                }
            };

            root.map(|hm| {
                for (key, value) in hm {
                    match key.as_str() {
                        Some(k) => {
                            debug!("adding variable     : {} -> {:?}", k, value);
                            new_context.add(&[self.prefix, k].concat(), value);
                        }
                        None => {
                            eprintln!("unusable key {:?}", key);
                        }
                    };
                }
            });
        });

        Ok(new_context)
    }
}

struct CSVDatasource<'res> {
    key: Option<&'res str>,
    target: PathBuf,
    prefix: &'res str,
}

type Record = HashMap<String, String>;

impl<'res> DataSource for CSVDatasource<'res> {
    fn load(&self) -> Result<Context, Error> {
        let mut new_context = Context::new();

        let mut searchkey: Option<String> = None;
        let mut rootkey: Option<String> = None;

        if let Some(path_as_str) = self.target.to_str().map(|s| s) {
            if path_as_str.ends_with(DEFAULT_CSV_EXTENSION) {
                let key = PathBuf::from(path_as_str.replace(DEFAULT_CSV_EXTENSION, ""));

                if let Some(name) = key.file_name() {
                    if let Some(name_as_str) = name.to_str() {
                        let v = Vec::from_iter(name_as_str.split('.'));
                        if v.len() == 2 {
                            searchkey = Some(String::from(v[1]));
                            rootkey = Some(String::from(v[0]));
                        } else if v.len() == 1 {
                            searchkey = Some(String::from(v[0]));
                            rootkey = Some(String::from(v[0]));
                        }
                    }
                }
            }
        }

        let simple_tree = searchkey == rootkey;
        let mut vec_store = Vec::new();
        let mut hash_store = HashMap::new();

        if let Some(searchkey) = searchkey {
            let mut file = File::open(&self.target)?;
            let mut rdr = csv::ReaderBuilder::new()
                .has_headers(true)
                .from_reader(file);

            for result in rdr.deserialize() {
                let record: Record = result?;
                if record.contains_key(&searchkey) {
                    let key = record.get(&searchkey);
                    if let Some(key_exists) = key {
                        let mut new_record = record.clone();
                        new_record.remove(&searchkey);
                        hash_store.insert(key_exists.clone(), new_record);
                    }
                } else {
                    vec_store.push(record);
                }
            }

            if let Some(rootkey) = rootkey {
                if vec_store.len() > 0 {
                    new_context.add(&rootkey, &vec_store);
                } else {
                    new_context.add(&rootkey, &hash_store);
                }
            }
        }

        Ok(new_context)
    }
}

fn build_search_path(base: &str, glob: &str, ext: &str) -> Result<String, Error> {
    let full_filename_pattern = [glob, ext].concat();
    debug!("full_pattern        : {:?}", full_filename_pattern);

    let mut extended_pattern = PathBuf::new();
    extended_pattern.push(base);
    extended_pattern.push(full_filename_pattern);

    let extended_pattern = extended_pattern
        .to_str()
        .ok_or(err_msg("str conversion failed"))?;

    debug!("extended_pattern    : {:?}", extended_pattern);

    Ok(String::from(extended_pattern))
}

impl<'ren> TemplateRenderer<'ren> {
    fn new(
        base_path: &'ren str,
        target_glob: &'ren str,
        target_extension: &'ren str,
        target_mp_extension: &'ren str,
    ) -> TemplateRenderer<'ren> {
        env_logger::init();

        let context = Context::new();

        TemplateRenderer {
            mode: VariableMode::DMTMode,
            context: Box::new(context),
            target_tpl_glob: target_glob,
            base_path,
            target_extension,
            target_mp_extension,
            data_sources: Vec::new(),
        }
    }

    pub fn default() -> TemplateRenderer<'ren> {
        let base_path = DEFAULT_PATH;
        let target_glob = DEFAULT_GLOB;
        let target_extension = DEFAULT_TPL_EXTENSION;
        let target_mp_extension = DEFAULT_MPTPL_EXTENSION;

        let mut n = TemplateRenderer::new(
            base_path,
            target_glob,
            target_extension,
            target_mp_extension,
        );

        n.add_all_datasources();

        n
    }

    fn add_datasource<D: DataSource + 'static>(&mut self, name: &str, source: D) {
        debug!("adding: {}", name);
        self.data_sources.push(Box::new(source));
    }

    fn set_mode(&mut self, mode: VariableMode) {
        self.target_extension = ".orig.tpl";
        self.mode = mode;
    }

    fn add_all_datasources(&mut self) -> Result<(), Error> {
        debug!("attempt to add all datasources");

        self.add_def_datasource(DEFAULT_DEF_PREFIX)?;
        self.add_ctx_datasource(DEFAULT_CTX_PREFIX)?;
        self.add_csv_datasource(DEFAULT_CSV_PREFIX)?;
        self.add_lcl_datasource(DEFAULT_LCL_PREFIX)?;
        self.add_env_datasource(DEFAULT_ENV_PREFIX)?;

        Ok(())
    }

    fn add_yml_datasource(&mut self, prefix: &'static str, target: &'ren str) -> Result<(), Error> {
        debug!("adding custom file variables to context ({})", target);
        let mut path = PathBuf::new();
        path.push(self.base_path);
        path.push(target);
        debug!("target_custom_yml   : {:?}", path);

        let mode = self.mode.clone();
        self.add_datasource(
            &["yml:", target].concat(),
            YamlFileDatasource {
                mode,
                target: path,
                prefix,
            },
        );
        Ok(())
    }

    fn add_lcl_datasource(&mut self, prefix: &'static str) -> Result<(), Error> {
        debug!("adding legacy file variables to context (local.yml)");
        let mut path = PathBuf::new();
        path.push(self.base_path);
        path.push(LOCAL_VAR_FILE);
        debug!("target_local_yml   : {:?}", path);

        let mode = self.mode.clone();
        self.add_datasource(
            &"local.yml",
            YamlFileDatasource {
                mode,
                target: path,
                prefix,
            },
        );
        Ok(())
    }

    fn add_def_datasource(&mut self, prefix: &'static str) -> Result<(), Error> {
        debug!("adding legacy file variables to context (default.yml)");
        let mut path = PathBuf::new();
        path.push(self.base_path);
        path.push(DEFAULT_VAR_FILE);
        debug!("target_default_yml  : {:?}", path);

        let mode = self.mode.clone();
        self.add_datasource(
            &"default.yml",
            YamlFileDatasource {
                mode,
                target: path,
                prefix,
            },
        );
        Ok(())
    }

    fn add_env_datasource(&mut self, prefix: &'static str) -> Result<(), Error> {
        debug!("adding environment datasource");
        self.add_datasource(&"environment", EnvironmentDatasource { prefix });
        Ok(())
    }

    fn add_csv_datasource(&mut self, prefix: &'static str) -> Result<(), Error> {
        debug!("processing csv files");
        debug!("base_path           : {:?}", self.base_path);
        debug!("target_glob         : {:?}", DEFAULT_GLOB);
        debug!("target_extension    : {:?}", DEFAULT_CSV_EXTENSION);

        let extended_pattern =
            build_search_path(self.base_path, DEFAULT_GLOB, DEFAULT_CSV_EXTENSION)?;

        for entry in glob(&extended_pattern).expect("Failed to read glob pattern") {
            match entry {
                Ok(path) => {
                    debug!("adding csv file variables to context");
                    debug!("ctx file            : {:?}", path);

                    let mode = self.mode.clone();
                    self.add_datasource(
                        &["csv:", path.to_str().unwrap()].concat(),
                        CSVDatasource {
                            key: None,
                            target: path,
                            prefix,
                        },
                    );
                }
                Err(e) => println!("ERRROR {:?}", e),
            }
        }
        Ok(())
    }

    fn add_ctx_datasource(&mut self, prefix: &'static str) -> Result<(), Error> {
        debug!("processing context files");
        debug!("base_path           : {:?}", self.base_path);
        debug!("target_glob         : {:?}", DEFAULT_GLOB);
        debug!("target_extension    : {:?}", DEFAULT_CTX_EXTENSION);

        let extended_pattern =
            build_search_path(self.base_path, DEFAULT_GLOB, DEFAULT_CTX_EXTENSION)?;

        for entry in glob(&extended_pattern).expect("Failed to read glob pattern") {
            match entry {
                Ok(path) => {
                    debug!("adding ctx file variables to context");
                    debug!("ctx file            : {:?}", path);

                    let mode = self.mode.clone();
                    self.add_datasource(
                        &["ctx:", path.to_str().unwrap()].concat(),
                        YamlFileDatasource {
                            mode,
                            target: path,
                            prefix,
                        },
                    );
                    //                    self.read_yml_file(prefix, path);
                }
                Err(e) => println!("ERRROR {:?}", e),
            }
        }
        Ok(())
    }

    fn refresh_contexts(&mut self) -> Result<(), Error> {
        let mut new_context = Context::new();

        debug!("refreshing {} datasources", &self.data_sources.len());

        for s in &self.data_sources {
            let c = s.load(); //.chain_err(|| "yaml deserializing error")?;
            match c {
                Ok(c) => {
                    debug!("Add context: {:#?}", c);
                    new_context.extend(c)
                }
                Err(_) => (),
            }
        }

        self.context = Box::new(new_context);

        Ok(())
    }

    pub fn render_default(&mut self) -> Result<(), Error> {
        debug!("refreshing datasources");
        self.refresh_contexts()?;

        debug!("processing templates");
        debug!("base_path           : {:?}", self.base_path);
        debug!("target_tpl_glob     : {:?}", self.target_tpl_glob);
        debug!("target_extension    : {:?}", self.target_extension);

        let extended_pattern =
            build_search_path(self.base_path, self.target_tpl_glob, self.target_extension)?;

        let tera = match Tera::new(&extended_pattern) {
            Ok(tera) => tera,
            Err(e) => {
                eprintln!("{}, ", e);
                for e in e.iter().skip(1) {
                    eprintln!("{}", e);
                }
                ::std::process::exit(1);
            }
        };

        for (path, _) in &tera.templates {
            let template_full_path = Path::new(&path);
            let template_directory = template_full_path
                .parent()
                .ok_or(err_msg("string conversion failed for path"))?;
            let template_filename = template_full_path
                .file_name()
                .ok_or(err_msg("string conversion failed for path"))?;

            let target_filename = template_filename
                .to_str()
                .ok_or(err_msg("string conversion failed for path"))?
                .replace(self.target_extension, "");

            let mut target_full_path = PathBuf::new();
            target_full_path.push(self.base_path);
            target_full_path.push(&template_directory);
            target_full_path.push(&target_filename);

            debug!("processing new template");
            debug!("template_filename   : {:?}", template_filename);
            debug!("template_full_path  : {:?}", template_full_path.display());
            debug!("template_directory  : {:?}", template_directory);
            debug!("target_filename     : {:?}", target_filename);
            debug!("target_full_path    : {:?}", target_full_path);

            let target_full_path_str = target_full_path
                .to_str()
                .ok_or(err_msg("string conversion failed for path"))?;

            if target_full_path_str.ends_with(self.target_extension) {
                return Err(err_msg(
                    "target still contains the template externsion, aborting",
                ));
            }

            let out = match tera.render(&template_full_path.to_str().unwrap_or(""), &self.context) {
                Ok(out) => out,
                Err(e) => {
                    eprintln!("{}, ", e);
                    for e in e.iter().skip(1) {
                        eprintln!("{}", e);
                    }
                    ::std::process::exit(1);
                }
            };

            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(true)
                .open(&target_full_path)?;

            file.write_all(out.as_bytes())?;
        }

        debug!("finished rendering all templates");

        Ok(())
    }

    pub fn render_multipart(&mut self) -> Result<(), Error> {
        debug!("refreshing datasources");
        self.refresh_contexts()?;

        debug!("processing multipart templates");
        debug!("base_path           : {:?}", self.base_path);
        debug!("target_tpl_glob     : {:?}", self.target_tpl_glob);
        debug!("target_extension    : {:?}", self.target_extension);

        let extended_pattern = build_search_path(
            self.base_path,
            self.target_tpl_glob,
            self.target_mp_extension,
        )?;

        for entry in glob(&extended_pattern).expect("Failed to read glob pattern") {
            match entry {
                Ok(path) => {
                    debug!("found multipart template target");
                    debug!("mp tpl file         : {:?}", path);

                    let mut file = File::open(path).unwrap();
                    let mut contents = String::new();
                    file.read_to_string(&mut contents).unwrap();
                    //FIXME: transition to fs::read(path) when stable
                    let mpt: MultipartTemplate = serde_yaml::from_str(&contents).unwrap();

                    println!("{:#?}", mpt);
                    let base = self.base_path.clone();

                    let parts = build_search_path(&base, &mpt.glob, "")?;
                    debug!("mp tpl extended     : {:?}", parts);

                    let tera = match Tera::new(&parts) {
                        Ok(tera) => tera,
                        Err(e) => {
                            eprintln!("{}, ", e);
                            for e in e.iter().skip(1) {
                                eprintln!("{}", e);
                            }
                            ::std::process::exit(1);
                        }
                    };

                }
                Err(e) => println!("ERRROR {:?}", e),
            }
        }

        debug!("finished rendering all multipart templates");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use failure::Fail;
    use std::env;

    #[test]
    fn test_custom() {
        let test_string = "yes_for_custom\n";

        let extension = DEFAULT_TPL_EXTENSION;
        let pattern = DEFAULT_GLOB;
        let base_path = "tests/custom/";
        fs::remove_file("tests/custom/custom.out");

        let mut tr = TemplateRenderer::new(&base_path, pattern, extension, "");

        tr.add_all_datasources();
        tr.add_yml_datasource(DEFAULT_CTX_PREFIX, "custom.yml");

        match tr.render_default() {
            Err(e) => {
                eprintln!("{:?}", e);
                ::std::process::exit(1);
            }
            _ => (),
        }

        let mut file = File::open("tests/custom/custom.out").unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();

        assert_eq!(contents, test_string);
    }

    #[test]
    fn test_precendence() {
        env::set_var("ENV", "yes_for_env");
        env::set_var("priority", "env");
        let test_string = "priority? env";

        let extension = DEFAULT_TPL_EXTENSION;
        let pattern = DEFAULT_GLOB;
        let base_path = "tests/precedence/";
        fs::remove_file("tests/precedence/precedence.out");

        let mut tr = TemplateRenderer::new(&base_path, pattern, extension, "");

        tr.add_all_datasources();

        match tr.render_default() {
            Err(e) => {
                eprintln!("{:?}", e);
                ::std::process::exit(1);
            }
            _ => (),
        }

        let mut file = File::open("tests/precedence/precedence.out").unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        assert!(contents.contains(test_string));
    }

    #[test]
    fn new_renderer_pwd() {
        let pwd = ::std::env::var("PWD").unwrap();

        let extension = DEFAULT_TPL_EXTENSION;
        let pattern = DEFAULT_GLOB;
        let base_path = "tests/pwd/";
        fs::remove_file("tests/pwd/pwd.out");

        let mut tr = TemplateRenderer::new(&base_path, pattern, extension, "");

        tr.add_all_datasources();

        match tr.render_default() {
            Err(e) => {
                eprintln!("{:?}", e);
                ::std::process::exit(1);
            }
            _ => (),
        }

        let mut file = File::open("tests/pwd/pwd.out").unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        assert_eq!(contents, pwd);
    }

    #[test]
    fn complex_renderer() {
        let extension = DEFAULT_TPL_EXTENSION;
        let pattern = DEFAULT_GLOB;
        let base_path = "tests/complex/";

        let mut tr = TemplateRenderer::new(&base_path, pattern, extension, "");
        tr.add_all_datasources();

        match tr.render_default() {
            Err(e) => {
                eprintln!("{:?}", e);
                ::std::process::exit(1);
            }
            _ => (),
        }
    }

    #[test]
    fn multipart_renderer() {
        let extension = DEFAULT_TPL_EXTENSION;
        let mp_extension = DEFAULT_MPTPL_EXTENSION;
        let pattern = DEFAULT_GLOB;
        let base_path = "tests/multipart/";

        let mut tr = TemplateRenderer::new(&base_path, pattern, extension, mp_extension);
        tr.add_all_datasources();

        match tr.render_default() {
            Err(e) => {
                eprintln!("{:?}", e);
                ::std::process::exit(1);
            }
            _ => (),
        }

        match tr.render_multipart() {
            Err(e) => {
                eprintln!("{:?}", e);
                ::std::process::exit(1);
            }
            _ => (),
        }
    }

    #[test]
    fn test_basic_csv_renderer() {
        let extension = DEFAULT_TPL_EXTENSION;
        let pattern = DEFAULT_GLOB;
        let base_path = "tests/csv/";

        let mut tr = TemplateRenderer::new(&base_path, pattern, extension, "");
        tr.add_all_datasources();

        match tr.render_default() {
            Err(e) => {
                eprintln!("{:?}", e);
                ::std::process::exit(1);
            }
            _ => (),
        }
    }

    #[test]
    fn legacy_renderer() {
        let extension = DEFAULT_TPL_EXTENSION;
        let pattern = DEFAULT_GLOB;
        let base_path = "tests/legacy/";

        let mut tr = TemplateRenderer::new(&base_path, pattern, extension, "");
        tr.set_mode(VariableMode::MiniMode);
        tr.add_all_datasources();

        match tr.render_default() {
            Err(e) => {
                eprintln!("{:?}", e);
                ::std::process::exit(1);
            }
            _ => (),
        }
    }
}
