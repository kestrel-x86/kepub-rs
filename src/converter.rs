use std::{
    collections::HashMap,
    fs::{create_dir_all, read_dir, remove_dir_all, File},
    io::Write,
    path::PathBuf,
    process::Output,
};
use xmltree::{Element, EmitterConfig, XMLNode};

use zip::{
    write::{FileOptions, SimpleFileOptions},
    CompressionMethod, ZipArchive, ZipWriter,
};

use crate::{
    errors::{io_err, xml_err, ConverterError},
    lmnt::LMNT,
};

pub struct Converter {
    working_dir: PathBuf,
    write_config: EmitterConfig,
    working_dir_str: String,
}

impl Converter {
    /// Will fail if write access to tmp dir is not available
    pub fn new() -> Result<Self, std::io::Error> {
        let mut write_config = EmitterConfig::new();
        write_config.perform_indent = true;

        let (pb, s) = Self::get_tmp_dir()?;

        return Ok(Self {
            working_dir: pb,
            working_dir_str: s,
            write_config: write_config,
        });
    }

    // Creates a tmp dir
    fn get_tmp_dir() -> Result<(PathBuf, String), std::io::Error> {
        let td = std::env::temp_dir().join("kepub-rs-conv");
        let s = match td.to_str() {
            Some(s) => s.to_string(),
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Could not get valid path to temporary directory",
                ))
            }
        };
        let _ = remove_dir_all(&td);

        create_dir_all(&td)?;
        println!("{:?}", td);
        return Ok((td, s));
    }

    pub fn convert(
        &self,
        epub: &mut ZipArchive<File>,
        out_path: &str,
    ) -> Result<(), ConverterError> {
        epub.extract(&self.working_dir)?;
        self.convert_opf()?;
        self.convert_html()?;

        match PathBuf::from(out_path).parent() {
            Some(p) => std::fs::create_dir_all(p)?,
            None => {
                return Err(io_err!(
                    std::io::ErrorKind::Other,
                    "Cannot get parent of output path: {}",
                    out_path
                ))
            }
        };
        self.write(out_path)?;
        return Ok(());
    }

    // Write contents of temporary working dir to kepub
    fn write(&self, out_path: &str) -> Result<(), std::io::Error> {
        let outzip_file = File::create(&out_path)?;
        let mut zip_arch = ZipWriter::new(outzip_file);

        let opts = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o755);

        let walkdir = walkdir::WalkDir::new(&self.working_dir).into_iter();

        for entry in walkdir {
            let file = match entry {
                Ok(o) => o,
                Err(e) => {
                    println!("Cannot zip file: {}", e);
                    continue;
                }
            };
            let path = file.path();

            let path_internal = path
                .strip_prefix(&self.working_dir)
                .unwrap()
                .components()
                .map(|x| x.as_os_str().to_str().unwrap())
                .collect::<Vec<&str>>()
                .join("/");

            let name = path.strip_prefix(&self.working_dir).unwrap();

            if path.is_file() {
                zip_arch.start_file(path_internal, opts)?;
                let content = std::fs::read(path)?;
                zip_arch.write_all(&content)?;
            } else if !name.as_os_str().is_empty() {
                zip_arch.add_directory(path_internal, opts)?;
            }
        }

        zip_arch.finish()?;
        return Ok(());
    }

    // Adds `properties='cover-image' attribute to cover image <item> element`
    fn convert_opf(&self) -> Result<(), ConverterError> {
        let fpath = match self.get_opt_path() {
            Some(f) => f,
            None => return Err(xml_err!("Could not find content.opf in epub archive")),
        };

        let mut root = Element::parse(std::fs::File::open(&fpath)?)?;

        let cover_id: String;
        {
            let meta_elem = match root.find_first_child_with_attrs("meta", &[("name", "cover")]) {
                Some(e) => e,
                None => {
                    return Err(xml_err!(
                        "Cannot find <meta name='cover'> element in content.opf"
                    ))
                }
            };

            cover_id = match meta_elem.attributes.get("content") {
                Some(c) => c.clone(),
                None => {
                    return Err(xml_err!(
                    "Cannot read content attribute in <meta name='cover'> element in content.opf"
                ))
                }
            };
        }

        match root.find_first_child_with_attrs_mut("item", &[("id", &cover_id)]) {
            Some(e) => e
                .attributes
                .insert("properties".to_string(), "cover-image".to_string()),
            None => {
                return Err(xml_err!(
                    "Cannot find <item id='{}'> element in content.opf",
                    cover_id
                ))
            }
        };

        return match root
            .write_with_config(std::fs::File::create(&fpath)?, self.write_config.clone())
        {
            Ok(_) => Ok(()),
            Err(e) => Err(e.into()),
        };
    }

    fn get_opt_path(&self) -> Option<PathBuf> {
        let rd = match read_dir(&self.working_dir) {
            Ok(rd) => rd,
            Err(_) => return None,
        };
        for entry in rd {
            match entry {
                Ok(e) => {
                    if e.file_name() == "content.opf" {
                        return Some(e.path());
                    }
                }
                Err(_) => {}
            }
        }
        return None;
    }

    fn convert_html(&self) -> Result<(), ConverterError> {
        let fpath = match self.get_opt_path() {
            Some(f) => f,
            None => return Err(xml_err!("Could not find content.opf in epub archive")),
        };
        let now = std::time::Instant::now();

        let doc = Element::parse(std::fs::File::open(&fpath)?)?;

        let mut hrefs = Vec::new();
        for d in doc.descendants() {
            if d.name != "item" {
                continue;
            }
            if d.attributes
                .get("media-type")
                .is_some_and(|val| val == "application/xhtml+xml")
            {
                match d.attributes.get("href") {
                    Some(h) => hrefs.push(h),
                    None => {}
                }
            }
        }

        for h in hrefs {
            self.convert_html_file(&h)?
        }

        println!("{}ms", now.elapsed().as_millis());
        return Ok(());
    }

    fn convert_html_file(&self, rel_path: &str) -> Result<(), ConverterError> {
        println!("Converting {}", rel_path);
        let fpath = self.working_dir.join(rel_path);

        let mut root = Element::parse(std::fs::File::open(&fpath)?)?;

        let body = match root.get_mut_child("body") {
            Some(e) => e,
            None => return Err(xml_err!("Cannot find <body> in {}", rel_path)),
        };

        let mut bk_col = Element::new("div");
        bk_col
            .attributes
            .insert("id".to_string(), "book-columns".to_string());
        let mut bk_inn = Element::new("div");
        bk_inn
            .attributes
            .insert("id".to_string(), "book-inner".to_string());

        bk_inn.children = body.children.drain(..).collect();

        bk_col.children.push(XMLNode::Element(bk_inn));
        body.children.push(XMLNode::Element(bk_col));

        self.convert_kobo_spans(body);

        return match root
            .write_with_config(std::fs::File::create(&fpath)?, self.write_config.clone())
        {
            Ok(_) => Ok(()),
            Err(e) => Err(e.into()),
        };
    }

    /// Convert paragraphs and sentences into kobospans
    /// Since Rust doesn't play nice with mutable iterators over nested structs
    /// this calls a recursive method to process the text content
    fn convert_kobo_spans(&self, root_elem: &mut Element) {
        if root_elem.descendants().any(|n| {
            n.attributes
                .get("class")
                .is_some_and(|cl| cl.contains("kobospan"))
        }) {
            println!("kobo spans found, not converting html content");
            // kobo spans exist, don't do anything
            return;
        }

        let new_children = self._convert_kobo_spans(root_elem, &mut 0, &mut 0, &mut false);
        root_elem.children = new_children;
    }

    fn _convert_kobo_spans(
        &self,
        parent_elem: &mut Element,
        para: &mut usize,
        sent: &mut usize,
        force_new_para: &mut bool,
    ) -> Vec<XMLNode> {
        let mut new_children = Vec::new();
        for child in parent_elem.children.drain(0..) {
            match child {
                XMLNode::Element(mut element) => {
                    match &*element.name {
                        // img elements get wrapped in their own para
                        "img" => {
                            *para += 1;
                            *sent = 0;
                            *force_new_para = false;

                            let mut s = make_span(*para, *sent, None);
                            s.children.push(XMLNode::Element(element.clone()));
                            new_children.push(XMLNode::Element(s));
                        }
                        // force start a new para after these elems
                        n if ["p", "ol", "ul", "table"].contains(&n)
                            || (n.len() == 2 && n[0..1] == *"h") =>
                        {
                            *force_new_para = true;
                        }
                        n if ["math", "svg"].contains(&n) => continue,
                        _ => {}
                    }

                    new_children.append(&mut self._convert_kobo_spans(
                        &mut element,
                        para,
                        sent,
                        force_new_para,
                    ));
                }
                XMLNode::Text(t) => {
                    let sentences = split_sentences(&t);

                    // // wrap each sentence in a span (don't wrap whitespace unless it is
                    // // directly under a P tag [TODO: are there any other cases we wrap
                    // // whitespace? ... I need to find a kepub like this]) and add it
                    // // back to the parent.
                    for sentence in sentences {
                        if sentence.trim().len() == 0 && parent_elem.name != "p" {
                            // whitespace sentence directly inside <p> -- do nothing
                        } else {
                            if *force_new_para {
                                *para += 1;
                                *sent = 0;
                                *force_new_para = false;
                            }
                            *sent += 1;
                            new_children.push(XMLNode::Element(make_span(
                                *para,
                                *sent,
                                Some(&sentence),
                            )));
                        }
                    }
                }
                _ => {}
            }
        }
        return new_children;
    }
}

