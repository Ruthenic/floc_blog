use std::ffi::OsStr;
use std::fmt::Write;
use std::fs::File;
use std::io::prelude::*;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};

use pulldown_cmark::{html, CodeBlockKind, CowStr, Event, Options, Parser, Tag};

mod arguments;
mod template;

use arguments::Arguments;
use template::format_template;

pub const VERSION: &str = "0.0.1";

macro_rules! multiline {
	( $($line:expr)* ) => {
		concat!( $($line, "\n"),* )
	}
}

macro_rules! map {
	[ $($key:expr => $value:expr,)* ] => {{
		let mut map = std::collections::HashMap::new();
		$(
			map.insert($key, $value);
		)*
		map
	}}
}

#[derive(Debug)]
struct BlogEntry {
	url_name: String,
	title: String,
	description: String,
	date: DateTime<Utc>,
}

#[derive(Debug)]
struct Fragments {
	css: String,
	header: String,
	footer: String,
	blog_entry: String,
	blog_list: String,
}

impl Fragments {
	fn retrive_or_shim(dir: Option<PathBuf>) -> Fragments {
		let mut dir = match dir {
			Some(dir) => dir,

			None => {
				return Fragments {
					css: String::new(),
					header: String::new(),
					footer: String::new(),
					blog_entry: String::new(),
					blog_list: String::new(),
				};
			}
		};

		fn get_fragment(dir: &mut PathBuf, name: &str) -> String {
			dir.push(name);

			let fragment = match std::fs::read_to_string(&dir) {
				Ok(fragment) => fragment.trim().to_string(),

				Err(err) => {
					eprintln!("Error loading fragment '{}': {}", name, err);
					std::process::exit(-1);
				}
			};

			dir.pop();
			fragment
		}

		let css = get_fragment(&mut dir, "style.css");
		let header = get_fragment(&mut dir, "header.html");
		let footer = get_fragment(&mut dir, "footer.html");
		let blog_entry = get_fragment(&mut dir, "blog_entry.html");
		let blog_list = get_fragment(&mut dir, "blog_list.html");

		Fragments {
			css,
			header,
			footer,
			blog_entry,
			blog_list,
		}
	}
}

struct Buffers {
	input: String,
	html: String,
	output: String,

	title: String,
	description: String,
	author: String,
	date: String,
}

