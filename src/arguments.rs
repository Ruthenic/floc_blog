use std::env::ArgsOs;
use std::ffi::OsString;
use std::path::PathBuf;

macro_rules! mark_used {
	($used:tt) => {};
}

macro_rules! arg_parse_error {
	( $($arg:tt)* ) => {{
		eprintln!("Error parsing arguments: {}", format_args!($($arg)*));
		std::process::exit(-1);
	}};
}

fn get_next_arg(args: &mut ArgsOs) -> OsString {
	if let Some(arg) = args.next() {
		arg
	} else {
		arg_parse_error!("Expected at least one more argument");
	}
}

macro_rules! define_flags {
	(
		$(
			activity $activity_name:ident ($activity_short_flag:literal, $activity_long_flag:literal) $activity_blurb:literal
			$({ withoutarg() $activity_withoutarg_block:block })?
			$({ witharg($activity_witharg_name:ident) $activity_witharg_block:block })?
		,)*

		$(
			optional $optional_name:ident ($optional_short_flag:literal, $optional_long_flag:literal) $optional_blurb:literal -> $optional_return_type:ty
			$({ withoutarg() $optional_withoutarg_block:block })?
			$({ witharg($optional_witharg_name:ident) $optional_witharg_block:block} )?
		,)*

		$(
			required $required_name:ident ($required_short_flag:literal, $required_long_flag:literal) $required_blurb:literal -> $required_return_type:ty
			$({ withoutarg() $required_withoutarg_block:block })?
			$({ witharg($required_witharg_name:ident) $required_witharg_block:block })?
		,)*
	) => {
		#[derive(Debug, Clone)]
		pub struct Arguments {
			$($optional_name: Option<$optional_return_type> ,)*
			$($required_name: $required_return_type ,)*
		}

		struct FlagParser;

		impl FlagParser {
			$($( fn $activity_name() $activity_withoutarg_block )?)?
			$($( fn $activity_name($activity_witharg_name: OsString) $activity_witharg_block )?)?

			$($( fn $optional_name() -> $optional_return_type $optional_withoutarg_block )?)?
			$($( fn $optional_name($optional_witharg_name: OsString) -> $optional_return_type $optional_witharg_block )?)?

			$($( fn $required_name() -> $required_return_type $required_withoutarg_block )?)?
			$($( fn $required_name($required_witharg_name: OsString) -> $required_return_type $required_witharg_block )?)?
		}

		pub fn parse() -> Arguments {
			struct ValueTracker {
				$($optional_name: Option<$optional_return_type> ,)*
				$($required_name: Option<$required_return_type> ,)*
			}

			let mut tracker = ValueTracker {
				$($optional_name: None ,)*
				$($required_name: None ,)*
			};

			let mut args = std::env::args_os();
			args.next().expect("There was no first argument to dispose of");
			while let Some(selector) = args.next() {
				match selector.to_str() {
					$(Some($activity_short_flag) | Some($activity_long_flag) => {
						(|| {
							$(
								return FlagParser::$activity_name();
								mark_used!($activity_withoutarg_block);
							)?
							$(
								let next = get_next_arg(&mut args);
								return FlagParser::$activity_name(next);
								mark_used!($activity_witharg_block);
							)?
						})();
					})*

					$(Some($optional_short_flag) | Some($optional_long_flag) => {
						tracker.$optional_name = Some((|| {
							$(
								return FlagParser::$optional_name();
								mark_used!($optional_withoutarg_block);
							)?
							$(
								let next = get_next_arg(&mut args);
								return FlagParser::$optional_name(next);
								mark_used!($optional_witharg_block);
							)?
						})());
					})*

					$(Some($required_short_flag) | Some($required_long_flag) => {
						tracker.$required_name = Some((|| {
							$(
								return FlagParser::$required_name();
								mark_used!($required_withoutarg_block);
							)?
							$(
								let next = get_next_arg(&mut args);
								return FlagParser::$required_name(next);
								mark_used!($required_witharg_block);
							)?
						})());
					})*

					_ => arg_parse_error!("Unexpected argument '{}'", selector.to_string_lossy()),
				}
			}

			$(
				let $optional_name = tracker.$optional_name;
			)*
			$(
				let $required_name = if let Some(value) = tracker.$required_name {
					value
				} else {
					arg_parse_error!("Missing required flag '{}'", $required_long_flag);
				};
			)*

			Arguments {
				$($optional_name,)*
				$($required_name,)*
			}
		}

		pub fn print_help() {
			const INDENT: &str = "    ";

			println!("floc_blog, a small barebones static blog generator");
			println!();

			println!("USAGE:");
			println!("{}floc_blog [ACTION]", INDENT);
			println!("{}floc_blog [FLAGS]", INDENT);
			println!();

			println!("ACTIONS:");
			$(
				print!("{}", INDENT);
				print!("{} {}", stringify!($activity_short_flag), stringify!($activity_long_flag));
				println!("\t{}", $activity_blurb);
			)*
			println!();

			println!("FLAGS:");
			$(
				print!("{}", INDENT);
				print!("{} {}", stringify!($optional_short_flag), stringify!($optional_long_flag));
				println!("\t(optional) {}", $optional_blurb);
			)*
			$(
				print!("{}", INDENT);
				print!("{} {}", stringify!($required_short_flag), stringify!($required_long_flag));
				println!("\t(required) {}", $required_blurb);
			)*

			println!();
		}
	};
}

define_flags! {
	activity print_help ("-h", "--help") "Print this help message" {
		withoutarg() {
			print_help();
			std::process::exit(0);
		}
	},

	optional fragments_dir ("-f", "--fragments") "Directory to retrive html footer/header/ect fragments from" -> PathBuf {
		witharg(dir) {
			dir.into()
		}
	},

	required input_dir ("-i", "--input") "Input directory to scan for .md files" -> PathBuf {
		witharg(dir) {
			dir.into()
		}
	},
	required output_dir ("-o", "--output") "Output directory to place .html files" -> PathBuf {
		witharg(dir) {
			dir.into()
		}
	},
}