fn make_span(para: usize, seg: usize, content: Option<&String>) -> Element {
    let mut e = Element::new("span");
    e.attributes = HashMap::from([
        ("class".to_string(), "kobospan".to_string()),
        ("id".to_string(), format!("kobo.{}.{}", para, seg)),
    ]);
    match content {
        Some(c) => {
            e.children.push(XMLNode::Text(c.clone()));
        }
        None => todo!(),
    }
    return e;
}

/// Splits text content into sentences for kobospans
/// There's no rules as to how precise this needs to be, but this tries
/// to split input text into
fn split_sentences(text: &String) -> Vec<String> {
    #[derive(PartialEq)]
    enum Input {
        PunctStandard,
        PunctExtra,
        Whitespace,
        Other,
        EOS,
    }

    enum Output {
        None,
        Next,
        Rest,
    }

    #[derive(PartialEq)]
    enum State {
        Default,
        AfterPunct,
        AfterPunctExtra,
        AfterSpace,
        Finished,
    }

    let mut sentences = Vec::new();
    let characters = text.chars().collect::<Vec<_>>();

    let mut seg_begin = 0;
    let mut i = 0;
    let mut state = State::Default;
    while state != State::Finished {
        let input: Input;

        if i >= characters.len() {
            input = Input::EOS;
        } else {
            let c = characters[i];
            input = match c {
                _ if ['.', '!', '?'].contains(&c) => Input::PunctStandard,
                _ if ['\'', '"', '”', '’', '“', '…'].contains(&c) => Input::PunctExtra,
                _ if ['\n', '\r', '\t', ' '].contains(&c) => Input::Whitespace,
                _ => Input::Other,
            };
        }

        let output: Output;

        (output, state) = match state {
            State::Default => match input {
                Input::PunctStandard => (Output::None, State::AfterPunct),
                Input::PunctExtra => (Output::None, State::Default),
                Input::Whitespace => (Output::None, State::Default),
                Input::Other => (Output::None, State::Default),
                Input::EOS => (Output::Rest, State::Finished), //
            },
            State::AfterPunct => match input {
                Input::PunctStandard => (Output::None, State::AfterPunct),
                Input::PunctExtra => (Output::None, State::AfterPunctExtra),
                Input::Whitespace => (Output::None, State::AfterSpace),
                Input::Other => (Output::None, State::Default),
                Input::EOS => (Output::Rest, State::Finished), //
            },
            State::AfterPunctExtra => match input {
                Input::PunctStandard => (Output::None, State::AfterPunct),
                Input::PunctExtra => (Output::None, State::Default),
                Input::Whitespace => (Output::None, State::AfterSpace),
                Input::Other => (Output::None, State::Default),
                Input::EOS => (Output::Rest, State::Finished), //
            },
            State::AfterSpace => match input {
                Input::PunctStandard => (Output::Next, State::AfterPunct),
                Input::PunctExtra => (Output::Next, State::Default),
                Input::Whitespace => (Output::None, State::AfterSpace),
                Input::Other => (Output::Next, State::Default),
                Input::EOS => (Output::Rest, State::Finished), //
            },
            State::Finished => (Output::Rest, state),
        };

        match output {
            Output::None => i += 1,
            Output::Next => {
                sentences.push(
                    text.chars()
                        .skip(seg_begin)
                        .take(i - seg_begin)
                        .collect::<String>(),
                );
                seg_begin = i;
                i += 1;
            }
            Output::Rest => {
                // if we've reached the end of the string but found no sentences
                // treat the input text as one sentence and push it
                if sentences.len() == 0 {
                    sentences.push(text.clone());
                } else if i > (seg_begin + 1) {
                    sentences.push(text.chars().skip(seg_begin).collect::<String>());
                }
            }
        }
    }

    return sentences;
}

mod test {
    use super::split_sentences;

    #[test]
    fn test_split_sentences() {
        let text = r#"Left Munich at 8:35 P.M., on 1st May, arriving at Vienna early next morning; should have arrived at 6:46, but train was an hour late. Buda-Pesth seems a wonderful place, from the glimpse which I got of it from the train and the little I could walk through the streets. I feared to go very far from the station, as we had arrived late and would start as near the correct time as possible."#;

        assert_eq!(split_sentences(&text.to_string()).len(), 3);
    }
}