fn process_markdown(fragments: &Fragments, args: &Arguments, buffers: &mut Buffers) {
	let mut options = Options::empty();
	options.insert(Options::ENABLE_TABLES);
	let parser = Parser::new_ext(&buffers.input, options);

	/*
	 * NOTE: Borrowing these here borrows just the field instead of the entire
	 * struct which allows the closure to have mutable access to these two fields
	 * while `html::push_html` writes to another field.
	 */
	let title_buffer = &mut buffers.title;
	title_buffer.clear();
	let description_buffer = &mut buffers.description;
	description_buffer.clear();
	let author_buffer = &mut buffers.author;
	author_buffer.clear();
	let date_buffer = &mut buffers.date;
	date_buffer.clear();

	let parser = parser.map(|event| {
		if let Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(language))) = &event {
			if *language == CowStr::Borrowed("image_description") {
				return Event::Html(CowStr::Borrowed(r#"<div class="ImageDescription"><p>"#));
			}
		}

		if let Event::End(Tag::CodeBlock(CodeBlockKind::Fenced(language))) = &event {
			if *language == CowStr::Borrowed("image_description") {
				return Event::Html(CowStr::Borrowed(r#"</p></div>"#));
			}
		}

		if let Event::Html(html) = &event {
			let html = html.trim();
			if html.starts_with("<!--") && html.ends_with("-->") {
				//We are reasonably confident that this is an HTML comment

				let contents = &html["<!--".len()..];
				let contents = &contents[..contents.len() - "-->".len()];

				if let Some(colon_index) = contents.find(':') {
					let label = &contents[..colon_index];
					let trailing = contents[colon_index + 1..].trim();

					match label {
						"title" => {
							title_buffer.clear();
							title_buffer.push_str(trailing);
						}

						"description" => {
							description_buffer.clear();
							description_buffer.push_str(trailing);
						}

						"author" => {
							author_buffer.clear();
							author_buffer.push_str(trailing);
						}

						"date" => {
							date_buffer.clear();
							date_buffer.push_str(trailing);
						}

						_ => {}
					}
				}
			}
		}

		event
	});

	buffers.html.clear();
	html::push_html(&mut buffers.html, parser);

	buffers.output.clear();
	buffers.output.push_str("<!DOCTYPE html>\n");
	if let Some(language) = &args.language {
		let _ = writeln!(buffers.output, r#"<html lang="{}">"#, language);
	}
	buffers.output.push_str(multiline!(
		"\n<head>"
		r#"<meta charset="UTF-8">"#
	));
	if !buffers.title.is_empty() {
		let _ = writeln!(buffers.output, "<title>{}</title>", buffers.title);
	}
	if let Some(favicon) = &args.favicon {
		let _ = writeln!(
			buffers.output,
			r#"<link rel="shortcut icon" type="image/png" href="{}" />"#,
			favicon
		);
	}
	if !buffers.description.is_empty() {
		let _ = write!(
			buffers.output,
			multiline!(
				r#"<meta name="description" content="{description}" />"#
				r#"<meta property="og:description" content="{description}" />"#
			),
			description = buffers.description,
		);
	}
	if !buffers.author.is_empty() {
		let _ = writeln!(
			buffers.output,
			r#"<meta name="author" content="{}" />"#,
			buffers.author
		);
	}
	if let Some(opengraph_locale) = &args.opengraph_locale {
		let _ = writeln!(
			buffers.output,
			r#"<meta property="og:locale" content="{}" />"#,
			opengraph_locale
		);
	}
	if let Some(opengraph_sitename) = &args.opengraph_sitename {
		let _ = writeln!(
			buffers.output,
			r#"<meta property="og:site_name" content="{}" />"#,
			opengraph_sitename
		);
	}

	if !fragments.css.is_empty() {
		buffers.output.push_str("<style>\n");
		buffers.output.push_str(&fragments.css);
		buffers.output.push_str("</style>\n");
	}

	buffers.output.push_str("</head>\n\n");

	if !fragments.header.is_empty() {
		buffers.output.push_str(&fragments.header);
		buffers.output.push_str("\n\n");
	}

	buffers.output.push_str(&buffers.html);

	if !fragments.footer.is_empty() {
		buffers.output.push_str("\n\n");
		buffers.output.push_str(&fragments.footer);
	}
}

fn process_file(
	args: &Arguments,
	path: &Path,
	output_path: PathBuf,
	url_name: &str,
	fragments: &Fragments,
	buffers: &mut Buffers,
	blog_entries: &mut Vec<BlogEntry>,
) {
	if let Some(dir_path) = output_path.parent() {
		/*
		 * NOTE: Silently swallow failure to create output path.
		 * If the path does not exist the write will still catch
		 * the error. Otherwise if this failed for some other
		 * reason but the write can still succeed then we do not
		 * care that this failed.
		 */
		let _ = std::fs::create_dir_all(dir_path);
	}

	let is_markdown = path.extension().map(|p| p.to_str()) == Some(Some("md"));

	if !is_markdown {
		if let Err(err) = std::fs::copy(&path, &output_path) {
			eprintln!(
				"Error copying input file '{}' to '{}': {}",
				path.to_string_lossy(),
				output_path.to_string_lossy(),
				err
			);
			std::process::exit(-1);
		}
	} else {
		let mut file = match File::open(&path) {
			Ok(file) => file,

			Err(err) => {
				eprintln!(
					"Error reading input file '{}': {}",
					path.to_string_lossy(),
					err
				);
				std::process::exit(-1);
			}
		};

		buffers.input.clear();
		if let Err(err) = file.read_to_string(&mut buffers.input) {
			eprintln!(
				"Error reading input markdown file '{}': {}",
				path.to_string_lossy(),
				err
			);
			std::process::exit(-1);
		}

		process_markdown(fragments, args, buffers);

		fn check_error<'a>(text: &'a str, attribute: &str, path: &Path) -> &'a str {
			if text.is_empty() {
				eprintln!(
					"Error input file '{}' is missing {} attribute",
					path.to_string_lossy(),
					attribute
				);
				std::process::exit(-1);
			} else {
				text
			}
		}

		let title = check_error(&buffers.title, "title", &path).to_string();
		let description = check_error(&buffers.description, "description", &path).to_string();

		let date = check_error(&buffers.date, "date", &path);
		let date = match DateTime::parse_from_str(date, "%d %b %Y %H:%M:%S %z") {
			Ok(date) => date,
			Err(err) => {
				eprintln!(
					"Error parsing date attribute in input file '{}': {}",
					path.to_string_lossy(),
					err
				);
				std::process::exit(-1);
			}
		};

		let blog_entry = BlogEntry {
			url_name: url_name.to_string(),
			title,
			description,
			date: date.into(),
		};
		blog_entries.push(blog_entry);

		if let Err(err) = std::fs::write(&output_path, &buffers.output) {
			eprintln!(
				"Error writing HTML to path '{}': {}",
				output_path.to_string_lossy(),
				err
			);
			std::process::exit(-1);
		}
	}
}

fn process_dir(
	args: &Arguments,
	folder_name: &OsStr,
	dir_path: &Path,
	fragments: &Fragments,
	buffers: &mut Buffers,
	blog_entries: &mut Vec<BlogEntry>,
) {
	let url_name = folder_name.to_string_lossy();
	let dir = match std::fs::read_dir(dir_path) {
		Ok(dir) => dir,

		Err(err) => {
			eprintln!(
				"Error opening dir '{}': {}",
				dir_path.to_string_lossy(),
				err
			);
			std::process::exit(-1);
		}
	};

	for entry in dir {
		match entry {
			Ok(entry) => {
				let file_path = entry.path();
				let file_name = file_path.file_name().unwrap_or_else(|| {
					eprintln!(
						"Failed to get filename for '{}'",
						file_path.to_string_lossy()
					);
					std::process::exit(-1);
				});
				let extension = file_path
					.extension()
					.map(|e| e.to_str())
					.unwrap_or(Some(""))
					.unwrap_or("");

				let output_path = {
					let mut output_path = args.output_dir.clone();
					output_path.push(folder_name);

					if extension == "md" {
						if file_name != "content.md" {
							eprintln!(
								"Error, markdown file '{}' is not named 'content.md'",
								file_path.to_string_lossy()
							);
							std::process::exit(-1);
						}
						output_path.push("index.html");
					} else {
						output_path.push(file_name);
					}

					output_path
				};

				process_file(
					args,
					&file_path,
					output_path,
					&url_name,
					fragments,
					buffers,
					blog_entries,
				);
			}

			Err(err) => {
				eprintln!(
					"Error walking dir '{}': {}",
					dir_path.to_string_lossy(),
					err
				);
				std::process::exit(-1);
			}
		}
	}
}

fn format_rss(args: &Arguments, blog_entries: &[BlogEntry]) -> String {
	let items = {
		let mut items = String::new();

		for entry in blog_entries {
			write!(
				items,
				multiline!(
					"<item>"
					"	<description>{description}</description>"
					"	<pubDate>{date}</pubDate>"
					"	<link>{base_url}/{url_name}</link>"
					"</item>"
				),
				description = entry.description,
				date = entry.date.to_rfc2822(),
				base_url = args.blog_base_url,
				url_name = entry.url_name,
			)
			.unwrap();
		}

		items
	};

	let rss = format!(
		multiline!(
			r#"<?xml version="1.0"?>"#
			"<!--RSS generated {date} by floc_blog {version}-->"
			r#"<rss version="2.0">"#
			"<language>{language}</language>"
			"<generator>floc_blog {version}</generator>"
			r#"<channel>"#
			"\n{items}"
			r#"</channel>"#
			r#"</rss>"#
		),
		date = Utc::now().to_rfc2822(),
		version = VERSION,
		language = args.language.clone().unwrap_or_else(|| "en_US".to_string()),
		items = items,
	);

	rss
}

fn format_blog_list(
	args: &Arguments,
	blog_entries: Vec<BlogEntry>,
	fragments: Fragments,
) -> String {
	let formatted_entries = {
		let mut formatted_entries = String::new();

		for entry in blog_entries {
			let link = format!("{}/{}", args.blog_base_url, entry.url_name);
			let template_values = map![
				"TITLE" => entry.title,
				"DESCRIPTION" => entry.description,
				"DATE" => format!("{}", entry.date.format("%A the %eth of %B %Y")),
				"LINK" => link,
			];

			let formatted = format_template(fragments.blog_entry.clone(), template_values);
			formatted_entries.push_str(&formatted);
		}
		formatted_entries
	};

	let template_values = map![
		"ENTRIES" => formatted_entries,
	];
	format_template(fragments.blog_list, template_values)
}

fn main() {
	let args = arguments::parse();

	let fragments = Fragments::retrive_or_shim(args.fragments_dir.clone());

	let input_dir = match std::fs::read_dir(&args.input_dir) {
		Ok(input_dir) => input_dir,

		Err(err) => {
			eprintln!(
				"Error opening input dir '{}': {}",
				args.input_dir.to_string_lossy(),
				err
			);
			std::process::exit(-1);
		}
	};

	/*
	 * NOTE: Silently swallow error here because it can fail
	 * if the folder does not already exist which is fine.
	 * If there really is something wrong with the path or
	 * permissions or whatever then the actual outputting will
	 * catch that. Otherwise we are uninterested in failure
	 * here.
	 */
	let _ = std::fs::remove_dir_all(&args.output_dir);

	let mut blog_entries = Vec::new();

	let mut buffers = Buffers {
		input: String::new(),
		html: String::new(),
		output: String::new(),
		title: String::new(),
		description: String::new(),
		author: String::new(),
		date: String::new(),
	};

	for entry in input_dir {
		match entry {
			Ok(entry) => {
				let path = entry.path();

				let file_name = path.file_stem().map(|name| name.to_str());
				if let Some(Some("index")) = file_name {
					eprintln!(
						"Error, file '{}' should not be named 'index.*'",
						path.to_string_lossy(),
					);
					std::process::exit(-1);
				}

				let is_dir = entry.file_type().map(|e| e.is_dir()).unwrap_or(false);

				if is_dir {
					let folder_name = path
						.file_name()
						.expect("Somehow failed to get folder filename");

					process_dir(
						&args,
						folder_name,
						&path,
						&fragments,
						&mut buffers,
						&mut blog_entries,
					);
				} else {
					eprintln!(
						"Found file '{}' at root level in input directory",
						path.to_string_lossy()
					);
					std::process::exit(-1);
				}
			}

			Err(err) => {
				eprintln!("Error walking input dir: {}", err);
				std::process::exit(-1);
			}
		}
	}

	blog_entries.sort_by(|left, right| right.date.cmp(&left.date));

	{
		let rss = format_rss(&args, &blog_entries);

		let mut output_path = args.output_dir.clone();
		output_path.push("feed.rss");

		if let Err(err) = std::fs::write(&output_path, &rss) {
			eprintln!(
				"Error writing RSS feed file'{}': {}",
				output_path.to_string_lossy(),
				err
			);
			std::process::exit(-1);
		}
	}

	{
		let list_page = format_blog_list(&args, blog_entries, fragments);

		let mut output_path = args.output_dir;
		output_path.push("index.html");

		if let Err(err) = std::fs::write(&output_path, &list_page) {
			eprintln!(
				"Error writing blog entry list '{}': {}",
				output_path.to_string_lossy(),
				err
			);
			std::process::exit(-1);
		}
	}
}
