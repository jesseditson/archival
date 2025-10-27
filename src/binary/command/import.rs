use super::BinaryCommand;
use crate::{
    binary::ExitStatus,
    events::{AddChildEvent, AddObjectEvent, ArchivalEvent, ArchivalEventResponse, EditFieldEvent},
    file_system_stdlib,
    object::{ObjectEntry, ValuePath},
    Archival, FieldValue, FileSystemAPI, ObjectDefinition,
};
use clap::{arg, value_parser, ArgMatches};
use indicatif::{ProgressBar, ProgressStyle};
use liquid::model::ArrayView;
use std::{
    collections::HashMap,
    fmt::Debug,
    fs::{self, File},
    io::{self, BufReader, Read},
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Arc},
};
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum ImportError {
    #[error("file {0} doesn't exist")]
    FileNotExists(PathBuf),
    #[error("either --format or a file path must be provided")]
    FormatOrFileRequired,
    #[error("no file extension or format provided")]
    NoExtension,
    #[error("invalid object filename '{0}'")]
    InvalidObjectFilename(PathBuf),
    #[error("object {0} of type {1} does not exist. found {2:?}")]
    ObjectNotExists(String, String, Option<Vec<String>>),
    #[error("invalid object path '{0}'")]
    InvalidObjectPath(PathBuf),
    #[error("invalid object type '{0}'")]
    InvalidObjectType(String),
    #[error("invalid field {0}")]
    InvalidField(String),
    #[error("child name is required when importing to an object file")]
    NoChild,
    #[error("child type {0} not found in specified object type")]
    InvalidChild(String),
    #[error("--name is required when batch importing objects")]
    MissingNameField,
    #[error("failed parsing file {0}")]
    ParseError(String),
    #[error("failed writing {0}/{1}: {2}")]
    WriteError(String, String, String),
    #[error("couldn't find specified name field {0} in {1:?}")]
    MissingName(String, HashMap<String, String>),
}

#[derive(Debug, Clone)]
pub struct FieldMap {
    pub from: String,
    pub to: String,
}
impl From<&str> for FieldMap {
    fn from(value: &str) -> Self {
        let parts = value.split(':');
        let mut fm = Self {
            from: "".to_string(),
            to: "".to_string(),
        };
        for (i, v) in parts.into_iter().enumerate() {
            match i {
                0 => v.clone_into(&mut fm.from),
                1 => v.clone_into(&mut fm.to),
                _ => panic!("Invalid map value - too many arguments"),
            }
        }
        if fm.from.is_empty() {
            panic!("Invalid map value - missing source")
        } else if fm.to.is_empty() {
            panic!("Invalid map value - missing destination")
        }
        fm
    }
}

#[derive(Debug, Clone)]
enum ImportFormat {
    #[cfg(feature = "import-csv")]
    Csv,
    Json,
}

impl From<&str> for ImportFormat {
    fn from(value: &str) -> Self {
        match &value.to_lowercase()[..] {
            #[cfg(feature = "import-csv")]
            "csv" => Self::Csv,
            "json" => Self::Json,
            _ => panic!("Unsupported format {}", value),
        }
    }
}

impl ImportFormat {
    #[cfg(feature = "import-csv")]
    fn parse_csv<R: Read>(
        input: &mut BufReader<R>,
    ) -> Result<Vec<HashMap<String, String>>, ImportError> {
        let mut output = vec![];
        let mut rdr = csv::ReaderBuilder::new()
            .delimiter(b',')
            .flexible(true)
            .has_headers(true)
            .quoting(true)
            .from_reader(input);
        let headers = rdr
            .headers()
            .map_err(|e| ImportError::ParseError(e.to_string()))?
            .clone();
        for result in rdr.records() {
            let mut record = HashMap::new();
            let s_record = result.map_err(|e| ImportError::ParseError(e.to_string()))?;
            for (idx, hn) in headers.into_iter().enumerate() {
                if let Some(v) = s_record.get(idx).map(|s| s.to_owned()) {
                    record.insert(hn.to_string(), v);
                }
            }
            output.push(record)
        }
        Ok(output)
    }
    fn parse_json<R: Read>(
        input: &mut BufReader<R>,
    ) -> Result<Vec<HashMap<String, String>>, ImportError> {
        let json = io::read_to_string(input).map_err(|e| ImportError::ParseError(e.to_string()))?;
        let map: Vec<HashMap<String, String>> = serde_json::from_str(&json).unwrap();
        Ok(map)
    }
    pub fn parse<R: Read>(
        &self,
        input: &mut BufReader<R>,
    ) -> Result<Vec<HashMap<String, String>>, ImportError> {
        match self {
            #[cfg(feature = "import-csv")]
            ImportFormat::Csv => Self::parse_csv(input),
            ImportFormat::Json => Self::parse_json(input),
        }
    }
}

pub struct Command {}
impl BinaryCommand for Command {
    fn name(&self) -> &str {
        "import"
    }
    fn cli(&self, cmd: clap::Command) -> clap::Command {
        cmd.about("import data into archival site objects")
            .arg(
                arg!([object] "The object type if generating objects, or an object file to import to (e.g. post/one.toml) if importing to a child list.")
                    .required(true)
                    .value_parser(value_parser!(PathBuf)),
            )
            .arg(
                arg!(-c --child <child> "If importing to a child list, this is the name of the children to import to.")
                    .value_parser(value_parser!(ValuePath)),
            )
            .arg(
                arg!(-n --name <field_name> "If generating objects, the data field to use to generate object file names.")
                    .value_parser(value_parser!(String)),
            )
            .arg(
                arg!(-f --format <"csv|json"> "The format of the source data. Inferred from the file extension if importing from a file.")
                    .value_parser(value_parser!(ImportFormat)),
            )
            .arg(
                arg!(-m --map <"from:to"> ... "map a source field name to a destination field name.")
                    .value_parser(value_parser!(FieldMap)),
            )
            .arg(
                arg!([file] "The file containing data to import. If not provided, will read from stdout (and --format is required).")
                    .value_parser(value_parser!(PathBuf)),
            )
    }
    fn handler(
        &self,
        build_dir: &Path,
        args: &ArgMatches,
        _quit: Arc<AtomicBool>,
    ) -> Result<crate::binary::ExitStatus, Box<dyn std::error::Error>> {
        // Fail fast if file doesn't exist
        let file_path = args.get_one::<PathBuf>("file");
        let file_format = if let Some(file_format) = args.get_one::<ImportFormat>("format") {
            file_format.to_owned()
        } else if let Some(file_path) = file_path {
            let metadata = fs::metadata(file_path)
                .map_err(|_| ImportError::FileNotExists(file_path.to_owned()))?;
            if !metadata.is_file() {
                return Err(ImportError::FileNotExists(file_path.to_owned()).into());
            }
            ImportFormat::from(
                file_path
                    .extension()
                    .ok_or(ImportError::NoExtension)?
                    .to_string_lossy()
                    .as_ref(),
            )
        } else {
            return Err(ImportError::FormatOrFileRequired.into());
        };
        let object = args.get_one::<PathBuf>("object").unwrap();
        // TODO: handle root objects?
        let (object_name, object_type) = if object.extension().is_some() {
            let object_name = object
                .with_extension("")
                .file_name()
                .ok_or_else(|| ImportError::InvalidObjectFilename(object.to_owned()))?
                .to_string_lossy()
                .to_string();
            let object_type = object
                .parent()
                .ok_or_else(|| ImportError::InvalidObjectPath(object.to_owned()))?
                .to_string_lossy()
                .to_string();
            (Some(object_name), object_type)
        } else {
            (None, object.to_string_lossy().to_string())
        };
        // Set up an archival site to make sure we're able to modify fields
        let fs = file_system_stdlib::NativeFileSystem::new(build_dir);
        let archival = Archival::new(fs)?;
        // Find the specified object definition
        let obj_def = archival
            .site
            .object_definitions
            .get(&object_type)
            .ok_or_else(|| ImportError::InvalidObjectType(object_type.to_owned()))?;
        let child_name = args.get_one::<ValuePath>("child");
        let child_def = if let Some(child_path) = child_name {
            // Find the specified child, if present
            Some(
                child_path
                    .get_definition(obj_def)
                    .map_err(|_| ImportError::InvalidField(child_path.to_string()))?,
            )
        } else {
            None
        };
        let name_field = args.get_one::<String>("name");
        let import_name = if let Some(object_name) = object_name {
            // If provided, make sure that the specified object exists, and that
            // we were also given a child name
            if !archival.object_exists(&object_type, &object_name)? {
                return Err(ImportError::ObjectNotExists(
                    object_name.to_string(),
                    object_type.to_string(),
                    archival.get_objects()?.get(&object_type).map(|o| match o {
                        ObjectEntry::List(l) => l.iter().map(|o| o.filename.to_owned()).collect(),
                        ObjectEntry::Object(o) => vec![o.filename.to_owned()],
                    }),
                )
                .into());
            }
            if child_name.is_none() {
                return Err(ImportError::NoChild.into());
            }
            if child_def.is_none() {
                return Err(ImportError::InvalidChild(child_name.unwrap().to_string()).into());
            }
            ImportName::File(object_name)
        } else {
            // Otherwise make sure we were provided with a field name to
            // generate filenames from.
            if let Some(name_field) = name_field {
                ImportName::Field(name_field.to_owned())
            } else {
                return Err(ImportError::MissingNameField.into());
            }
        };
        let field_map_args = args.get_many::<FieldMap>("map");
        let mut field_map = HashMap::new();
        if let Some(fm) = field_map_args {
            for fm in fm {
                field_map.insert(fm.to.to_owned(), fm.from.to_owned());
            }
        }
        let mapped_type = if let Some(child_def) = child_def {
            child_def
        } else {
            obj_def
        };
        let f = if let Some(fp) = file_path {
            File::open(fp)?
        } else {
            todo!("stdin reading not implemented yet")
        };
        let bar = ProgressBar::new_spinner();
        bar.set_style(ProgressStyle::with_template("{msg} {spinner}").unwrap());
        let mut reader = BufReader::new(f);
        Command::parse(
            &mut reader,
            &object_type,
            field_map,
            import_name,
            if let Some(child_name) = child_name {
                child_name.to_owned()
            } else {
                ValuePath::default()
            },
            mapped_type,
            &file_format,
            &archival,
            |msg, p, t| {
                bar.set_message(msg.to_string());
                bar.set_length(t);
                bar.set_position(p);
            },
        )?;
        Ok(ExitStatus::Ok)
    }
}

#[derive(Debug)]
enum ImportName {
    File(String),
    Field(String),
}

impl Command {
    #[allow(clippy::too_many_arguments)]
    fn parse<R: Read, F: FileSystemAPI + Debug + Clone>(
        reader: &mut BufReader<R>,
        object: &str,
        field_map: HashMap<String, String>,
        import_name: ImportName,
        root_path: ValuePath,
        mapped_type: &ObjectDefinition,
        file_format: &ImportFormat,
        archival: &Archival<F>,
        progress: impl Fn(&str, u64, u64),
    ) -> Result<(), ImportError> {
        // Generate a list of rows from our input data
        progress("parsing file...", 0, 0);
        let inverted_field_map: HashMap<&String, &String> =
            field_map.iter().map(|(k, v)| (v, k)).collect();
        let parsed = file_format.parse(reader)?;
        if let ImportName::Field(f) = &import_name {
            progress("creating object...", 0, 0);
            archival
                .send_event(
                    ArchivalEvent::AddObject(AddObjectEvent {
                        object: object.to_string(),
                        filename: f.to_string(),
                        order: Some(0.),
                        values: vec![],
                    }),
                    None,
                )
                .map_err(|e| {
                    ImportError::WriteError(object.to_string(), f.to_string(), e.to_string())
                })?;
        }
        let mut idx = 0;
        let total = parsed.size();
        for row in parsed {
            idx += 1;
            progress(
                &format!("importing {} of {} rows...", idx, total),
                idx as u64,
                total as u64,
            );
            let mut current_path = root_path.clone();
            let filename = match &import_name {
                ImportName::File(f) => {
                    let r = archival
                        .send_event(
                            ArchivalEvent::AddChild(AddChildEvent {
                                object: object.to_string(),
                                filename: f.to_string(),
                                path: current_path.to_owned(),
                                values: vec![],
                                index: None,
                            }),
                            None,
                        )
                        .map_err(|e| {
                            ImportError::WriteError(
                                object.to_string(),
                                f.to_string(),
                                e.to_string(),
                            )
                        })?;
                    if let ArchivalEventResponse::Index(i) = r {
                        current_path = current_path.append(ValuePath::index(i));
                    } else {
                        panic!("archival did not return an index for an inserted child");
                    }
                    f.to_owned()
                }
                ImportName::Field(f) => {
                    let file_name = row
                        .get(f)
                        .ok_or(ImportError::MissingName(f.to_owned(), row.clone()))
                        .map(|s| s.split_whitespace().collect::<Vec<_>>().join("-"))?;
                    archival
                        .send_event(
                            ArchivalEvent::AddObject(AddObjectEvent {
                                object: object.to_string(),
                                filename: file_name.to_string(),
                                order: Some(0.),
                                values: vec![],
                            }),
                            None,
                        )
                        .map_err(|e| {
                            ImportError::WriteError(
                                object.to_string(),
                                f.to_string(),
                                e.to_string(),
                            )
                        })?;
                    file_name
                }
            };
            let mut unused_cols = row.clone();
            for (name, field_type) in &mapped_type.fields {
                let from_name = if let Some(mapped) = field_map.get(name) {
                    mapped
                } else {
                    name
                };
                if let Some(value) = row.get(from_name) {
                    // Validate type
                    let value = FieldValue::from_string(name, field_type, value.to_string())
                        .map_err(|e| ImportError::ParseError(e.to_string()))?;
                    archival
                        .send_event(
                            ArchivalEvent::EditField(EditFieldEvent {
                                object: object.to_string(),
                                filename: filename.to_string(),
                                path: current_path.clone(),
                                value: Some(value),
                                field: name.to_string(),
                                source: None,
                            }),
                            None,
                        )
                        .map_err(|e| {
                            ImportError::WriteError(
                                object.to_string(),
                                filename.to_string(),
                                e.to_string(),
                            )
                        })?;
                    unused_cols.remove(from_name);
                } else {
                    println!("field '{}' not found in row: {:?}", from_name, row);
                }
            }
            // We support pathing in csv column names, so if the name begins
            // with a child name, insert it.
            for (name, value) in unused_cols {
                let import_name = if let Some(import_name) = inverted_field_map.get(&name) {
                    import_name
                } else {
                    &name
                };
                let mut col_path = ValuePath::from_string(import_name);
                // The final component is the field
                let col_field = col_path.pop();
                if col_field.is_none() {
                    continue;
                }
                if let Ok(found_type) = col_path.get_definition(mapped_type) {
                    let col_field = col_field.unwrap().to_string().trim().to_string();
                    if let Some(field_type) = found_type.fields.get(&col_field) {
                        // Validate type
                        let value = FieldValue::from_string(&name, field_type, value.to_string())
                            .map_err(|e| ImportError::ParseError(e.to_string()))?;
                        archival
                            .send_event(
                                ArchivalEvent::EditField(EditFieldEvent {
                                    object: object.to_string(),
                                    filename: filename.to_string(),
                                    path: current_path.clone().concat(col_path),
                                    value: Some(value),
                                    field: col_field,
                                    source: None,
                                }),
                                None,
                            )
                            .map_err(|e| {
                                ImportError::WriteError(
                                    object.to_string(),
                                    filename.to_string(),
                                    e.to_string(),
                                )
                            })?;
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[cfg(feature = "import-csv")]
mod csv_tests {

    use super::{Command, ImportFormat, ImportName};
    use crate::fields::DateTime;
    use crate::object::ValuePath;
    use crate::{unpack_zip, FieldValue, MemoryFileSystem};
    use std::collections::HashMap;
    use std::error::Error;
    use std::io::BufReader;

    #[test]
    fn parse_csv_data_to_files() -> Result<(), Box<dyn Error>> {
        let csv_data = "some_number,title,content,date\n1,hello, string,01/21/1987";
        let mut reader = BufReader::new(csv_data.as_bytes());
        let mut fs = MemoryFileSystem::default();
        let zip = include_bytes!("../../../tests/fixtures/archival-website.zip");
        unpack_zip(zip.to_vec(), &mut fs)?;
        let archival = crate::Archival::new_with_upload_prefix(fs, "")?;
        let obj_def = archival.site.object_definitions.get("post").unwrap();
        Command::parse(
            &mut reader,
            "post",
            HashMap::new(),
            ImportName::Field("title".to_string()),
            ValuePath::default(),
            obj_def,
            &ImportFormat::Csv,
            &archival,
            |m, p, t| println!("{} ({}/{})", m, p, t),
        )?;
        let objects = archival.get_objects()?;
        let posts = objects.get("post").unwrap();
        let mut found = false;
        for post in posts {
            if post.filename == "hello" {
                found = true;
                assert_eq!(
                    post.values.get("some_number"),
                    Some(&crate::FieldValue::Number(1.0))
                );
                assert_eq!(
                    post.values.get("title"),
                    Some(&crate::FieldValue::String("hello".to_string()))
                );
                assert_eq!(
                    post.values.get("content"),
                    Some(&crate::FieldValue::Markdown(" string".to_string()))
                );
                let expected_date =
                    crate::FieldValue::Date(DateTime::from_ymd(1987, 1, 21)).to_string();
                assert_eq!(post.values.get("date").unwrap().to_string(), expected_date);
            }
        }
        assert!(found);
        Ok(())
    }

    #[test]
    // #[traced_test]
    fn parse_csv_data_to_children() -> Result<(), Box<dyn Error>> {
        let csv_data = "number,name,renamed_date\n128,hello,01/21/1987";
        let mut reader = BufReader::new(csv_data.as_bytes());
        let mut fs = MemoryFileSystem::default();
        let zip = include_bytes!("../../../tests/fixtures/archival-website.zip");
        unpack_zip(zip.to_vec(), &mut fs)?;
        let archival = crate::Archival::new_with_upload_prefix(fs, "")?;
        let obj_def = archival.site.object_definitions.get("childlist").unwrap();
        Command::parse(
            &mut reader,
            "childlist",
            HashMap::from([("date".to_string(), "renamed_date".to_string())]),
            ImportName::File("has-list".to_string()),
            ValuePath::from_string("list"),
            obj_def.children.get("list").unwrap(),
            &ImportFormat::Csv,
            &archival,
            |m, p, t| println!("{} ({}/{})", m, p, t),
        )?;
        let objects = archival.get_objects()?;
        let children = objects.get("childlist").unwrap();
        let mut found = false;
        for children in children {
            if children.filename == "has-list" {
                found = true;
                let list = ValuePath::from_string("list")
                    .get_in_object(children)
                    .unwrap();
                println!("LIST: {:?}", list);
                assert!(matches!(list, FieldValue::Objects(_)));
                if let FieldValue::Objects(list) = list {
                    let existing = list.iter().find(|e| {
                        println!("LIST ITEM: {:?}", e);
                        e.get("name")
                            .map(|n| {
                                println!("FIELD: {:?}", n);
                                n.to_string() == "existing"
                            })
                            .unwrap_or(false)
                    });
                    assert!(existing.is_some());
                    let new = list.iter().find(|e| {
                        e.get("name")
                            .map(|n| n.to_string() == "hello")
                            .unwrap_or(false)
                    });
                    assert!(new.is_some());
                    let new = new.unwrap();
                    assert_eq!(new.get("number"), Some(&crate::FieldValue::Number(128.0)));
                    assert_eq!(
                        new.get("name"),
                        Some(&crate::FieldValue::String("hello".to_string()))
                    );
                    let expected_date =
                        crate::FieldValue::Date(DateTime::from_ymd(1987, 1, 21)).to_string();
                    assert_eq!(new.get("date").unwrap().to_string(), expected_date);
                }
            }
        }
        assert!(found);
        Ok(())
    }
}
